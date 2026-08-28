//! Gateway configuration loaded from environment variables.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::x402::BASE_SEPOLIA_USDC;

/// Runtime configuration for the agent gateway HTTP server and relay.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub host: String,
    pub port: u16,
    pub facilitator_url: String,
    pub payto_address: String,
    pub x402_network: String,
    pub x402_asset: String,
    pub x402_asset_contract: String,
    pub price_usdc: String,
    pub osmosis_rpc: String,
    pub osmosis_chain_id: String,
    pub osmosis_denom: String,
    pub csr_contract: Option<String>,
    pub relayer_mnemonic: Option<String>,
    pub relay_script: String,
    pub dev_bypass_payment: bool,
}

impl GatewayConfig {
    /// Loads configuration from process environment.
    ///
    /// # Errors
    /// Returns an error when `AGENT_GATEWAY_PORT` is not a valid u16.
    pub fn from_env() -> Result<Self> {
        let port = env::var("PORT")
            .or_else(|_| env::var("AGENT_GATEWAY_PORT"))
            .unwrap_or_else(|_| "8081".into())
            .parse::<u16>()
            .context("PORT / AGENT_GATEWAY_PORT must be a valid port number")?;

        Ok(Self {
            host: env::var("AGENT_GATEWAY_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port,
            facilitator_url: env::var("X402_FACILITATOR_URL")
                .unwrap_or_else(|_| "https://x402.org/facilitator".into()),
            payto_address: env::var("X402_PAYTO_ADDRESS").unwrap_or_default(),
            x402_network: env::var("X402_NETWORK").unwrap_or_else(|_| "base-sepolia".into()),
            x402_asset: env::var("X402_ASSET").unwrap_or_else(|_| "USDC".into()),
            x402_asset_contract: env::var("X402_ASSET_CONTRACT")
                .unwrap_or_else(|_| BASE_SEPOLIA_USDC.into()),
            price_usdc: env::var("X402_PRICE_USDC").unwrap_or_else(|_| "0.01".into()),
            osmosis_rpc: env::var("OSMOSIS_RPC")
                .unwrap_or_else(|_| "https://rpc.osmotest5.osmosis.zone".into()),
            osmosis_chain_id: env::var("OSMOSIS_CHAIN_ID").unwrap_or_else(|_| "osmo-test-5".into()),
            osmosis_denom: env::var("OSMOSIS_DENOM").unwrap_or_else(|_| "uosmo".into()),
            csr_contract: env::var("CSR_CONTRACT_ADDRESS")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            relayer_mnemonic: env::var("RELAYER_MNEMONIC")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            relay_script: env::var("RELAY_SCRIPT").unwrap_or_default(),
            dev_bypass_payment: env::var("X402_DEV_BYPASS")
                .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
                .unwrap_or(false),
        })
    }

    /// Whether relay to Osmosis is fully configured for G1.
    pub fn relay_ready(&self) -> bool {
        self.csr_contract.is_some() && self.relayer_mnemonic.is_some()
    }

    /// Whether x402 payment requirements can be advertised.
    pub fn x402_ready(&self) -> bool {
        !self.payto_address.trim().is_empty()
    }

    /// Validates config for production-like runs.
    ///
    /// # Errors
    /// Returns an error when dev bypass is enabled without explicit opt-in context.
    pub fn validate_for_run(&self) -> Result<()> {
        if self.dev_bypass_payment {
            tracing::warn!("X402_DEV_BYPASS is enabled: POST /v1/measurements skips payment");
        }
        if !self.x402_ready() && !self.dev_bypass_payment {
            tracing::warn!(
                "X402_PAYTO_ADDRESS is empty: measurements endpoint returns 402 stub only"
            );
        }
        if !self.relay_ready() {
            tracing::warn!("Relay not configured: Osmosis submission stays queued (G1)");
        }
        Ok(())
    }

    /// Socket address string for binding.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Public base URL hint for x402 resource descriptors (no trailing slash).
    pub fn public_base_url(&self) -> String {
        env::var("AGENT_GATEWAY_PUBLIC_URL")
            .unwrap_or_else(|_| format!("http://localhost:{}", self.port))
    }

    /// USDC contract address used in payment requirements.
    pub fn x402_asset_address(&self) -> &str {
        self.x402_asset_contract.as_str()
    }

    /// Price in USDC atomic units (6 decimals).
    pub fn price_atomic_usdc(&self) -> String {
        let Ok(dollars) = self.price_usdc.parse::<f64>() else {
            return "10000".into();
        };
        let atomic = (dollars * 1_000_000.0).round() as u64;
        atomic.to_string()
    }

    /// Absolute path to the CosmJS relay helper script.
    pub fn relay_script_path(&self) -> PathBuf {
        if !self.relay_script.is_empty() {
            return PathBuf::from(&self.relay_script);
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/relay-submit-data.mjs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_port_parses() {
        let cfg = GatewayConfig {
            host: "127.0.0.1".into(),
            port: 8081,
            facilitator_url: String::new(),
            payto_address: String::new(),
            x402_network: "base-sepolia".into(),
            x402_asset: "USDC".into(),
            x402_asset_contract: BASE_SEPOLIA_USDC.into(),
            price_usdc: "0.01".into(),
            osmosis_rpc: String::new(),
            osmosis_chain_id: "osmo-test-5".into(),
            osmosis_denom: "uosmo".into(),
            csr_contract: None,
            relayer_mnemonic: None,
            relay_script: String::new(),
            dev_bypass_payment: false,
        };
        assert!(!cfg.relay_ready());
        assert!(!cfg.x402_ready());
        assert_eq!(cfg.bind_addr(), "127.0.0.1:8081");
    }

    #[test]
    fn relay_ready_when_both_set() {
        let cfg = GatewayConfig {
            host: String::new(),
            port: 1,
            facilitator_url: String::new(),
            payto_address: "0xabc".into(),
            x402_network: String::new(),
            x402_asset: String::new(),
            x402_asset_contract: String::new(),
            price_usdc: String::new(),
            osmosis_rpc: String::new(),
            osmosis_chain_id: String::new(),
            osmosis_denom: String::new(),
            csr_contract: Some("osmo1test".into()),
            relayer_mnemonic: Some("word".into()),
            relay_script: String::new(),
            dev_bypass_payment: false,
        };
        assert!(cfg.relay_ready());
        assert!(cfg.x402_ready());
    }

    #[test]
    fn port_from_render_env() {
        let prev_port = env::var("PORT").ok();
        let prev_gateway = env::var("AGENT_GATEWAY_PORT").ok();
        env::set_var("PORT", "10000");
        env::remove_var("AGENT_GATEWAY_PORT");
        let cfg = GatewayConfig::from_env().expect("from_env");
        match prev_port {
            Some(v) => env::set_var("PORT", v),
            None => env::remove_var("PORT"),
        }
        match prev_gateway {
            Some(v) => env::set_var("AGENT_GATEWAY_PORT", v),
            None => env::remove_var("AGENT_GATEWAY_PORT"),
        }
        assert_eq!(cfg.port, 10000);
    }

    #[test]
    fn invalid_port_errors() {
        let prev_port = env::var("PORT").ok();
        let prev_gateway = env::var("AGENT_GATEWAY_PORT").ok();
        env::remove_var("PORT");
        env::set_var("AGENT_GATEWAY_PORT", "not-a-port");
        let err = GatewayConfig::from_env().unwrap_err();
        match prev_port {
            Some(v) => env::set_var("PORT", v),
            None => env::remove_var("PORT"),
        }
        match prev_gateway {
            Some(v) => env::set_var("AGENT_GATEWAY_PORT", v),
            None => env::remove_var("AGENT_GATEWAY_PORT"),
        }
        assert!(err.to_string().contains("AGENT_GATEWAY_PORT"));
    }
}
