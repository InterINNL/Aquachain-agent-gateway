# Agent client samples

## Dev bypass (no EVM wallet)

```bash
cd ..
cp .env.example .env
echo 'X402_DEV_BYPASS=true' >> .env
make run
```

```bash
curl -sS -X POST http://localhost:8081/v1/measurements \
  -H 'Content-Type: application/json' \
  -d @drone-measurement-yamuna.json | jq .
```

## x402 drone agent (G1)

Fund the payer wallet on **Base Sepolia testnet** (chain id 84532):

1. Test ETH for gas: https://www.alchemy.com/faucets/base-sepolia
2. Test USDC: https://faucet.circle.com/ (pick Base Sepolia)

Fund the gateway relayer on **Osmosis osmo-test-5** when relay returns queued/failed:

```bash
cd ../../contracts/scripts
node fund-relayer-osmosis.mjs
```

```bash
npm install
GATEWAY_URL=https://x402.interchouette.net \
EVM_PRIVATE_KEY=0xYOUR_BASE_SEPOLIA_KEY \
node drone-agent.mjs
```

## Payload

See [`drone-measurement-yamuna.json`](./drone-measurement-yamuna.json). Numeric fields are strings so CosmWasm JSON accepts them on relay.
