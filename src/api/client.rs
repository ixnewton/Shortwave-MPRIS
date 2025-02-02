// Shortwave - client.rs
// Copyright (C) 2021-2025  Felix Häcker <haeckerfelix@gnome.org>
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

use std::net::IpAddr;
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

use async_compat::Compat;
use async_std_resolver::{config as rconfig, resolver, resolver_from_system_conf};
use rand::prelude::SliceRandom;
use rand::thread_rng;
use reqwest::header::{self, HeaderMap};
use reqwest::Request;
use serde::de;
use url::Url;

use crate::api::*;
use crate::app::SwApplication;
use crate::config;
use crate::settings::{settings_manager, Key};

static USER_AGENT: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{}/{}-{}",
        config::PKGNAME,
        config::VERSION,
        config::PROFILE
    )
});

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(
        "content-type",
        header::HeaderValue::from_static("application/json"),
    );

    reqwest::ClientBuilder::new()
        .user_agent(USER_AGENT.as_str())
        .default_headers(headers)
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap()
});

pub async fn station_request(request: StationRequest) -> Result<Vec<SwStation>, Error> {
    let url = build_url(STATION_SEARCH, Some(&request.url_encode()))?;

    let request = HTTP_CLIENT.get(url.as_ref()).build().map_err(Rc::new)?;
    let stations_md = send_request_compat::<Vec<StationMetadata>>(request).await?;

    let stations: Vec<SwStation> = stations_md
        .into_iter()
        .map(|metadata| SwStation::new(&metadata.stationuuid.clone(), false, metadata, None))
        .collect();

    Ok(stations)
}

pub async fn station_metadata_by_uuid(uuids: Vec<String>) -> Result<Vec<StationMetadata>, Error> {
    let url = build_url(STATION_BY_UUID, None)?;

    let uuids = format!(
        r#"{{"uuids":{}}}"#,
        serde_json::to_string(&uuids).unwrap_or_default()
    );
    debug!("Post body: {}", uuids);

    let request = HTTP_CLIENT.post(url).body(uuids).build().map_err(Rc::new)?;
    send_request_compat(request).await
}

pub async fn lookup_rb_server() -> Option<String> {
    let lookup_domain = settings_manager::string(Key::ApiLookupDomain);
    let resolver = if let Ok(resolver) = resolver_from_system_conf().await {
        resolver
    } else {
        warn!("Unable to use dns resolver from system conf");

        let config = rconfig::ResolverConfig::default();
        let opts = rconfig::ResolverOpts::default();
        resolver(config, opts).await
    };

    // Do forward lookup to receive a list with the api servers
    let response = resolver.lookup_ip(lookup_domain).await.ok()?;
    let mut ips: Vec<IpAddr> = response.iter().collect();

    // Shuffle it to make sure we're not using always the same one
    ips.shuffle(&mut thread_rng());

    for ip in ips {
        // Do a reverse lookup to get the hostname
        let result = resolver
            .reverse_lookup(ip)
            .await
            .ok()
            .and_then(|r| r.into_iter().next());

        if result.is_none() {
            warn!("Reverse lookup for {} failed", ip);
            continue;
        }

        // We need to strip the trailing "." from the domain name, otherwise TLS hostname verification fails
        let domain = result.unwrap().to_string();
        let hostname = domain.trim_end_matches(".");

        // Check if the server is online / returns data
        // If not, try using the next one in the list
        debug!("Trying to connect to {} ({})", hostname, ip.to_string());
        match server_stats(&hostname).await {
            Ok(stats) => {
                debug!(
                    "Successfully connected to {} ({}), server version {}, {} stations",
                    hostname,
                    ip.to_string(),
                    stats.software_version,
                    stats.stations
                );
                return Some(format!("https://{hostname}/"));
            }
            Err(err) => warn!("Unable to connect to {hostname}: {}", err.to_string()),
        }
    }

    None
}

fn build_url(param: &str, options: Option<&str>) -> Result<Url, Error> {
    let rb_server = SwApplication::default().rb_server();
    if rb_server.is_none() {
        return Err(Error::NoServerAvailable);
    }

    let mut url = Url::parse(&rb_server.unwrap())
        .expect("Unable to parse server url")
        .join(param)
        .expect("Unable to join url");

    if let Some(options) = options {
        url.set_query(Some(options))
    }

    debug!("Retrieve data: {}", url);
    Ok(url)
}

async fn server_stats(host: &str) -> Result<Stats, Error> {
    let request = HTTP_CLIENT
        .get(format!("https://{host}/{STATS}"))
        .build()
        .map_err(Rc::new)?;

    send_request_compat(request).await
}

async fn send_request<T: de::DeserializeOwned>(request: Request) -> Result<T, Error> {
    let response = HTTP_CLIENT.execute(request).await.map_err(Rc::new)?;
    let json = response.text().await.map_err(Rc::new)?;
    let deserialized = serde_json::from_str(&json);

    match deserialized {
        Ok(d) => Ok(d),
        Err(err) => {
            error!("Unable to deserialize data: {}", err.to_string());
            error!("Raw unserialized data: {}", json);
            Err(Error::Deserializer(err.into()))
        }
    }
}

async fn send_request_compat<T: de::DeserializeOwned>(request: Request) -> Result<T, Error> {
    Compat::new(async move { send_request(request).await }).await
}
