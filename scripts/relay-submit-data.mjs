#!/usr/bin/env node
/**
 * Relay submit_data to citizen-science-registry on Osmosis.
 * Reads JSON from stdin, writes JSON to stdout.
 *
 * Input: { rpc, denom, contract, mnemonic, sensor_id, data }
 * Output: { ok: true, tx_hash } | { ok: false, error }
 */
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(
  resolve(dirname(fileURLToPath(import.meta.url)), "package.json"),
);

const { SigningCosmWasmClient } = require("@cosmjs/cosmwasm-stargate");
const { DirectSecp256k1HdWallet } = require("@cosmjs/proto-signing");

async function main() {
  const raw = readFileSync(0, "utf8");
  const input = JSON.parse(raw);
  const {
    rpc,
    denom = "uosmo",
    contract,
    mnemonic,
    sensor_id: sensorId,
    data,
  } = input;

  if (!rpc || !contract || !mnemonic || sensorId == null || !data) {
    process.stdout.write(
      JSON.stringify({ ok: false, error: "missing required relay fields" }),
    );
    process.exit(1);
  }

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic.trim(), {
    prefix: "osmo",
  });
  const [account] = await wallet.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(rpc, wallet);

  const fee = {
    amount: [{ denom, amount: "80000" }],
    gas: "1500000",
  };

  const msg = { submit_data: { sensor_id: Number(sensorId), data } };
  const res = await client.execute(
    account.address,
    contract,
    msg,
    fee,
    "agent-gateway drone measurement",
  );

  process.stdout.write(
    JSON.stringify({ ok: true, tx_hash: res.transactionHash }),
  );
}

main().catch((err) => {
  process.stdout.write(
    JSON.stringify({
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    }),
  );
  process.exit(1);
});
