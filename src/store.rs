//! In-memory measurement mirror until G1 persists relay tx hashes.

use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::measurement::DroneMeasurement;
use crate::relay::{RelayOutcome, RelayStatus};

/// Stored measurement row returned by GET /v1/measurements/:id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMeasurement {
    pub id: String,
    pub measurement: DroneMeasurement,
    pub relay: RelayOutcome,
    pub created_at: String,
}

/// Thread-safe in-memory store for demo and G0 acceptance tests.
#[derive(Debug, Default)]
pub struct MeasurementStore {
    inner: RwLock<HashMap<String, StoredMeasurement>>,
}

impl MeasurementStore {
    /// Inserts a measurement record.
    pub fn insert(&self, record: StoredMeasurement) {
        if let Ok(mut map) = self.inner.write() {
            map.insert(record.id.clone(), record);
        }
    }

    /// Returns a clone of one record by id.
    pub fn get(&self, id: &str) -> Option<StoredMeasurement> {
        self.inner.read().ok().and_then(|map| map.get(id).cloned())
    }

    /// Count of stored rows (health metrics).
    pub fn len(&self) -> usize {
        self.inner.read().map(|m| m.len()).unwrap_or(0)
    }

    /// True when no measurements are stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl RelayOutcome {
    /// Serializes relay status for JSON APIs.
    pub fn to_api(&self) -> serde_json::Value {
        serde_json::json!({
            "status": match self.status {
                RelayStatus::Queued => "queued",
                RelayStatus::Submitted => "submitted",
                RelayStatus::NotConfigured => "not_configured",
                RelayStatus::Failed => "failed",
            },
            "txHash": self.tx_hash,
            "message": self.message,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::RelayStatus;

    #[test]
    fn store_round_trip() {
        let store = MeasurementStore::default();
        let record = StoredMeasurement {
            id: "abc".into(),
            measurement: DroneMeasurement {
                lat: "1".into(),
                lon: "1".into(),
                turbidity: "1".into(),
                image_hash: "h".into(),
                flight_id: "f".into(),
                sensor_id: None,
                unit: None,
                site: None,
            },
            relay: RelayOutcome {
                status: RelayStatus::NotConfigured,
                tx_hash: None,
                message: "queued".into(),
            },
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        store.insert(record);
        assert_eq!(store.len(), 1);
        assert!(store.get("abc").is_some());
    }
}
