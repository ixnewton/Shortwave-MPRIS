// Shortwave - ffmpeg_wrapper.rs
// Copyright (C) 2024  Felix Häcker <haeckerfelix@gnome.org>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::sync::{Arc, mpsc, atomic::{AtomicU64, Ordering}};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::process::Child;
use uuid::Uuid;

// Commands sent to FFmpeg thread
#[derive(Debug, Clone)]
pub enum FfmpegCommand {
    StartStream {
        stream_url: String,
        stream_id: String,
        force_restart: bool,
    },
    StopStream,
    GetStatus,
    Shutdown,
}

// Status reports from FFmpeg thread
#[derive(Debug, Clone)]
pub enum FfmpegStatus {
    Starting { stream_id: String },
    Streaming { 
        stream_id: String,
        proxy_url: String,
        bytes_sent: u64,
        duration: Duration,
    },
    Stopped { stream_id: String, reason: String },
    Error { stream_id: String, error: String },
}

// Stream output format options
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Mp3 { bitrate: u32 },
    Aac { bitrate: u32 },
    Opus { bitrate: u32 },
    Passthrough, // No transcoding
}

// Stream type detection
#[derive(Debug, Clone, PartialEq)]
pub enum StreamType {
    Mp3,
    Aac,
    Hls,
    Ogg,
    Unknown,
}

// Parameters for starting a stream
#[derive(Debug, Clone)]
pub struct StreamStartParams {
    // Input stream parameters
    pub stream_url: String,
    pub stream_id: String,
    
    // Network parameters
    pub local_ip: String,
    pub listen_port: u16, // 0 for auto-assign
    
    // Transcoding parameters
    pub force_transcode: bool,
    pub output_format: OutputFormat,
    pub bitrate: Option<u32>, // e.g., 128000
    
    // Optional metadata
    pub station_title: String,
    pub station_favicon: String,
}

// FFmpeg session state
#[derive(Debug)]
struct FfmpegSession {
    stream_id: String,
    stream_url: String,
    proxy_url: String,
    process: Child,
    start_time: Instant,
    bytes_sent: Arc<AtomicU64>,
    is_transcoding: bool,
}

// Main FFmpeg wrapper thread
#[derive(Debug)]
pub struct FfmpegWrapper {
    // Thread handle
    thread_handle: Option<JoinHandle<()>>,
    
    // Command channel (single producer, single consumer)
    command_sender: Option<mpsc::Sender<FfmpegCommand>>,
    
    // Status reporting channel
    status_sender: Option<mpsc::Sender<FfmpegStatus>>,
}

impl FfmpegWrapper {
    pub fn new() -> Self {
        Self {
            thread_handle: None,
            command_sender: None,
            status_sender: None,
        }
    }
    
    /// Start the FFmpeg wrapper thread
    pub fn start(&mut self) -> Result<(), String> {
        if self.thread_handle.is_some() {
            return Err("FFmpeg wrapper already running".to_string());
        }
        
        let (cmd_sender, cmd_receiver) = mpsc::channel::<FfmpegCommand>();
        let (status_sender, status_receiver) = mpsc::channel::<FfmpegStatus>();
        
        self.command_sender = Some(cmd_sender);
        self.status_sender = Some(status_sender.clone());
        
        // Spawn the wrapper thread
        let handle = thread::spawn(move || {
            Self::ffmpeg_thread_main(cmd_receiver, status_receiver, status_sender);
        });
        
        self.thread_handle = Some(handle);
        Ok(())
    }
    
    /// Send a command to the FFmpeg thread
    pub fn send_command(&self, command: FfmpegCommand) -> Result<(), String> {
        if let Some(ref sender) = self.command_sender {
            sender.send(command)
                .map_err(|e| format!("Failed to send command: {}", e))
        } else {
            Err("FFmpeg wrapper not started".to_string())
        }
    }
    
    /// Check if the wrapper has an active session
    pub fn has_active_session(&self) -> bool {
        // Send a GetStatus command to check
        if let Some(ref sender) = self.command_sender {
            // Create a temporary channel for the response
            let (resp_sender, resp_receiver) = mpsc::channel::<bool>();
            
            // For now, we'll use a simple approach - just check if we have a sender
            // In a more complete implementation, we'd have a status tracking mechanism
            true // Placeholder - actual implementation would track session state
        } else {
            false
        }
    }
    
    /// Main thread function for FFmpeg wrapper
    fn ffmpeg_thread_main(
        command_receiver: mpsc::Receiver<FfmpegCommand>,
        _status_receiver: mpsc::Receiver<FfmpegStatus>,
        status_sender: mpsc::Sender<FfmpegStatus>,
    ) {
        info!("FFMPEG-WRAPPER: Thread started");
        
        let mut current_session: Option<FfmpegSession> = None;
        
        // Process commands
        while let Ok(command) = command_receiver.recv() {
            match command {
                FfmpegCommand::StartStream { stream_url, stream_id, force_restart } => {
                    info!("FFMPEG-WRAPPER: StartStream command for {}", stream_url);
                    
                    // Check if we can reuse existing session
                    let mut can_reuse = false;
                    if let Some(ref mut session) = current_session {
                        // First check if the process is still alive
                        match session.process.try_wait() {
                            Ok(None) => {
                                // Process is still running
                                if session.stream_url == stream_url && !force_restart {
                                    info!("FFMPEG-WRAPPER: Reusing existing session for {}", stream_url);
                                    can_reuse = true;
                                    let _ = status_sender.send(FfmpegStatus::Streaming {
                                        stream_id: session.stream_id.clone(),
                                        proxy_url: session.proxy_url.clone(),
                                        bytes_sent: session.bytes_sent.load(Ordering::Relaxed),
                                        duration: session.start_time.elapsed(),
                                    });
                                }
                            }
                            Ok(Some(_)) => {
                                // Process has already exited
                                warn!("FFMPEG-WRAPPER: Existing process has exited, will start new one");
                            }
                            Err(e) => {
                                // Error checking process status
                                warn!("FFMPEG-WRAPPER: Error checking process status: {}", e);
                            }
                        }
                    }
                    
                    if can_reuse {
                        continue;
                    }
                    
                    // Stop existing session if needed
                    if let Some(mut session) = current_session.take() {
                        info!("FFMPEG-WRAPPER: Stopping existing session");
                        // Kill the process
                        if let Err(e) = session.process.kill() {
                            warn!("FFMPEG-WRAPPER: Failed to kill process: {}", e);
                        }
                        // Wait for the process to actually exit
                        if let Err(e) = session.process.wait() {
                            warn!("FFMPEG-WRAPPER: Error waiting for process to exit: {}", e);
                        } else {
                            info!("FFMPEG-WRAPPER: Process successfully terminated");
                        }
                    }
                    
                    // Start new session
                    match Self::start_ffmpeg_session(&stream_url, &stream_id, &status_sender) {
                        Ok(session) => {
                            let proxy_url = session.proxy_url.clone();
                            current_session = Some(session);
                            
                            let _ = status_sender.send(FfmpegStatus::Streaming {
                                stream_id,
                                proxy_url,
                                bytes_sent: 0,
                                duration: Duration::from_secs(0),
                            });
                        }
                        Err(e) => {
                            error!("FFMPEG-WRAPPER: Failed to start session: {}", e);
                            let _ = status_sender.send(FfmpegStatus::Error {
                                stream_id,
                                error: e,
                            });
                        }
                    }
                }
                
                FfmpegCommand::StopStream => {
                    info!("FFMPEG-WRAPPER: StopStream command");
                    if let Some(mut session) = current_session.take() {
                        // Kill the process
                        if let Err(e) = session.process.kill() {
                            warn!("FFMPEG-WRAPPER: Failed to kill process: {}", e);
                        }
                        // Wait for the process to actually exit
                        if let Err(e) = session.process.wait() {
                            warn!("FFMPEG-WRAPPER: Error waiting for process to exit: {}", e);
                        } else {
                            info!("FFMPEG-WRAPPER: Process successfully terminated");
                        }
                        
                        let _ = status_sender.send(FfmpegStatus::Stopped {
                            stream_id: session.stream_id,
                            reason: "Stop command received".to_string(),
                        });
                    }
                }
                
                FfmpegCommand::GetStatus => {
                    info!("FFMPEG-WRAPPER: GetStatus command");
                    if let Some(ref session) = current_session {
                        let _ = status_sender.send(FfmpegStatus::Streaming {
                            stream_id: session.stream_id.clone(),
                            proxy_url: session.proxy_url.clone(),
                            bytes_sent: session.bytes_sent.load(Ordering::Relaxed),
                            duration: session.start_time.elapsed(),
                        });
                    } else {
                        let _ = status_sender.send(FfmpegStatus::Stopped {
                            stream_id: "none".to_string(),
                            reason: "No active session".to_string(),
                        });
                    }
                }
                
                FfmpegCommand::Shutdown => {
                    info!("FFMPEG-WRAPPER: Shutdown command");
                    if let Some(mut session) = current_session.take() {
                        let _ = session.process.kill();
                    }
                    break;
                }
            }
        }
        
        info!("FFMPEG-WRAPPER: Thread exiting");
    }
    
    /// Start a new FFmpeg session
    fn start_ffmpeg_session(
        stream_url: &str,
        stream_id: &str,
        status_sender: &mpsc::Sender<FfmpegStatus>,
    ) -> Result<FfmpegSession, String> {
        // Send starting status
        let _ = status_sender.send(FfmpegStatus::Starting {
            stream_id: stream_id.to_string(),
        });
        
        // Detect stream type
        let stream_type = Self::detect_stream_type(stream_url);
        info!("FFMPEG-WRAPPER: Detected stream type: {:?}", stream_type);
        
        // Determine if transcoding is needed
        let output_format = if matches!(stream_type, StreamType::Mp3) {
            OutputFormat::Passthrough
        } else {
            OutputFormat::Mp3 { bitrate: 128000 }
        };
        
        // Build FFmpeg command
        let mut args = vec![];
        
        // Add input URL
        info!("FFMPEG-WRAPPER: Adding input URL");
        args.extend_from_slice(&[
            "-i".to_string(),
            stream_url.to_string(),
        ]);
        info!("FFMPEG-WRAPPER: Input URL added, args length: {}", args.len());
        
        // Note: Reconnection options are not used in HTTP server mode
        // as they can cause conflicts with the listen functionality
        
        // Add transcoding options
        match output_format {
            OutputFormat::Mp3 { bitrate } => {
                args.extend_from_slice(&[
                    "-c:a".to_string(),
                    "libmp3lame".to_string(),
                    "-b:a".to_string(),
                    format!("{}k", bitrate / 1000).to_string(),
                    "-f".to_string(),
                    "mp3".to_string(),
                ]);
            }
            OutputFormat::Passthrough => {
                args.extend_from_slice(&[
                    "-c".to_string(),
                    "copy".to_string(),
                ]);
            }
            _ => {}
        }
        
        // Add HTTP server options (use default port 8080)
        args.extend_from_slice(&[
            "-listen".to_string(),
            "1".to_string(),
            "http://0.0.0.0:8080/stream".to_string(),
        ]);
        
        info!("FFMPEG-WRAPPER: Starting FFmpeg with args: {:?}", args);
        debug!("FFMPEG-WRAPPER: Full FFmpeg command: ffmpeg {}", args.join(" "));
        
        // Start FFmpeg process
        let result = std::process::Command::new("ffmpeg")
            .args(&args)
            .spawn();
            
        let process = match result {
            Ok(process) => {
                info!("FFMPEG-WRAPPER: FFmpeg process started successfully");
                process
            }
            Err(e) => {
                error!("FFMPEG-WRAPPER: Failed to start FFmpeg: {}", e);
                error!("FFMPEG-WRAPPER: Command: ffmpeg {}", args.join(" "));
                return Err(format!("Failed to start FFmpeg: {}", e));
            }
        };
        
        // Create session
        let session = FfmpegSession {
            stream_id: stream_id.to_string(),
            stream_url: stream_url.to_string(),
            proxy_url: "http://localhost:8080/stream".to_string(),
            process,
            start_time: Instant::now(),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            is_transcoding: !matches!(output_format, OutputFormat::Passthrough),
        };
        
        info!("FFMPEG-WRAPPER: FFmpeg session started successfully");
        Ok(session)
    }
    
    /// Detect stream type from URL
    fn detect_stream_type(url: &str) -> StreamType {
        if url.ends_with(".mp3") {
            StreamType::Mp3
        } else if url.ends_with(".aac") || url.contains("aac") {
            StreamType::Aac
        } else if url.contains(".m3u8") {
            StreamType::Hls
        } else if url.ends_with(".ogg") || url.contains("opus") {
            StreamType::Ogg
        } else {
            StreamType::Unknown
        }
    }
    
    /// Stop the wrapper thread and clean up
    pub fn shutdown(&mut self) {
        if let Some(sender) = self.command_sender.take() {
            let _ = sender.send(FfmpegCommand::Shutdown);
        }
        
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        
        info!("FFMPEG-WRAPPER: Wrapper shutdown complete");
    }
}

impl Drop for FfmpegWrapper {
    fn drop(&mut self) {
        // Send shutdown command if possible
        if let Some(sender) = self.command_sender.take() {
            let _ = sender.send(FfmpegCommand::Shutdown);
        }
        
        // Join thread
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        
        info!("FFMPEG-WRAPPER: Wrapper dropped");
    }
}
