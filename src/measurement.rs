//! Drone measurement payload validation and CosmWasm mapping.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

/// Incoming drone reading from an x402-paying agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DroneMeasurement {
    /// Latitude as decimal string (CosmWasm JSON rejects bare floats).
    pub lat: String,
    /// Longitude as decimal string.
    pub lon: String,
    /// Turbidity reading (NTU by default).
    pub turbidity: String,
    /// Content hash of captured imagery (e.g. sha256 hex).
    pub image_hash: String,
    /// Unique flight or agent run identifier.
    pub flight_id: String,
    /// Optional existing citizen-science sensor id on Osmosis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor_id: Option<u64>,
    /// Measurement unit label (defaults to NTU).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Optional human-readable site label (India demo rivers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
}

/// Validation failures for [`DroneMeasurement`].
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum MeasurementError {
    #[error("lat must be a decimal between -90 and 90")]
    InvalidLat,
    #[error("lon must be a decimal between -180 and 180")]
    InvalidLon,
    #[error("turbidity must be a non-negative decimal")]
    InvalidTurbidity,
    #[error("image_hash must not be empty")]
    MissingImageHash,
    #[error("flight_id must not be empty")]
    MissingFlightId,
}

impl DroneMeasurement {
    /// Validates coordinate and reading fields.
    ///
    /// # Errors
    /// Returns [`MeasurementError`] when any field fails policy checks.
    pub fn validate(&self) -> Result<(), MeasurementError> {
        if self.flight_id.trim().is_empty() {
            return Err(MeasurementError::MissingFlightId);
        }
        if self.image_hash.trim().is_empty() {
            return Err(MeasurementError::MissingImageHash);
        }
        parse_lat(&self.lat)?;
        parse_lon(&self.lon)?;
        parse_non_negative(&self.turbidity, MeasurementError::InvalidTurbidity)?;
        Ok(())
    }

    /// Maps the drone reading to citizen-science `submit_data` JSON payload.
    pub fn to_chain_data(&self) -> Value {
        let unit = self.unit.clone().unwrap_or_else(|| "NTU".into());
        let mut obj = Map::new();
        obj.insert("source".into(), Value::String("drone_agent".into()));
        obj.insert("flight_id".into(), Value::String(self.flight_id.clone()));
        obj.insert("lat".into(), Value::String(self.lat.clone()));
        obj.insert("lon".into(), Value::String(self.lon.clone()));
        obj.insert("value".into(), Value::String(self.turbidity.clone()));
        obj.insert("unit".into(), Value::String(unit));
        obj.insert("image_hash".into(), Value::String(self.image_hash.clone()));
        if let Some(site) = &self.site {
            obj.insert("site".into(), Value::String(site.clone()));
        }
        Value::Object(obj)
    }
}

fn parse_lat(raw: &str) -> Result<(), MeasurementError> {
    let v: f64 = raw
        .trim()
        .parse()
        .map_err(|_| MeasurementError::InvalidLat)?;
    if !(-90.0..=90.0).contains(&v) {
        return Err(MeasurementError::InvalidLat);
    }
    Ok(())
}

fn parse_lon(raw: &str) -> Result<(), MeasurementError> {
    let v: f64 = raw
        .trim()
        .parse()
        .map_err(|_| MeasurementError::InvalidLon)?;
    if !(-180.0..=180.0).contains(&v) {
        return Err(MeasurementError::InvalidLon);
    }
    Ok(())
}

fn parse_non_negative(raw: &str, err: MeasurementError) -> Result<(), MeasurementError> {
    let v: f64 = raw.trim().parse().map_err(|_| err.clone())?;
    if v < 0.0 {
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DroneMeasurement {
        DroneMeasurement {
            lat: "28.70".into(),
            lon: "77.22".into(),
            turbidity: "14.2".into(),
            image_hash: "sha256:abc123".into(),
            flight_id: "yamuna-drone-001".into(),
            sensor_id: Some(1),
            unit: None,
            site: Some("Yamuna Wazirabad, Delhi NCR, India".into()),
        }
    }

    #[test]
    fn valid_measurement_passes() {
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn rejects_empty_flight_id() {
        let mut m = sample();
        m.flight_id = "  ".into();
        assert_eq!(m.validate(), Err(MeasurementError::MissingFlightId));
    }

    #[test]
    fn rejects_out_of_range_lat() {
        let mut m = sample();
        m.lat = "91".into();
        assert_eq!(m.validate(), Err(MeasurementError::InvalidLat));
    }

    #[test]
    fn chain_payload_uses_string_numbers() {
        let data = sample().to_chain_data();
        assert_eq!(data["source"], "drone_agent");
        assert_eq!(data["value"], "14.2");
        assert_eq!(data["unit"], "NTU");
        assert_eq!(data["site"], "Yamuna Wazirabad, Delhi NCR, India");
    }
}
