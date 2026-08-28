#!/usr/bin/env node
/**
 * Sample drone agent: pays via x402 USDC then POSTs a measurement.
 *
 *   cd aquachain/agent-gateway/examples
 *   npm install
 *   GATEWAY_URL=http://localhost:8081 \
 *   EVM_PRIVATE_KEY=0x… \
 *   node drone-agent.mjs
 */
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { wrapFetchWithPayment, decodeX402Error } from "@x402/fetch";
import { privateKeyToAccount } from "viem/accounts";
import { baseSepolia } from "viem/chains";
import { createWalletClient, http } from "viem";

const gatewayUrl = (process.env.GATEWAY_URL ?? "http://localhost:8081").replace(
  /\/$/,
  "",
);
const privateKey = process.env.EVM_PRIVATE_KEY?.trim();
if (!privateKey) {
  console.error("Set EVM_PRIVATE_KEY (Base Sepolia account with USDC)");
  process.exit(1);
}

const fixturePath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "drone-measurement-yamuna.json",
);
const payload = JSON.parse(readFileSync(fixturePath, "utf8"));

const account = privateKeyToAccount(
  privateKey.startsWith("0x") ? privateKey : `0x${privateKey}`,
);
const walletClient = createWalletClient({
  account,
  chain: baseSepolia,
  transport: http(),
});

const x402Fetch = wrapFetchWithPayment(fetch, walletClient);

console.log("Gateway:", gatewayUrl);
console.log("Agent:", account.address);
console.log("Payload flight:", payload.flight_id);

const res = await x402Fetch(`${gatewayUrl}/v1/measurements`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(payload),
});

if (!res.ok) {
  const body = await res.text();
  let detail = body;
  try {
    detail = decodeX402Error(JSON.parse(body));
  } catch {
    // keep raw body
  }
  console.error("Request failed:", res.status, detail);
  process.exit(1);
}

const result = await res.json();
console.log("Accepted measurement id:", result.id);
console.log("Relay:", result.relay);
if (result.relay?.txHash) {
  console.log(
    "Osmosis tx:",
    `https://www.mintscan.io/osmosis-testnet/tx/${result.relay.txHash}`,
  );
}
