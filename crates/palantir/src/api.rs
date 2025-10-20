use std::{net::IpAddr, time::Duration};

use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IpInfo {
    pub country: String,
    #[serde(alias = "regionName")]
    pub region: String,
    pub city: String,
    pub lat: f32,
    pub lon: f32,
    pub isp: String,
    pub mobile: bool,
    pub proxy: bool,
    pub hosting: bool,
}

pub struct IpInfoClient {
    client: Client,
}

impl IpInfoClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }

    pub async fn query(&self, addr: IpAddr) -> Option<IpInfo> {
        self.client
            .get(format!("http://ip-api.com/json/{}?fields=16990937", addr))
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()
    }

    pub async fn query_self(&self) -> Option<IpInfo> {
        self.client
            .get("http://ip-api.com/json?fields=16990937")
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()
    }
}
