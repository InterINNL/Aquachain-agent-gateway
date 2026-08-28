# AquaChain Agent Gateway

HTTP gateway for **AI agents** that pay via **x402 (USDC on Base Sepolia)** and relay **drone water measurements** to **citizen-science-registry** on **Osmosis** (`osmo-test-5`).

Humans keep using Keplr + the Angular app. Agents use machine payments + this API.

## Phase status

| Phase            | Scope                                                                             |
| ---------------- | --------------------------------------------------------------------------------- |
| **G0**           | Rust scaffold, measurement validation, 402 stub, API spec                         |
| **G1** (current) | x402 facilitator verify/settle, CosmJS Osmosis relay, `@x402/fetch` sample client |
| **G2**           | LocalDAO v2 executable proposals                                                  |
| **G3**           | Agent verifier registry + UI polish                                               |

## Quick start

```bash
cd aquachain/agent-gateway
cp .env.example .env
# Set RELAYER_MNEMONIC + X402_PAYTO_ADDRESS for full stack
make test
make run
```

Dev acceptance without x402 wallet:

```bash
# in .env
X402_DEV_BYPASS=true
```

```bash
curl -sS http://localhost:8081/v1/capabilities | jq .
curl -sS -X POST http://localhost:8081/v1/measurements \
  -H 'Content-Type: application/json' \
  -d @examples/drone-measurement-yamuna.json | jq .
```

## x402 agent client

```bash
cd examples
npm install
GATEWAY_URL=http://localhost:8081 \
EVM_PRIVATE_KEY=0x… \
node drone-agent.mjs
```

Requires Base Sepolia USDC on the agent wallet. The client uses `@x402/fetch` to handle HTTP 402 automatically.

## Configuration

See [`.env.example`](.env.example). Secrets stay local (relayer mnemonic, pay-to address).

| Variable               | Role                               |
| ---------------------- | ---------------------------------- |
| `X402_PAYTO_ADDRESS`   | EVM address receiving USDC         |
| `X402_FACILITATOR_URL` | x402 facilitator (default testnet) |
| `CSR_CONTRACT_ADDRESS` | Osmosis citizen-science-registry   |
| `RELAYER_MNEMONIC`     | Signs Osmosis `submit_data`        |

Osmosis relay uses [`scripts/relay-submit-data.mjs`](scripts/relay-submit-data.mjs) (CosmJS bundled under `scripts/`).

## Production

Public URL: **`https://aquachain-gateway.interchouette.net`** (custom domain; TLS via hosting provider).

### Custom domain (DNS)

| Type  | Name                | Target                                 |
| ----- | ------------------- | -------------------------------------- |
| CNAME | `x402` | `aquachain-agent-gateway.onrender.com` |

After the CNAME propagates, attach the hostname on the gateway web service and set:

`AGENT_GATEWAY_PUBLIC_URL=https://aquachain-gateway.interchouette.net`

Point the frontend prod env `agentGatewayUrl` at the same URL (no trailing slash). x402 `resource` URLs in 402 responses use this value; it must match the URL agents call.

Set secrets in the hosting dashboard (never commit):

- `RELAYER_MNEMONIC` - Osmosis relayer with test OSMO
- `X402_PAYTO_ADDRESS` - Base Sepolia EVM address for USDC

Health check: `GET /health`. Capabilities: `GET /v1/capabilities`.

## API

Full spec: [`docs/api.md`](docs/api.md).

## Related

- [Aquachain-contracts](https://github.com/InterINNL/Aquachain-contracts)
- [Frontend](https://github.com/InterINNL/frontend)
