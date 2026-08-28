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

```bash
npm install
GATEWAY_URL=http://localhost:8081 \
EVM_PRIVATE_KEY=0xYOUR_BASE_SEPOLIA_KEY \
node drone-agent.mjs
```

Fund the agent with Base Sepolia USDC before running. The script pays ~$0.01 USDC per measurement via x402, then the gateway relays to Osmosis when `RELAYER_MNEMONIC` is configured.

## Payload

See [`drone-measurement-yamuna.json`](./drone-measurement-yamuna.json). Numeric fields are strings so CosmWasm JSON accepts them on relay.
