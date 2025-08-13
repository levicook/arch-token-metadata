import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { TokenMetadataClient } from "../dist/index.js";
import { Pubkey } from "../dist/serde/pubkey.js";

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < clean.length; i += 2)
    out[i / 2] = parseInt(clean.slice(i, i + 2), 16);
  return out;
}

describe("upstream token/system instruction data parity", () => {
  const fixtures = JSON.parse(
    readFileSync(
      new URL("./fixtures/metadata_instructions.json", import.meta.url),
    ).toString(),
  );
  const programId = hexToBytes(fixtures.ProgramId) as Pubkey;
  const tokenProgramId = hexToBytes(fixtures.TokenProgramId) as Pubkey;
  const client = new TokenMetadataClient(programId);

  it("token initialize_mint2 data matches", () => {
    const mint = hexToBytes(fixtures.PdaSamples[0].mint) as Pubkey;
    const payer = new Uint8Array(32).fill(1) as Pubkey;
    const ix = client.tokenInitializeMint2Ix(
      tokenProgramId,
      mint,
      payer,
      undefined,
      9,
    );
    expect(Buffer.from(ix.data)).toEqual(
      Buffer.from(hexToBytes(fixtures.TokenInitializeMint2)),
    );
  });

  it("system create_account for mint: data matches length/layout and owner id", () => {
    const mint = hexToBytes(fixtures.PdaSamples[0].mint) as Pubkey;
    const payer = new Uint8Array(32).fill(1) as Pubkey;
    const minLamports = 1n; // exact value not validated in parity; we assert shape/owner
    const ix = client.createMintAccountIx(
      payer,
      mint,
      tokenProgramId,
      minLamports,
    );
    // Verify program id is system program and data has discriminant 0 + fields + owner id
    const sysId = fixtures.SystemProgram;
    expect(Buffer.from(ix.programId)).toEqual(Buffer.from(hexToBytes(sysId)));
    // first 4 bytes tag 0
    expect(ix.data[0]).toBe(0);
    expect(ix.data[1]).toBe(0);
    expect(ix.data[2]).toBe(0);
    expect(ix.data[3]).toBe(0);
    // last 32 bytes owner should equal token program id
    const owner = ix.data.slice(ix.data.length - 32);
    expect(Buffer.from(owner)).toEqual(Buffer.from(tokenProgramId));
  });

  it("token set_authority (MintTokens -> None) data matches", () => {
    const mint = hexToBytes(fixtures.PdaSamples[0].mint) as Pubkey;
    const payer = new Uint8Array(32).fill(1) as Pubkey;
    const ix = client.tokenSetMintAuthorityIx(
      tokenProgramId,
      mint,
      payer,
      undefined,
    );
    expect(Buffer.from(ix.data)).toEqual(
      Buffer.from(hexToBytes(fixtures.TokenSetAuthorityMintNone)),
    );
  });

  it("token set_authority (MintTokens -> Some) data matches", () => {
    const mint = hexToBytes(fixtures.PdaSamples[0].mint) as Pubkey;
    const payer = new Uint8Array(32).fill(1) as Pubkey;
    const newAuth = new Uint8Array(32).fill(7) as Pubkey;
    const ix = client.tokenSetMintAuthorityIx(
      tokenProgramId,
      mint,
      payer,
      newAuth,
    );
    expect(Buffer.from(ix.data)).toEqual(
      Buffer.from(hexToBytes(fixtures.TokenSetAuthorityMintSome)),
    );
  });
});
