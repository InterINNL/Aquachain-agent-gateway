//! Osmosis relay to citizen-science-registry via CosmJS helper script.

use std::process::Stdio;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::GatewayConfig;
use crate::measurement::DroneMeasurement;

/// Result of a relay attempt to CosmWasm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayOutcome {
    pub status: RelayStatus,
    pub tx_hash: Option<String>,
    pub message: String,
}

/// Relay lifecycle for G0/G1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayStatus {
    Queued,
    Submitted,
    NotConfigured,
    Failed,
}

#[derive(Debug, Deserialize)]
struct RelayScriptOk {
    ok: bool,
    tx_hash: Option<String>,
    error: Option<String>,
}

/// Builds the CosmWasm execute message for `submit_data`.
pub fn build_submit_data_msg(sensor_id: u64, data: Value) -> Value {
    serde_json::json!({
        "submit_data": {
            "sensor_id": sensor_id,
            "data": data
        }
    })
}

/// Signs and broadcasts `submit_data` through the Node CosmJS relay script.
///
/// # Errors
/// Returns an error when relay is configured but the script exits with failure.
pub async fn relay_measurement(
    config: &GatewayConfig,
    measurement: &DroneMeasurement,
    measurement_id: &str,
) -> Result<RelayOutcome> {
    if !config.relay_ready() {
        return Ok(RelayOutcome {
            status: RelayStatus::NotConfigured,
            tx_hash: None,
            message: format!(
                "Measurement {measurement_id} validated; configure RELAYER_MNEMONIC and CSR_CONTRACT_ADDRESS for Osmosis relay"
            ),
        });
    }

    let sensor_id = measurement.sensor_id.unwrap_or(1);
    let chain_data = measurement.to_chain_data();
    let mnemonic = config
        .relayer_mnemonic
        .as_ref()
        .context("relayer mnemonic missing")?;
    let contract = config
        .csr_contract
        .as_ref()
        .context("CSR contract missing")?;

    let stdin_payload = serde_json::json!({
        "rpc": config.osmosis_rpc,
        "denom": config.osmosis_denom,
        "contract": contract,
        "mnemonic": mnemonic,
        "sensor_id": sensor_id,
        "data": chain_data,
    });

    let script = config.relay_script_path();
    if !script.exists() {
        bail!("relay script not found at {}", script.display());
    }

    let mut child = Command::new("node")
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn relay script {}", script.display()))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(stdin_payload.to_string().as_bytes())
            .await
            .context("write relay stdin")?;
    }

    let output = child
        .wait_with_output()
        .await
        .context("wait for relay script")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: RelayScriptOk = serde_json::from_str(stdout.trim()).unwrap_or(RelayScriptOk {
        ok: false,
        tx_hash: None,
        error: Some(format!(
            "invalid relay output (exit {}): {}",
            output.status.code().unwrap_or(1),
            stdout.trim()
        )),
    });

    if parsed.ok {
        return Ok(RelayOutcome {
            status: RelayStatus::Submitted,
            tx_hash: parsed.tx_hash,
            message: format!("submit_data broadcast for measurement {measurement_id}"),
        });
    }

    Ok(RelayOutcome {
        status: RelayStatus::Failed,
        tx_hash: None,
        message: parsed.error.unwrap_or_else(|| "relay script failed".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measurement::DroneMeasurement;

    #[test]
    fn submit_data_msg_shape() {
        let m = DroneMeasurement {
            lat: "28.70".into(),
            lon: "77.22".into(),
            turbidity: "10".into(),
            image_hash: "abc".into(),
            flight_id: "f1".into(),
            sensor_id: Some(3),
            unit: None,
            site: None,
        };
        let msg = build_submit_data_msg(3, m.to_chain_data());
        assert_eq!(msg["submit_data"]["sensor_id"], 3);
        assert_eq!(msg["submit_data"]["data"]["source"], "drone_agent");
    }

    #[tokio::test]
    async fn relay_not_configured_queues() {
        let cfg = GatewayConfig {
            host: String::new(),
            port: 1,
            facilitator_url: String::new(),
            payto_address: String::new(),
            x402_network: String::new(),
            x402_asset: String::new(),
            x402_asset_contract: String::new(),
            price_usdc: String::new(),
            osmosis_rpc: String::new(),
            osmosis_chain_id: String::new(),
            osmosis_denom: String::new(),
            csr_contract: None,
            relayer_mnemonic: None,
            relay_script: String::new(),
            dev_bypass_payment: false,
        };
        let m = DroneMeasurement {
            lat: "1".into(),
            lon: "1".into(),
            turbidity: "1".into(),
            image_hash: "h".into(),
            flight_id: "f".into(),
            sensor_id: None,
            unit: None,
            site: None,
        };
        let out = relay_measurement(&cfg, &m, "id-1").await.unwrap();
        assert_eq!(out.status, RelayStatus::NotConfigured);
    }
}
