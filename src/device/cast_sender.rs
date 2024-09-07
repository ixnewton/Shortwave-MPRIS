// Shortwave - cast_sender.rs
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

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use cast_sender::namespace::media::*;
use cast_sender::{AppId, ImageBuilder, MediaController};
use glib::clone;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwCastSender)]
    pub struct SwCastSender {
        #[property(get)]
        pub stream_url: RefCell<String>,
        #[property(get)]
        pub cover_url: RefCell<String>,
        #[property(get)]
        pub title: RefCell<String>,
        #[property(get, set=Self::set_volume, type=f64)]
        pub volume: Cell<f64>,
        #[property(get)]
        pub is_connected: Cell<bool>,

        pub receiver: cast_sender::Receiver,
        pub app: RefCell<Option<cast_sender::App>>,
        pub media_controller: RefCell<Option<MediaController>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwCastSender {
        const NAME: &'static str = "SwCastSender";
        type Type = super::SwCastSender;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwCastSender {}

    impl SwCastSender {
        fn set_volume(&self, volume: f64) {
            self.volume.set(volume);

            glib::spawn_future_local(clone!(
                #[strong(rename_to = receiver)]
                self.receiver,
                #[strong]
                volume,
                async move {
                    receiver.set_volume(volume, false).await; // TODO: Error handling
                }
            ));
        }

        pub async fn load(&self) -> Result<(), cast_sender::Error> {
            if let Some(media_controller) = self.media_controller() {
                let volume = self.receiver.volume().await?;
                if let Some(level) = volume.level {
                    self.volume.set(level);
                    self.obj().notify_volume();
                }

                let metadata = MusicTrackMediaMetadataBuilder::default()
                    .title(self.obj().title())
                    .images(vec![ImageBuilder::default()
                        .url(self.obj().cover_url())
                        .build()
                        .unwrap()])
                    .build()
                    .unwrap()
                    .into();

                let media_info = MediaInformation {
                    content_id: self.obj().stream_url(),
                    stream_type: StreamType::Live,
                    content_type: "audio/*".into(),
                    metadata: Some(metadata),
                    ..Default::default()
                };

                media_controller.load(media_info).await?;
            }

            Ok(())
        }

        pub fn media_controller(&self) -> Option<MediaController> {
            self.media_controller.borrow().clone()
        }
    }
}

glib::wrapper! {
    pub struct SwCastSender(ObjectSubclass<imp::SwCastSender>);
}

impl SwCastSender {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub async fn connect(&self, ip: &str) -> Result<(), cast_sender::Error> {
        if self.is_connected() {
            self.disconnect().await;
        }
        let receiver = &self.imp().receiver;
        receiver.connect(ip).await?;

        let app = receiver
            .launch_app(AppId::Custom("E3F31F9F".into()))
            .await?;
        let media_controller = MediaController::new(app.clone(), receiver.clone())?;

        self.imp().app.borrow_mut().replace(app);
        self.imp()
            .media_controller
            .borrow_mut()
            .replace(media_controller);

        self.imp().is_connected.set(true);
        self.notify_is_connected();

        Ok(())
    }

    pub async fn disconnect(&self) {
        if !self.is_connected() {
            return;
        }

        let app = { self.imp().app.borrow_mut().take() };
        if let Some(app) = app {
            let _ = self.imp().receiver.stop_app(&app).await;
        }

        self.imp().receiver.disconnect().await;
        self.imp().app.borrow_mut().take();
        self.imp().media_controller.borrow_mut().take();

        self.imp().is_connected.set(false);
        self.notify_is_connected();
    }

    pub async fn load_media(
        &self,
        stream_url: &str,
        cover_url: &str,
        title: &str,
    ) -> Result<(), cast_sender::Error> {
        *self.imp().stream_url.borrow_mut() = stream_url.to_string();
        *self.imp().cover_url.borrow_mut() = cover_url.to_string();
        *self.imp().title.borrow_mut() = title.to_string();

        self.notify_stream_url();
        self.notify_cover_url();
        self.notify_title();

        self.imp().load().await?;
        Ok(())
    }

    pub async fn start_playback(&self) -> Result<(), cast_sender::Error> {
        if !self.is_connected() {
            return Ok(());
        }

        self.imp().load().await?;
        if let Some(media_controller) = self.imp().media_controller() {
            media_controller.start().await?;
        }

        Ok(())
    }

    pub async fn stop_playback(&self) -> Result<(), cast_sender::Error> {
        if !self.is_connected() {
            return Ok(());
        }

        if let Some(media_controller) = self.imp().media_controller() {
            media_controller.stop().await?;
        }

        Ok(())
    }
}

impl Default for SwCastSender {
    fn default() -> Self {
        Self::new()
    }
}
