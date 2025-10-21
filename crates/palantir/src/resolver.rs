use std::{net::IpAddr, str::FromStr};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct IpInfo {
    pub lat: f64,
    pub lon: f64,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize)]
pub enum Source {
    City,
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
        if let Ok(Some(city)) = self.city_reader.lookup::<maxminddb::geoip2::City>(addr) {
            if let Some(location) = city.location {
                if let (Some(lat), Some(lon)) = (location.latitude, location.longitude) {
                    return Some(IpInfo {
                        lat,
                        lon,
                        source: Source::City,
                    });
                }
            }
            if let Some(registered_country) = city.registered_country {
                if let Some(iso_code) = registered_country.iso_code {
                    if let Ok(country) = my_country::Country::from_str(iso_code) {
                        let geo = country.geo();
                        if let (Some(lat), Some(lon)) = (geo.latitude, geo.longitude) {
                            return Some(IpInfo {
                                lat,
                                lon,
                                source: Source::RegisteredCountry,
                            });
                        }
                    }
                }
            }
        }

        None
    }
}
