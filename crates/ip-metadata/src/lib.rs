use std::{collections::HashMap, net::IpAddr};

use maxminddb::geoip2::{self};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IpMetadata {
    geo: Option<GeoMetadata>,
    asn: Option<AsnMetadata>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoMetadata {
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub accuracy: Accuracy,
    pub city_name: Option<String>,
    pub country_iso_code: String,
    pub country_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub enum Accuracy {
    High,
    #[default]
    Low,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AsnMetadata {
    pub number: u32,
    pub organization: String,
}

pub struct Resolver<R>
where
    R: AsRef<[u8]>,
{
    asn_reader: maxminddb::Reader<R>,
    geo_reader: maxminddb::Reader<R>,
    iso_lookup: HashMap<String, GeoMetadata>,
}

impl<R> Resolver<R>
where
    R: AsRef<[u8]>,
{
    pub fn new(
        asn_reader: maxminddb::Reader<R>,
        geo_reader: maxminddb::Reader<R>,
        iso_lookup: HashMap<String, GeoMetadata>,
    ) -> Self {
        Self {
            asn_reader,
            geo_reader,
            iso_lookup,
        }
    }

    pub fn lookup(&self, addr: IpAddr) -> IpMetadata {
        IpMetadata {
            geo: self.lookup_geo(addr),
            asn: self.lookup_asn(addr),
        }
    }

    fn lookup_geo(&self, addr: IpAddr) -> Option<GeoMetadata> {
        let record = self
            .geo_reader
            .lookup(addr)
            .ok()?
            .decode::<geoip2::City>()
            .ok()??;

        if let (Some(lat), Some(lon), Some(city_name), Some(country_name), Some(country_iso_code)) = (
            record.location.latitude,
            record.location.longitude,
            record.city.names.english,
            record.country.names.english,
            record.country.iso_code,
        ) {
            return Some(GeoMetadata {
                lat,
                lon,
                accuracy: Accuracy::High,
                city_name: Some(city_name.into()),
                country_iso_code: country_iso_code.into(),
                country_name: country_name.into(),
            });
        }

        if let Some(registered_country_iso_code) = record.registered_country.iso_code {
            return self.iso_lookup.get(registered_country_iso_code).cloned();
        }

        None
    }

    fn lookup_asn(&self, addr: IpAddr) -> Option<AsnMetadata> {
        let record = self
            .asn_reader
            .lookup(addr)
            .ok()?
            .decode::<geoip2::Asn>()
            .ok()??;

        if let (Some(number), Some(organization)) = (
            record.autonomous_system_number,
            record.autonomous_system_organization,
        ) {
            return Some(AsnMetadata {
                number,
                organization: organization.into(),
            });
        }

        None
    }
}
