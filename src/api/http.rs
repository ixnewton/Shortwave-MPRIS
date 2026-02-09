// Shortwave - http.rs
// Copyright (C) 2021-2023  Felix Häcker <haeckerfelix@gnome.org>
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

use mpsc::UnboundedSender;
use oneshot::Sender;
use reqwest::{ClientBuilder, Method, Request, Response};
use std::sync::LazyLock;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::sync::{mpsc, oneshot};
use url::Url;

use crate::config;

type RequestSender = UnboundedSender<(Request, Sender<Result<Response, reqwest::Error>>)>;

static USER_AGENT: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{}/{}-{}",
        config::PKGNAME,
        config::VERSION,
        config::PROFILE
    )
});

static HTTP_THREAD: LazyLock<RequestSender> = LazyLock::new(|| {
    let (tx, mut rx): (RequestSender, _) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let rt = Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();

        let client = ClientBuilder::new()
            .user_agent(USER_AGENT.as_str())
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();

        rt.block_on(async {
            while let Some((request, response_tx)) = rx.recv().await {
                let client = client.clone();
                tokio::spawn(async move {
                    let result = client.execute(request).await;
                    let _ = response_tx.send(result);
                });
            }
        });
    });

    tx
});

pub async fn send(request: Request) -> Result<Response, reqwest::Error> {
    let (tx, rx) = oneshot::channel();
    HTTP_THREAD.send((request, tx)).unwrap();
    rx.await.unwrap()
}

pub async fn get(url: Url) -> Result<Response, reqwest::Error> {
    let request = Request::new(Method::GET, url);
    let (tx, rx) = oneshot::channel();
    HTTP_THREAD.send((request, tx)).unwrap();
    rx.await.unwrap()
}
