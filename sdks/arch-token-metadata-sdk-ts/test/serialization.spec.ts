import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { TokenMetadataClient } from "../src/index";
import { Pubkey, systemProgram } from "../src/serde/pubkey";

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < clean.length; i += 2)
    out[i / 2] = parseInt(clean.slice(i, i + 2), 16);
  return out;
}

describe("instruction serialization matches Rust fixtures", () => {
  const fixtures = JSON.parse(
    readFileSync(
      new URL("./fixtures/metadata_instructions.json", import.meta.url)
    ).toString()
  );

  const programId = hexToBytes(fixtures.ProgramId) as Pubkey;
  const client = new TokenMetadataClient(programId);
  const payer = Buffer.alloc(32, 1) as Pubkey;
  const mint = Buffer.alloc(32, 2) as Pubkey;
  const auth = Buffer.alloc(32, 3) as Pubkey;

  it("CreateMetadata", () => {
    const ix = client.createMetadataIx({
      payer,
      mint,
      mintOrFreezeAuthority: auth,
      name: "Name",
      symbol: "SYM",
      image: "https://i",
      description: "desc",
      immutable: false,
    });
    // Only compare data payload to golden fixture (accounts are host-specific)
    const golden = hexToBytes(fixtures.CreateMetadata);
    expect(Buffer.from(ix.data)).toEqual(Buffer.from(golden));
  });

  it("UpdateMetadata", () => {
    const ix = client.updateMetadataIx({
      mint,
      updateAuthority: auth,
      name: "New",
      symbol: undefined,
      image: undefined,
      description: undefined,
    });
    const golden = hexToBytes(fixtures.UpdateMetadata);
    expect(Buffer.from(ix.data)).toEqual(Buffer.from(golden));
  });

  it("CreateAttributes", () => {
    const ix = client.createAttributesIx({
      payer,
      mint,
      updateAuthority: auth,
      data: [
        ["k1", "v1"],
        ["k2", "v2"],
      ],
    });
    const golden = hexToBytes(fixtures.CreateAttributes);
    expect(Buffer.from(ix.data)).toEqual(Buffer.from(golden));
  });

  it("ReplaceAttributes", () => {
    const ix = client.replaceAttributesIx({
      mint,
      updateAuthority: auth,
      data: [["a", "1"]],
    });
    const golden = hexToBytes(fixtures.ReplaceAttributes);
    expect(Buffer.from(ix.data)).toEqual(Buffer.from(golden));
  });

  it("TransferAuthority", () => {
    const newAuth = Buffer.alloc(32, 7) as Pubkey;
    const ix = client.transferAuthorityIx({
      mint,
      currentUpdateAuthority: auth,
      newAuthority: newAuth,
    });
    const golden = hexToBytes(fixtures.TransferAuthority);
    expect(Buffer.from(ix.data)).toEqual(Buffer.from(golden));
  });

  it("MakeImmutable", () => {
    const ix = client.makeImmutableIx({ mint, currentUpdateAuthority: auth });
    const golden = hexToBytes(fixtures.MakeImmutable);
    expect(Buffer.from(ix.data)).toEqual(Buffer.from(golden));
  });
});

describe("PDA and system/program id parity with Rust fixtures", () => {
  const fixtures = JSON.parse(
    readFileSync(
      new URL("./fixtures/metadata_instructions.json", import.meta.url)
    ).toString()
  );
  const programId = hexToBytes(fixtures.ProgramId) as Pubkey;
  const client = new TokenMetadataClient(programId);

  it("system program id matches", () => {
    const sysHex = fixtures.SystemProgram;
    expect(Buffer.from(systemProgram())).toEqual(
      Buffer.from(hexToBytes(sysHex))
    );
  });

  it("metadata/attributes PDA match for given mints", () => {
    for (const row of fixtures.PdaSamples) {
      const mint = hexToBytes(row.mint) as Pubkey;
      const md = client.metadataPda(mint);
      const attrs = client.attributesPda(mint);
      expect(Buffer.from(md)).toEqual(Buffer.from(hexToBytes(row.metadata)));
      expect(Buffer.from(attrs)).toEqual(
        Buffer.from(hexToBytes(row.attributes))
      );
    }
  });
});
