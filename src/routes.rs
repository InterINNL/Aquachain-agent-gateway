//! HTTP routes for the agent gateway.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::GatewayConfig;
use crate::measurement::{DroneMeasurement, MeasurementError};
use crate::relay::{relay_measurement, RelayStatus};
use crate::store::{MeasurementStore, StoredMeasurement};
use crate::x402::{
    has_payment_header, payment_header_value, payment_required_response, settle_payment,
    verify_payment, PaymentError, PaymentSession, HEADER_PAYMENT_REQUIRED, HEADER_PAYMENT_RESPONSE,
};

const PHASE: &str = "g1";

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<GatewayConfig>,
    pub store: Arc<MeasurementStore>,
}

/// Builds the gateway router.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/measurements", post(post_measurement))
        .route("/v1/measurements/{id}", get(get_measurement))
        .with_state(state)
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    phase: &'static str,
    relay_ready: bool,
    x402_ready: bool,
    stored_measurements: usize,
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        phase: PHASE,
        relay_ready: state.config.relay_ready(),
        x402_ready: state.config.x402_ready(),
        stored_measurements: state.store.len(),
    })
}

#[derive(Serialize)]
struct CapabilitiesResponse {
    phase: &'static str,
    endpoints: Vec<EndpointInfo>,
    payment: PaymentInfo,
    relay: RelayInfo,
    sample_payload: DroneMeasurement,
}

#[derive(Serialize)]
struct EndpointInfo {
    method: &'static str,
    path: &'static str,
    price: &'static str,
    description: &'static str,
}

#[derive(Serialize)]
struct PaymentInfo {
    protocol: &'static str,
    asset: String,
    network: String,
    price_usdc: String,
    facilitator: String,
    ready: bool,
}

#[derive(Serialize)]
struct RelayInfo {
    chain_id: String,
    rpc: String,
    contract: Option<String>,
    ready: bool,
}

async fn capabilities(State(state): State<AppState>) -> Json<CapabilitiesResponse> {
    Json(CapabilitiesResponse {
        phase: PHASE,
        endpoints: vec![
            EndpointInfo {
                method: "GET",
                path: "/v1/capabilities",
                price: "free",
                description: "Service discovery for agents",
            },
            EndpointInfo {
                method: "POST",
                path: "/v1/measurements",
                price: "x402 USDC",
                description: "Submit drone reading; relays to citizen-science-registry on Osmosis",
            },
            EndpointInfo {
                method: "GET",
                path: "/v1/measurements/{id}",
                price: "free",
                description: "Fetch mirrored measurement and relay status",
            },
        ],
        payment: PaymentInfo {
            protocol: "x402",
            asset: state.config.x402_asset.clone(),
            network: state.config.x402_network.clone(),
            price_usdc: state.config.price_usdc.clone(),
            facilitator: state.config.facilitator_url.clone(),
            ready: state.config.x402_ready(),
        },
        relay: RelayInfo {
            chain_id: state.config.osmosis_chain_id.clone(),
            rpc: state.config.osmosis_rpc.clone(),
            contract: state.config.csr_contract.clone(),
            ready: state.config.relay_ready(),
        },
        sample_payload: DroneMeasurement {
            lat: "28.70".into(),
            lon: "77.22".into(),
            turbidity: "14.2".into(),
            image_hash: "sha256:demo-yamuna-frame-001".into(),
            flight_id: "yamuna-drone-001".into(),
            sensor_id: Some(1),
            unit: Some("NTU".into()),
            site: Some("Yamuna Wazirabad, Delhi NCR, India".into()),
        },
    })
}

#[derive(Serialize)]
struct MeasurementAccepted {
    id: String,
    relay: serde_json::Value,
    chain_data: serde_json::Value,
    payment: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    details: Option<String>,
}

async fn post_measurement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DroneMeasurement>,
) -> Response {
    if let Err(err) = body.validate() {
        return measurement_error(StatusCode::BAD_REQUEST, err);
    }

    let payment_session = if state.config.dev_bypass_payment {
        info!("X402_DEV_BYPASS: skipping payment verify/settle");
        None
    } else if !has_payment_header(&headers) {
        let (body, header_value) = payment_required_response(&state.config, "/v1/measurements");
        return (
            StatusCode::PAYMENT_REQUIRED,
            [(HEADER_PAYMENT_REQUIRED, header_value)],
            Json(body),
        )
            .into_response();
    } else {
        match resolve_payment(&state.config, &headers).await {
            Ok(session) => session,
            Err(err) => return payment_error(err),
        }
    };

    let id = Uuid::new_v4().to_string();
    let relay = match relay_measurement(&state.config, &body, &id).await {
        Ok(outcome) => outcome,
        Err(err) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorBody {
                    error: "relay_error".into(),
                    details: Some(err.to_string()),
                }),
            )
                .into_response();
        }
    };

    if relay.status == RelayStatus::Failed {
        return (
            StatusCode::BAD_GATEWAY,
            Json(ErrorBody {
                error: "relay_failed".into(),
                details: Some(relay.message.clone()),
            }),
        )
            .into_response();
    }

    let payment_response = if let Some(session) = payment_session {
        match settle_payment(&state.config, &session).await {
            Ok(settled) => Some(settled),
            Err(err) => {
                warn!(error = %err, "x402 settle failed after successful relay");
                return payment_error(err);
            }
        }
    } else {
        None
    };

    let created_at = chrono_now_rfc3339();
    state.store.insert(StoredMeasurement {
        id: id.clone(),
        measurement: body.clone(),
        relay: relay.clone(),
        created_at,
    });

    let mut response = Json(MeasurementAccepted {
        id,
        relay: relay.to_api(),
        chain_data: body.to_chain_data(),
        payment: payment_response.clone(),
    })
    .into_response();

    if let Some(settled) = payment_response {
        use base64::{engine::general_purpose::STANDARD, Engine};
        if let Ok(header) = axum::http::HeaderValue::from_str(&STANDARD.encode(settled.to_string()))
        {
            response
                .headers_mut()
                .insert(HEADER_PAYMENT_RESPONSE, header);
        }
    }

    response
}

async fn get_measurement(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.store.get(&id) {
        Some(record) => Json(record).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: "not_found".into(),
                details: None,
            }),
        )
            .into_response(),
    }
}

async fn resolve_payment(
    config: &GatewayConfig,
    headers: &HeaderMap,
) -> Result<Option<PaymentSession>, PaymentError> {
    let header = payment_header_value(headers).ok_or(PaymentError::MissingHeader)?;
    let session = verify_payment(config, &header, "/v1/measurements").await?;
    info!("x402 payment verified");
    Ok(Some(session))
}

fn payment_error(err: PaymentError) -> Response {
    match err {
        PaymentError::MissingHeader => {
            // handled by caller before resolve_payment in post flow - should not happen
            (
                StatusCode::PAYMENT_REQUIRED,
                Json(ErrorBody {
                    error: "payment_required".into(),
                    details: Some(err.to_string()),
                }),
            )
                .into_response()
        }
        PaymentError::VerifyFailed(details) | PaymentError::SettleFailed(details) => (
            StatusCode::PAYMENT_REQUIRED,
            Json(ErrorBody {
                error: "payment_invalid".into(),
                details: Some(details),
            }),
        )
            .into_response(),
    }
}

fn measurement_error(status: StatusCode, err: MeasurementError) -> Response {
    (
        status,
        Json(ErrorBody {
            error: "invalid_measurement".into(),
            details: Some(err.to_string()),
        }),
    )
        .into_response()
}

fn chrono_now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::x402::BASE_SEPOLIA_USDC;

    fn test_state(dev_bypass: bool) -> AppState {
        AppState {
            config: Arc::new(GatewayConfig {
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
                dev_bypass_payment: dev_bypass,
            }),
            store: Arc::new(MeasurementStore::default()),
        }
    }

    #[tokio::test]
    async fn health_returns_g1_phase() {
        let app = build_router(test_state(false));
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["phase"], PHASE);
    }

    #[tokio::test]
    async fn post_without_payment_returns_402() {
        let app = build_router(test_state(false));
        let body = serde_json::json!({
            "lat": "28.70",
            "lon": "77.22",
            "turbidity": "14.2",
            "image_hash": "sha256:abc",
            "flight_id": "test-flight"
        });
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/measurements")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(res.headers().contains_key(HEADER_PAYMENT_REQUIRED));
    }

    #[tokio::test]
    async fn post_with_dev_bypass_accepts() {
        let app = build_router(test_state(true));
        let body = serde_json::json!({
            "lat": "28.70",
            "lon": "77.22",
            "turbidity": "14.2",
            "image_hash": "sha256:abc",
            "flight_id": "test-flight"
        });
        let res = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/measurements")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.get("id").is_some());
    }
}
