use std::{net::IpAddr, str::FromStr};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct IpInfo {
    pub lat: f64,
    pub lon: f64,
    pub country_code: String,

    #[serde(flatten)]
    pub details: LocationDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "source")]
pub enum LocationDetails {
    City {
        city_name: String,
        accuracy_radius: u16,
    },
    Manual,
    RegisteredCountry,
}

pub struct Resolver<R>
where
    R: AsRef<[u8]>,
{
    city_reader: maxminddb::Reader<R>,
}

impl<R> Resolver<R>
where
    R: AsRef<[u8]>,
{
    pub fn new(city_reader: maxminddb::Reader<R>) -> Self {
        Self { city_reader }
    }

    pub fn resolve(&self, addr: IpAddr) -> Option<IpInfo> {
        let city_data = self
            .city_reader
            .lookup::<maxminddb::geoip2::City>(addr)
            .ok()??;

        if let Some(location) = &city_data.location {
            if let (Some(lat), Some(lon), Some(accuracy_radius)) = (
                location.latitude,
                location.longitude,
                location.accuracy_radius,
            ) {
                let country_code = city_data
                    .country
                    .as_ref()
                    .and_then(|c| c.iso_code)
                    .map(|c| c.to_string())
                    .unwrap_or_default();

                let city_name = city_data
                    .city
                    .as_ref()
                    .and_then(|c| c.names.as_ref())
                    .and_then(|n| n.get("en"))
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                return Some(IpInfo {
                    lat,
                    lon,
                    country_code: country_code.clone(),
                    details: LocationDetails::City {
                        city_name,
                        accuracy_radius,
                    },
                });
            }
        }

        if let Some(registered_country) = &city_data.registered_country {
            if let Some(iso_code) = registered_country.iso_code {
                if let Ok(country) = my_country::Country::from_str(iso_code) {
                    let geo = country.geo();
                    if let (Some(lat), Some(lon)) = (geo.latitude, geo.longitude) {
                        return Some(IpInfo {
                            lat,
                            lon,
                            country_code: iso_code.to_string(),
                            details: LocationDetails::RegisteredCountry,
                        });
                    }
                }
            }
        }

        None
    }
}
