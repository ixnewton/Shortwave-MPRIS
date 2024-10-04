// Shortwave - client.rs
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

use std::net::IpAddr;
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

use async_std_resolver::{config as rconfig, resolver, resolver_from_system_conf};
use isahc::config::RedirectPolicy;
use isahc::prelude::*;
use rand::prelude::SliceRandom;
use rand::thread_rng;
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

static HTTP_CLIENT: LazyLock<isahc::HttpClient> = LazyLock::new(|| {
    isahc::HttpClientBuilder::new()
        // Limit to reduce ram usage. We don't need 250 concurrent connections
        .max_connections(8)
        // Icons are fetched from different urls.
        // There's a lot of probability we aren't going to reuse the same connection
        .connection_cache_size(8)
        .timeout(Duration::from_secs(15))
        .redirect_policy(RedirectPolicy::Follow)
        .default_header("content-type", "application/json")
        .default_header("User-Agent", USER_AGENT.as_str())
        .build()
        .unwrap()
});

pub async fn station_request(request: StationRequest) -> Result<Vec<SwStation>, Error> {
    let url = build_url(STATION_SEARCH, Some(&request.url_encode()))?;

    let response = HTTP_CLIENT
        .get_async(url.as_ref())
        .await?
        .text()
        .await
        .map_err(Rc::new)?;
    let deserialized: Result<Vec<StationMetadata>, _> = serde_json::from_str(&response);

    let stations_md = match deserialized {
        Ok(deserialized) => deserialized,
        Err(err) => {
            error!("Unable to deserialize data: {}", err.to_string());
            error!("Raw unserialized data: {}", response);
            return Err(Error::Deserializer(err.into()));
        }
    };

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

    let response = HTTP_CLIENT
        .post_async(url.as_ref(), uuids)
        .await?
        .text()
        .await
        .map_err(Rc::new)?;
    let deserialized: Result<Vec<StationMetadata>, _> = serde_json::from_str(&response);

    match deserialized {
        Ok(deserialized) => Ok(deserialized),
        Err(err) => {
            error!("Unable to deserialize data: {}", err.to_string());
            error!("Raw unserialized data: {}", response);
            Err(Error::Deserializer(err.into()))
        }
    }
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
            warn!("Reverse lookup failed for {} failed", ip);
            continue;
        }
        let hostname = result.unwrap();

        // Check if the server is online / returns data
        // If not, try using the next one in the list
        debug!(
            "Trying to connect to {} ({})",
            hostname.to_string(),
            ip.to_string()
        );
        match test_rb_server(hostname.to_string()).await {
            Ok(_) => {
                debug!(
                    "Successfully connected to {} ({})",
                    hostname.to_string(),
                    ip.to_string()
                );
                return Some(format!("https://{hostname}/"));
            }
            Err(err) => {
                warn!(
                    "Unable to connect to {}: {}",
                    ip.to_string(),
                    err.to_string()
                );
            }
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

async fn test_rb_server(ip: String) -> Result<(), Error> {
    let _stats: Option<Stats> = HTTP_CLIENT
        .get_async(format!("https://{ip}/{STATS}"))
        .await?
        .json()
        .await
        .map_err(Rc::new)?;
    Ok(())
}
