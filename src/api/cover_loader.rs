// Shortwave - cover_loader.rs
// Copyright (C) 2024-2025  Felix Häcker <haeckerfelix@gnome.org>
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

use async_compat::CompatExt;
use std::time::{Duration, SystemTime};

use anyhow::{Error, Result};
use async_channel::Sender;
use futures_util::StreamExt;
use gdk::RGBA;
use glycin::Loader;
use gtk::gio::{Cancelled, File};
use gtk::graphene::Rect;
use gtk::prelude::TextureExt;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, gsk};
use url::Url;

use crate::{config, path};
#[derive(Debug, Clone)]
struct CoverRequest {
    favicon_url: Url,
    size: i32,
    sender: Sender<Result<gdk::Texture>>,
    cancellable: gio::Cancellable,
    tmp_file: gio::File,
    tmp_stream: gio::FileIOStream,
}

impl CoverRequest {
    pub async fn handle_request(self) {
        let res = gio::CancellableFuture::new(self.cover_texture(), self.cancellable.clone()).await;
        let msg = match res {
            Ok(res) => res,
            Err(Cancelled) => Err(Error::msg("cancelled")),
        };

        let _ = self.delete_tmp_file().await;
        self.sender.send(msg).await.unwrap();
    }

    async fn cover_texture(&self) -> Result<gdk::Texture> {
        if let Ok(texture) = self.cached_texture().await {
            return Ok(texture);
        }

        self.compute_texture().await
    }

    async fn cached_texture(&self) -> Result<gdk::Texture> {
        let key = format!("{}@{}", self.favicon_url, self.size);
        let data = cacache::read(&*path::CACHE, key).await?;
        let bytes = glib::Bytes::from_owned(data);

        Ok(gdk::Texture::from_bytes(&bytes)?)
    }

    async fn compute_texture(&self) -> Result<gdk::Texture> {
        let (cover_texture, cover_bytes) = self.cover_bytes().await?;

        let key = format!("{}@{}", self.favicon_url, self.size);
        cacache::write(&*path::CACHE, key, &cover_bytes).await?;

        Ok(cover_texture)
    }

    async fn cover_bytes(&self) -> Result<(gdk::Texture, Vec<u8>)> {
        self.download_tmp_file().await?;

        let loader = Loader::new(self.tmp_file.clone());
        let image = loader.load().await?;
        let texture = image.next_frame().await?.texture();

        let snapshot = gtk::Snapshot::new();
        snapshot_thumbnail(&snapshot, texture, self.size as f32);

        let node = snapshot.to_node().unwrap();
        let renderer = gsk::CairoRenderer::new();
        let display = gdk::Display::default().expect("No default display available");
        renderer.realize_for_display(&display)?;

        let rect = Rect::new(0.0, 0.0, self.size as f32, self.size as f32);
        let texture = renderer.render_texture(node, Some(&rect));
        renderer.unrealize();

        let png_bytes = texture.save_to_png_bytes().to_vec();
        Ok((texture, png_bytes))
    }

    async fn download_tmp_file(&self) -> Result<()> {
        let response = reqwest::get(self.favicon_url.as_str()).compat().await?;
        let body_bytes = response.bytes().compat().await?;

        // We have to write the data to the disk in order to be able to load them using Glycin
        // TODO: Load bytes directly
        // TODO: https://gitlab.gnome.org/GNOME/glycin/-/issues/98
        let bytes = glib::Bytes::from_owned(body_bytes);
        self.tmp_stream
            .output_stream()
            .write_bytes_future(&bytes, glib::Priority::LOW)
            .await?;

        Ok(())
    }

    async fn delete_tmp_file(&self) -> Result<()> {
        self.tmp_stream.close_future(glib::Priority::LOW).await?;
        self.tmp_file.delete_future(glib::Priority::LOW).await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CoverLoader {
    request_sender: Sender<CoverRequest>,
}

impl CoverLoader {
    pub fn new() -> Self {
        let (request_sender, request_receiver) = async_channel::unbounded::<CoverRequest>();
        let request_stream = request_receiver
            .map(|r| r.handle_request())
            .buffer_unordered(usize::max(glib::num_processors() as usize / 2, 2));

        glib::spawn_future_local(async move {
            request_stream.collect::<Vec<_>>().await;
        });

        Self { request_sender }
    }

    pub async fn prune_cache(&self) {
        // Remove old Shortwave pre v4.0 cache
        let mut path = path::CACHE.clone();
        path.push("favicons");
        let _ = std::fs::remove_dir_all(&path);

        // Remove cached covers which are older > 30 days
        let ttl = Duration::from_secs(86400 * 30);
        for md in cacache::list_sync(&*path::CACHE).flatten() {
            let now = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let age = Duration::from_millis((now - md.time).try_into().unwrap_or_default());
            if age > ttl {
                let _ = cacache::remove_hash_sync(&*path::CACHE, &md.integrity);
                let _ = cacache::remove_sync(&*path::CACHE, &md.key);
            }
        }
    }

    pub async fn load_cover(
        &mut self,
        favicon_url: &Url,
        size: i32,
        cancellable: gio::Cancellable,
    ) -> Result<gdk::Texture> {
        let (sender, receiver) = async_channel::bounded(1);

        let (tmp_file, tmp_stream) = File::new_tmp_future(
            Some(&format!("{}-Cover-XXXXXX", config::NAME)),
            glib::Priority::LOW,
        )
        .await?;

        let request = CoverRequest {
            favicon_url: favicon_url.clone(),
            size,
            sender,
            cancellable: cancellable.clone(),
            tmp_file,
            tmp_stream,
        };
        self.request_sender
            .send(request)
            .await
            .map_err(|_| Error::msg("Unable to send cover request"))?;

        receiver.recv().await?
    }
}

impl Default for CoverLoader {
    fn default() -> Self {
        Self::new()
    }
}

// Ported from Highscore (Alice Mikhaylenko)
// https://gitlab.gnome.org/World/highscore/-/blob/b07460f0c1475269381902c6305e4d91e55b61f5/src/library/cover-loader.vala#L124
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
        &RGBA::new(0.96, 0.96, 0.96, 1.0),
        &Rect::new(0.0, 0.0, size, size),
    );

    if width < size || height < size {
        let blur_radius = size / 4.0;

        let outer_rect_width;
        let outer_rect_height;
        if aspect_ratio < 1.0 {
            outer_rect_width = size + blur_radius * 2.0;
            outer_rect_height = outer_rect_width / aspect_ratio;
        } else {
            outer_rect_height = size + blur_radius * 2.0;
            outer_rect_width = aspect_ratio * outer_rect_height;
        }

        let outer_rect = Rect::new(
            (size - outer_rect_width) / 2.0,
            (size - outer_rect_height) / 2.0,
            outer_rect_width,
            outer_rect_height,
        );

        snapshot.push_blur(blur_radius as f64);
        snapshot.append_texture(&cover, &outer_rect);
        snapshot.pop();
        snapshot.append_color(&RGBA::new(0.0, 0.0, 0.0, 0.2), &outer_rect);
    }

    snapshot.append_scaled_texture(&cover, gsk::ScalingFilter::Trilinear, &cover_rect);
    snapshot.pop();
}
