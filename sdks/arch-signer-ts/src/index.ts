import { Signer as Bip322Signer } from "bip322-js";
import * as bitcoin from "bitcoinjs-lib";
import wif from "wif";
import { secp256k1 } from "@noble/curves/secp256k1";

export type ArchNetwork = "regtest" | "testnet" | "mainnet";

function toBitcoinNetwork(network: ArchNetwork) {
  switch (network) {
    case "mainnet":
      return bitcoin.networks.bitcoin;
    case "testnet":
      return bitcoin.networks.testnet;
    case "regtest":
      return bitcoin.networks.regtest;
  }
}

function wifVersion(network: ArchNetwork): number {
  return network === "mainnet" ? 0x80 : 0xef;
}

export function deriveP2trAddress(
  pubkeyXOnly: Uint8Array,
  network: ArchNetwork,
): string {
  const addr = bitcoin.payments.p2tr({
    internalPubkey: Buffer.from(pubkeyXOnly),
    network: toBitcoinNetwork(network),
  }).address;
  if (!addr) throw new Error("Failed to derive P2TR address");
  return addr;
}

export function toWif(privHex: string, network: ArchNetwork): string {
  return wif.encode(wifVersion(network), Buffer.from(privHex, "hex"), true);
}

export function signBip322P2tr(
  privHex: string,
  message: Uint8Array,
  network: ArchNetwork,
): Uint8Array {
  const priv = Buffer.from(privHex, "hex");
  const full = secp256k1.getPublicKey(priv, true);
  const xOnly = full.slice(1);
  const addr = deriveP2trAddress(xOnly, network);
  const w = toWif(privHex, network);
  const b64 = Bip322Signer.sign(w, addr, Buffer.from(message));
  const witness = Buffer.from(b64, "base64");
  const end =
    witness[witness.length - 1] === 1 ? witness.length - 1 : witness.length;
  return witness.slice(end - 64, end);
}
