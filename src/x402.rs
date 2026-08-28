//! x402 payment verify/settle via facilitator HTTP API.

use std::time::Duration;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::config::GatewayConfig;

/// Parsed payment header names accepted by x402 clients.
pub const HEADER_PAYMENT_SIGNATURE: &str = "payment-signature";
pub const HEADER_PAYMENT_LEGACY: &str = "x-payment";
pub const HEADER_PAYMENT_REQUIRED: &str = "payment-required";
pub const HEADER_PAYMENT_RESPONSE: &str = "payment-response";

/// Base Sepolia USDC (Circle testnet).
pub const BASE_SEPOLIA_USDC: &str = "0x036CbD53842c5426634e7929541eC2318f3dCF7e";

/// Minimal payment requirements returned on HTTP 402.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequiredBody {
    pub error: String,
    pub message: String,
    pub price: String,
    pub asset: String,
    pub network: String,
    pub payto: String,
    pub facilitator: String,
    pub resource: String,
}

/// Facilitator verify/settle context kept between verify and settle calls.
#[derive(Debug, Clone)]
pub struct PaymentSession {
    pub payment_header: String,
    pub requirements: Value,
}

/// x402 payment failures surfaced to HTTP handlers.
#[derive(Debug, Error)]
pub enum PaymentError {
    #[error("missing payment header")]
    MissingHeader,
    #[error("payment verification failed: {0}")]
    VerifyFailed(String),
    #[error("payment settlement failed: {0}")]
    SettleFailed(String),
}

/// Builds v1 payment requirements JSON for facilitator calls.
pub fn build_payment_requirements(config: &GatewayConfig, resource_path: &str) -> Value {
    let resource = format!(
        "{}{resource_path}",
        config.public_base_url().trim_end_matches('/')
    );
    json!({
        "scheme": "exact",
        "network": config.x402_network,
        "maxAmountRequired": config.price_atomic_usdc(),
        "payTo": config.payto_address,
        "asset": config.x402_asset_address(),
        "resource": resource,
        "maxTimeoutSeconds": 300
    })
}

/// Builds the JSON body and base64 `PAYMENT-REQUIRED` header value.
pub fn payment_required_response(
    config: &GatewayConfig,
    resource_path: &str,
) -> (PaymentRequiredBody, String) {
    let resource = format!(
        "{}{resource_path}",
        config.public_base_url().trim_end_matches('/')
    );
    let requirements = build_payment_requirements(config, resource_path);
    let body = PaymentRequiredBody {
        error: "Payment Required".into(),
        message: "This endpoint requires x402 USDC payment before relaying to Osmosis.".into(),
        price: format!("${}", config.price_usdc),
        asset: config.x402_asset.clone(),
        network: config.x402_network.clone(),
        payto: config.payto_address.clone(),
        facilitator: config.facilitator_url.clone(),
        resource: resource.clone(),
    };
    let header = STANDARD.encode(requirements.to_string());
    (body, header)
}

/// Returns true when a payment header is present.
pub fn has_payment_header(headers: &axum::http::HeaderMap) -> bool {
    payment_header_value(headers).is_some()
}

/// Extracts the raw payment header value from the request.
pub fn payment_header_value(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(HEADER_PAYMENT_SIGNATURE)
        .or_else(|| headers.get(HEADER_PAYMENT_LEGACY))
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// Verifies payment with the configured facilitator (no on-chain settle yet).
///
/// # Errors
/// Returns [`PaymentError`] when the facilitator rejects the payload.
pub async fn verify_payment(
    config: &GatewayConfig,
    payment_header: &str,
    resource_path: &str,
) -> Result<PaymentSession, PaymentError> {
    let requirements = build_payment_requirements(config, resource_path);
    let body = json!({
        "x402Version": 1,
        "paymentHeader": payment_header,
        "paymentRequirements": requirements
    });

    let url = facilitator_endpoint(config, "verify");
    let client = facilitator_client().map_err(|e| PaymentError::VerifyFailed(e.to_string()))?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("facilitator verify request failed")
        .map_err(|e| PaymentError::VerifyFailed(e.to_string()))?;

    let status = resp.status();
    let payload: Value = resp
        .json()
        .await
        .context("facilitator verify response parse failed")
        .map_err(|e| PaymentError::VerifyFailed(e.to_string()))?;

    if !status.is_success() {
        return Err(PaymentError::VerifyFailed(format!(
            "facilitator HTTP {status}: {payload}"
        )));
    }

    if payload
        .get("isValid")
        .and_then(Value::as_bool)
        .is_some_and(|v| !v)
    {
        let reason = payload
            .get("invalidReason")
            .or_else(|| payload.get("error"))
            .map(|v| v.to_string())
            .unwrap_or_else(|| "invalid payment".into());
        return Err(PaymentError::VerifyFailed(reason));
    }

    Ok(PaymentSession {
        payment_header: payment_header.to_string(),
        requirements,
    })
}

/// Settles a previously verified payment on-chain via facilitator.
///
/// # Errors
/// Returns [`PaymentError`] when settlement fails.
pub async fn settle_payment(
    config: &GatewayConfig,
    session: &PaymentSession,
) -> Result<Value, PaymentError> {
    let body = json!({
        "x402Version": 1,
        "paymentHeader": session.payment_header,
        "paymentRequirements": session.requirements
    });

    let url = facilitator_endpoint(config, "settle");
    let client = facilitator_client().map_err(|e| PaymentError::SettleFailed(e.to_string()))?;
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| PaymentError::SettleFailed(e.to_string()))?;

    let status = resp.status();
    let payload: Value = resp
        .json()
        .await
        .map_err(|e| PaymentError::SettleFailed(e.to_string()))?;

    if !status.is_success() {
        return Err(PaymentError::SettleFailed(format!(
            "facilitator HTTP {status}: {payload}"
        )));
    }

    if payload
        .get("success")
        .and_then(Value::as_bool)
        .is_some_and(|v| !v)
    {
        let reason = payload
            .get("errorReason")
            .or_else(|| payload.get("error"))
            .map(|v| v.to_string())
            .unwrap_or_else(|| "settlement failed".into());
        return Err(PaymentError::SettleFailed(reason));
    }

    Ok(payload)
}

fn facilitator_endpoint(config: &GatewayConfig, action: &str) -> String {
    format!(
        "{}/{}",
        config.facilitator_url.trim_end_matches('/'),
        action
    )
}

fn facilitator_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("build facilitator HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GatewayConfig;

    fn test_config() -> GatewayConfig {
        GatewayConfig {
            host: "127.0.0.1".into(),
            port: 8081,
            facilitator_url: "https://x402.org/facilitator".into(),
            payto_address: "0x1234567890123456789012345678901234567890".into(),
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
        }
    }

    #[test]
    fn payment_required_includes_price_and_network() {
        let (body, header) = payment_required_response(&test_config(), "/v1/measurements");
        assert_eq!(body.price, "$0.01");
        assert_eq!(body.network, "base-sepolia");
        assert!(!header.is_empty());
        let decoded = STANDARD.decode(header).unwrap();
        let v: Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(v["network"], "base-sepolia");
        assert_eq!(v["maxAmountRequired"], "10000");
    }

    #[test]
    fn price_atomic_usdc_parses_cents() {
        let cfg = test_config();
        assert_eq!(cfg.price_atomic_usdc(), "10000");
    }
}
