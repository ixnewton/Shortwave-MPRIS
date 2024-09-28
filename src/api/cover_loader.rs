// Shortwave - cover_loader.rs
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

use anyhow::Result;
use async_channel::Sender;
use futures_util::StreamExt;
use gdk::RGBA;
use glycin::Loader;
use gtk::gio::{Cancelled, File};
use gtk::graphene::Rect;
use gtk::prelude::TextureExt;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, gsk};

use crate::api::SwStation;

#[derive(Debug, Clone)]
struct CoverRequest {
    station: SwStation,
    size: i32,
    sender: Sender<Result<Option<gdk::Texture>>>,
    cancellable: gio::Cancellable,
}

#[derive(Debug, Clone)]
pub struct CoverLoader {
    request_sender: Sender<CoverRequest>,
}

impl CoverLoader {
    pub fn new() -> Self {
        let (request_sender, request_receiver) = async_channel::unbounded::<CoverRequest>();
        let request_stream = request_receiver
            .map(Self::handle_request)
            .buffer_unordered(usize::max(glib::num_processors() as usize / 2, 2));

        glib::spawn_future_local(async move {
            request_stream.collect::<Vec<_>>().await;
        });

        Self { request_sender }
    }

    pub async fn load_cover(
        &mut self,
        station: &SwStation,
        size: i32,
        cancellable: gio::Cancellable,
    ) -> Result<Option<gdk::Texture>> {
        let (sender, receiver) = async_channel::bounded(1);

        let request = CoverRequest {
            station: station.clone(),
            size,
            sender,
            cancellable,
        };
        self.request_sender
            .send(request)
            .await
            .map_err(|_| anyhow::Error::msg("Unable to send cover request"))?;

        receiver.recv().await?
    }

    async fn handle_request(request: CoverRequest) {
        if request.cancellable.is_cancelled() {
            request.sender.send(Ok(None)).await.unwrap();
            return;
        }

        let res = gio::CancellableFuture::new(
            Self::cover_texture(&request.station, request.size),
            request.cancellable.clone(),
        )
        .await;

        let msg = match res {
            Ok(texture) => texture,
            Err(Cancelled) => Ok(None),
        };

        request.sender.send(msg).await.unwrap();
    }

    async fn cover_texture(station: &SwStation, size: i32) -> Result<Option<gdk::Texture>> {
        if let Some(favicon_url) = station.metadata().favicon {
            let file = File::for_uri(favicon_url.as_str());

            let loader = Loader::new(file);
            let image = loader.load().await?;
            let cover = image.next_frame().await?.texture();

            let snapshot = gtk::Snapshot::new();
            Self::snapshot_thumbnail(&snapshot, cover, size as f32);

            let node = snapshot.to_node().unwrap();
            let renderer = gsk::CairoRenderer::new();
            let display = gdk::Display::default().expect("Unable to get default display");
            renderer.realize_for_display(&display)?;

            let texture =
                renderer.render_texture(node, Some(&Rect::new(0.0, 0.0, size as f32, size as f32)));
            renderer.unrealize();

            Ok(Some(texture))
        } else {
            Err(anyhow::Error::msg("No cover available"))
        }
    }

    fn snapshot_thumbnail(snapshot: &gtk::Snapshot, cover: gdk::Texture, size: f32) {
        let aspect_ratio = cover.width() as f32 / cover.height() as f32;
        let mut width = size;
        let mut height = size;

        if aspect_ratio < 1.0 {
            width = aspect_ratio * size;
        } else {
            height = size / aspect_ratio;
        }

        if width >= size - 2.0 {
            width = size;
        }

        if height >= size - 2.0 {
            height = size;
        }

        let cover_rect = Rect::new((size - width) / 2.0, (size - height) / 2.0, width, height);

        snapshot.push_clip(&Rect::new(0.0, 0.0, size, size));

        snapshot.append_color(
            &RGBA::new(0.0, 0.0, 0.0, 1.0),
            &Rect::new(0.0, 0.0, size, size),
        );

        if width < size || height < size {
            let blur_radio = size / 4.0;

            let outer_rect_width;
            let outer_rect_height;
            if aspect_ratio < 1.0 {
                outer_rect_width = size + blur_radio * 2.0;
                outer_rect_height = outer_rect_width / aspect_ratio;
            } else {
                outer_rect_height = size + blur_radio * 2.0;
                outer_rect_width = aspect_ratio * outer_rect_height;
            }

            let outer_rect = Rect::new(
                (size - outer_rect_width) / 2.0,
                (size - outer_rect_height) / 2.0,
                outer_rect_width,
                outer_rect_height,
            );

            snapshot.push_blur(blur_radio as f64);
            snapshot.append_texture(&cover, &outer_rect);
            snapshot.pop();
            snapshot.append_color(&RGBA::new(0.0, 0.0, 0.0, 0.2), &outer_rect);
        }

        snapshot.append_scaled_texture(&cover, gsk::ScalingFilter::Trilinear, &cover_rect);
        snapshot.pop();
    }
}

impl Default for CoverLoader {
    fn default() -> Self {
        Self::new()
    }
}
