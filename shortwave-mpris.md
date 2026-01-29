# Shortwave-MPRIS Feature Set

## Overview
Shortwave-MPRIS is an enhanced version of the Shortwave internet radio player that includes MPRIS (Media Player Remote Interfacing Specification) support, DLNA/UPnP streaming, Google Cast support, and advanced FFmpeg proxy capabilities for incompatible streams.

## Core Features

### 📻 Radio Streaming
- **50,000+ Station Database**: Access to community-maintained radio station database via radio-browser.info
- **Multiple Stream Formats**: Support for MP3, AAC, HLS, and other streaming formats
- **GStreamer Backend**: Robust audio pipeline with format auto-detection
- **Stream Metadata**: Real-time display of track titles and station information
- **Automatic Fallback**: FFmpeg transcoding for incompatible stream formats

### 🎵 Playback Control
- **Play/Pause/Stop**: Full control over radio playback
- **Volume Control**: Precise volume adjustment with system integration
- **Station Switching**: Quick switching between stations without stopping playback
- **Auto-play**: Optional automatic playback when selecting stations

### 📚 Library Management
- **Favorites System**: Save and organize favorite radio stations
- **Station Sorting**: Sort by name, country, language, genre, and more
- **Search Functionality**: Full-text search across station database
- **Popular Stations**: Quick access to trending and popular stations
- **Random Discovery**: Discover new stations through random selection
- **Offline Storage**: Local database for saved stations and metadata

### 🎮 MPRIS Integration
- **System-wide Control**: Control playback from system media controls
- **Desktop Integration**: Integration with GNOME, KDE, and other desktop environments
- **Media Keys**: Support for keyboard media keys (play/pause, next, previous)
- **Notification Display**: Track information in system notifications
- **Background Playback**: Continue playback when application is hidden
- **Next/Previous Track**: Navigate through favorite stations using MPRIS

### 📺 Device Streaming

#### DLNA/UPnP Support
- **Device Discovery**: Automatic discovery of DLNA/UPnP devices on local network
- **Direct Streaming**: Stream radio stations to DLNA-compatible devices
- **FFmpeg Integration**: Automatic transcoding for DLNA compatibility
- **Volume Control**: Remote volume control on DLNA devices
- **Multi-device Support**: Switch between multiple DLNA devices
- **Wake-on-LAN**: PrepareForConnection support for devices in suspend mode

#### Google Cast Support
- **Chromecast Support**: Stream to Chromecast and Cast-enabled devices
- **Media Metadata**: Display station and track information on Cast devices
- **FFmpeg Proxy**: Automatic proxy for incompatible streams (HLS, AAC, etc.)
- **Seamless Switching**: Switch between local and Cast playback
- **Auto-proxy Detection**: Automatically detects when Cast devices reject streams
- **MP3 Transcoding**: Converts streams to MP3 at 128kbps for Cast compatibility

### 🔧 FFmpeg Proxy Features
- **Automatic Activation**: Proxy starts only when needed for compatibility
- **Local IP Detection**: Automatically detects network configuration
- **Port 8080**: Uses firewall-friendly port for HTTP streaming
- **Stream Naming**: Uses `.mp3` extension for better device recognition
- **Clean Shutdown**: Proper cleanup when stopping or switching stations
- **Error Handling**: Graceful fallback when proxy fails

### 🎨 User Interface
- **Libadwaita Design**: Modern GNOME-style adaptive interface
- **Dark/Light Theme**: Automatic theme switching based on system preferences
- **Mobile Responsive**: Optimized for mobile devices and Linux phones
- **Compact View**: Player gadget for mini-player mode
- **Station Details**: Detailed information about each radio station
- **Cover Art**: Display station logos and album art when available

### 🔍 Advanced Features
- **Track Recording**: Record currently playing tracks (where supported)
- **Track History**: View history of played songs
- **Station Ratings**: Rate and review radio stations
- **Export/Import**: Backup and restore favorite stations
- **Stream Statistics**: Monitor stream quality and connection status
- **Network Diagnostics**: Tools for troubleshooting streaming issues

### ⚙️ Technical Features
- **Rust Implementation**: Memory-safe and performant native application
- **Async/Await**: Non-blocking operations for smooth UI
- **SQLite Database**: Efficient local storage for stations and metadata
- **Network Discovery**: mDNS/Bonjour for device discovery
- **SOAP Protocol**: DLNA/UPnP communication
- **HTTP Server**: Built-in server for FFmpeg proxy streaming

### 🔐 Privacy & Security
- **No Telemetry**: No data collection or telemetry
- **Local Storage**: All data stored locally
- **Open Source**: Fully auditable codebase
- **Minimal Permissions**: Only requests necessary permissions

### 🌍 Internationalization
- **Multi-language**: Translated into 40+ languages via GNOME Translation Project
- **Unicode Support**: Full support for international station names and metadata
- **Regional Content**: Easy access to local and regional radio stations

## System Requirements

### Dependencies
- GTK4 >= 4.16.0
- libadwaita >= 1.6.0
- libshumate >= 1.3.0
- GStreamer >= 1.16.0
- FFmpeg >= 4.0.0
- SQLite >= 3.20.0
- Rust toolchain

### Supported Architectures
- x86_64 (Intel/AMD)
- aarch64 (ARM64)

### Supported Platforms
- Linux distributions
- Linux phones (PinePhone, Librem 5, etc.)
- Steam Deck

## Installation Methods

### AUR (Arch Linux)
```bash
yay -S shortwave-mpris-git
```

### Flatpak
```bash
flatpak install https://flathub.org/repo/appstream/de.haeckerfelix.Shortwave.flatpakref
```

### Source Build
```bash
git clone https://gitlab.gnome.org/World/Shortwave.git
cd Shortwave
meson --prefix=/usr build
ninja -C build
sudo ninja -C build install
```

## Configuration

### Environment Variables
- `RUST_LOG=shortwave=debug` - Enable debug logging
- `RUST_BACKTRACE=1` - Enable backtrace on errors

### GSettings Keys
- Background playback toggle
- Notification preferences
- Audio device selection
- Network settings

## Troubleshooting

### Debug Information
Run with debug logging:
```bash
RUST_LOG=shortwave=debug RUST_BACKTRACE=1 shortwave
```

### Common Issues
1. **DLNA devices not showing**: Check network firewall settings
2. **Cast proxy not working**: Ensure port 8080 is open
3. **Playback failures**: Check GStreamer plugin installation
4. **MPRIS not working**: Verify D-Bus session is running

## Development

### Building for Development
```bash
cargo build --bin shortwave
```

### Building Release
```bash
cargo build --release
sudo meson install -C build
```

### Code Structure
- `src/audio/` - Audio playback and MPRIS implementation
- `src/device/` - DLNA and Cast device support
- `src/database/` - Library and station management
- `src/ui/` - User interface components
- `src/api/` - Radio station database integration

## License
GPL3 - See COPYING.md for details

## Contributing
Contributions welcome! Please see CODE_OF_CONDUCT.md for guidelines.

## Support
- Matrix: #shortwave:gnome.org
- Issues: https://gitlab.gnome.org/World/Shortwave/-/issues
