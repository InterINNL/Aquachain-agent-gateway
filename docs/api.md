# Agent Gateway API (Phase G1)

Base path: `/v1`. Payment protocol: [x402](https://docs.x402.org/) (USDC on Base Sepolia for demo).

## GET /health

```json
{
  "status": "ok",
  "phase": "g1",
  "relay_ready": true,
  "x402_ready": true,
  "stored_measurements": 0
}
```

## GET /v1/capabilities

Free discovery for agents. Returns endpoint list, x402 pricing, Osmosis relay target, and a sample drone payload.

## POST /v1/measurements

| Case                        | Status | Body                                                                              |
| --------------------------- | ------ | --------------------------------------------------------------------------------- |
| Invalid JSON fields         | 400    | `{ "error": "invalid_measurement", "details": "..." }`                            |
| No payment header           | 402    | Payment requirements + `PAYMENT-REQUIRED` header (base64 JSON)                    |
| Invalid x402 payment        | 402    | `{ "error": "payment_invalid", "details": "..." }`                                |
| Paid + relay ok             | 200    | `{ "id", "relay", "chain_data", "payment" }` + optional `PAYMENT-RESPONSE` header |
| Relay configured but failed | 502    | `{ "error": "relay_failed", "details": "..." }`                                   |

Flow: **verify** (facilitator) → **relay** (CosmJS `submit_data` on Osmosis) → **settle** (facilitator).

Dev shortcut: `X402_DEV_BYPASS=true` skips verify/settle.

### Request body

| Field        | Type   | Required         |
| ------------ | ------ | ---------------- |
| `lat`        | string | yes              |
| `lon`        | string | yes              |
| `turbidity`  | string | yes              |
| `image_hash` | string | yes              |
| `flight_id`  | string | yes              |
| `sensor_id`  | number | no               |
| `unit`       | string | no (default NTU) |
| `site`       | string | no               |

Relays to citizen-science-registry as:

```json
{
  "submit_data": {
    "sensor_id": 1,
    "data": {
      "source": "drone_agent",
      "flight_id": "...",
      "lat": "...",
      "lon": "...",
      "value": "...",
      "unit": "NTU",
      "image_hash": "..."
    }
  }
}
```

## GET /v1/measurements/{id}

Returns stored mirror row after successful POST.

## Agent client

See [`examples/drone-agent.mjs`](../examples/drone-agent.mjs) (`@x402/fetch` + Base Sepolia USDC wallet).

## Configuration

See [`.env.example`](../.env.example): `X402_PAYTO_ADDRESS`, `RELAYER_MNEMONIC`, `CSR_CONTRACT_ADDRESS`.
