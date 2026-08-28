#!/usr/bin/env node
/**
 * Sample drone agent: pays via x402 USDC then POSTs a measurement.
 *
 *   cd aquachain/agent-gateway/examples
 *   npm install
 *   GATEWAY_URL=https://aquachain-agent-gateway.onrender.com \
 *   EVM_PRIVATE_KEY=0x… \
 *   node drone-agent.mjs
 */
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { wrapFetchWithPayment, x402Client } from "@x402/fetch";
import { toClientEvmSigner } from "@x402/evm";
import { ExactEvmSchemeV1 } from "@x402/evm/v1";
import { privateKeyToAccount } from "viem/accounts";
import { baseSepolia } from "viem/chains";
import { createPublicClient, http } from "viem";

const gatewayUrl = (process.env.GATEWAY_URL ?? "http://localhost:8081").replace(
  /\/$/,
  "",
);
const privateKey = process.env.EVM_PRIVATE_KEY?.trim();
if (!privateKey) {
  console.error(
    "Set EVM_PRIVATE_KEY (Base Sepolia account with USDC + ETH for gas)",
  );
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
const rpc = process.env.BASE_SEPOLIA_RPC ?? "https://sepolia.base.org";
const publicClient = createPublicClient({
  chain: baseSepolia,
  transport: http(rpc),
});
const evmSigner = toClientEvmSigner(account, publicClient);

const client = new x402Client().registerV1(
  "base-sepolia",
  new ExactEvmSchemeV1(evmSigner),
);
const x402Fetch = wrapFetchWithPayment(fetch, client);

console.log("Gateway:", gatewayUrl);
console.log("Agent:", account.address);
console.log("Payload flight:", payload.flight_id);

const ethBal = await publicClient.getBalance({ address: account.address });
const usdc = "0x036CbD53842c5426634e7929541eC2318f3dCF7e";
const usdcBal = await publicClient.readContract({
  address: usdc,
  abi: [
    {
      type: "function",
      name: "balanceOf",
      stateMutability: "view",
      inputs: [{ name: "account", type: "address" }],
      outputs: [{ name: "", type: "uint256" }],
    },
  ],
  functionName: "balanceOf",
  args: [account.address],
});
console.log(
  "Balances:",
  `${Number(ethBal) / 1e18} ETH,`,
  `${Number(usdcBal) / 1e6} USDC on Base Sepolia`,
);

const res = await x402Fetch(`${gatewayUrl}/v1/measurements`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(payload),
});

if (!res.ok) {
  const body = await res.text();
  console.error("Request failed:", res.status, body);
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
