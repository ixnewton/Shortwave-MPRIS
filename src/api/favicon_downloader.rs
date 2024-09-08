// Shortwave - favicon_downloader.rs
// Copyright (C) 2021-2024  Felix Häcker <haeckerfelix@gnome.org>
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

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use futures_lite::AsyncReadExt;
use gdk::{Paintable, Texture};
use gio::prelude::*;
use gtk::{gdk, gio, glib};
use url::Url;

use crate::api::client::HTTP_CLIENT;
use crate::api::Error;
use crate::path;

pub struct FaviconDownloader {}

impl FaviconDownloader {
    pub async fn download(url: Url) -> Result<Paintable, Error> {
        if let Some(texture) = Self::cached_texture(&url).await {
            return Ok(texture.upcast());
        } else {
            debug!("No cached favicon available for {:?}", url.as_str());
        }

        // We currently don't support "data:image/png" urls
        if url.scheme() == "data" {
            debug!("Unsupported favicon type for {:?}", url);
            return Err(Error::UnsupportedUrlScheme);
        }

        // Download favicon
        let mut bytes = vec![];
        HTTP_CLIENT
            .get_async(url.as_str())
            .await?
            .into_body()
            .read_to_end(&mut bytes)
            .await
            .map_err(Rc::new)?;

        let texture = Texture::from_bytes(&glib::Bytes::from(&bytes))?;

        // Write downloaded bytes into file
        let file = Self::file(&url)?;
        file.replace_contents_future(bytes, None, false, gio::FileCreateFlags::NONE)
            .await
            .expect("Could not write favicon data");

        Ok(texture.upcast())
    }

    async fn cached_texture(url: &Url) -> Option<Texture> {
        let file = Self::file(url).ok()?;
        Texture::from_file(&file).ok().and_upcast()
    }

    pub fn file(url: &Url) -> Result<gio::File, Error> {
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();

        let mut path = path::CACHE.clone();
        path.push("favicons");
        std::fs::create_dir_all(path.as_path()).map_err(Rc::new)?;

        path.push(hash.to_string());

        Ok(gio::File::for_path(&path))
    }
}
