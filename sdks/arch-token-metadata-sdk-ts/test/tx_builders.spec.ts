import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import {
  TokenMetadataClient,
  type Instruction,
  type TxCreateTokenWithMetadataParams,
  type TxCreateTokenWithMetadataAndAttributesParams,
  type TxCreateTokenWithFreezeAuthMetadataParams,
} from "../dist/index.js";
import { Pubkey } from "../dist/serde/pubkey.js";

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < clean.length; i += 2)
    out[i / 2] = parseInt(clean.slice(i, i + 2), 16);
  return out;
}

function dummyIx(programIdByte: number, dataBytes: number): Instruction {
  return {
    programId: Buffer.alloc(32, programIdByte) as Pubkey,
    accounts: [],
    data: Buffer.alloc(dataBytes, 9),
  };
}

describe("transaction builders compose upstream instructions correctly", () => {
  const fixtures = JSON.parse(
    readFileSync(
      new URL("./fixtures/metadata_instructions.json", import.meta.url),
    ).toString(),
  );
  const programId = hexToBytes(fixtures.ProgramId) as Pubkey;
  const client = new TokenMetadataClient(programId);
  const payer = Buffer.alloc(32, 1) as Pubkey;
  const mint = Buffer.alloc(32, 2) as Pubkey;
  const auth = Buffer.alloc(32, 3) as Pubkey;
  const freeze = Buffer.alloc(32, 4) as Pubkey;

  it("createTokenWithMetadataTx: preserves upstream instructions and appends createMetadata", () => {
    const upstream: Instruction[] = [dummyIx(10, 3), dummyIx(11, 4)];
    const params: TxCreateTokenWithMetadataParams = {
      payer,
      mint,
      mintAuthority: auth,
      freezeAuthority: undefined,
      decimals: 9,
      name: "Name",
      symbol: "SYM",
      image: "https://i",
      description: "desc",
      immutable: false,
      mintInitializeInstructions: upstream,
    };

    const ixs = client.createTokenWithMetadataTx(params);
    const ix0 = ixs[0]!;
    const ix1 = ixs[1]!;
    const ix2 = ixs[2]!;
    const up0 = upstream[0]!;
    const up1 = upstream[1]!;
    expect(ixs.length).toBe(3);
    // upstream preserved
    expect(Buffer.from(ix0.programId)).toEqual(Buffer.from(up0.programId));
    expect(Buffer.from(ix1.programId)).toEqual(Buffer.from(up1.programId));
    expect(Buffer.from(ix0.data)).toEqual(Buffer.from(up0.data));
    expect(Buffer.from(ix1.data)).toEqual(Buffer.from(up1.data));
    // last is create metadata with correct data
    const golden = hexToBytes(fixtures.CreateMetadata);
    expect(Buffer.from(ix2.data)).toEqual(Buffer.from(golden));
  });

  it("createTokenWithMetadataAndAttributesTx: upstream + createMetadata + createAttributes", () => {
    const upstream: Instruction[] = [dummyIx(10, 3), dummyIx(11, 4)];
    const params: TxCreateTokenWithMetadataAndAttributesParams = {
      payer,
      mint,
      mintAuthority: auth,
      freezeAuthority: undefined,
      decimals: 9,
      name: "Name",
      symbol: "SYM",
      image: "https://i",
      description: "desc",
      immutable: false,
      attributes: [
        ["k1", "v1"],
        ["k2", "v2"],
      ],
      mintInitializeInstructions: upstream,
    };
    const ixs = client.createTokenWithMetadataAndAttributesTx(params);
    const ix0 = ixs[0]!;
    const ix1 = ixs[1]!;
    const ix2 = ixs[2]!;
    const ix3 = ixs[3]!;
    const up0 = upstream[0]!;
    const up1 = upstream[1]!;
    expect(ixs.length).toBe(4);
    // upstream preserved
    expect(Buffer.from(ix0.programId)).toEqual(Buffer.from(up0.programId));
    expect(Buffer.from(ix1.programId)).toEqual(Buffer.from(up1.programId));
    // check tails
    const goldenMd = hexToBytes(fixtures.CreateMetadata);
    const goldenAttrs = hexToBytes(fixtures.CreateAttributes);
    expect(Buffer.from(ix2.data)).toEqual(Buffer.from(goldenMd));
    expect(Buffer.from(ix3.data)).toEqual(Buffer.from(goldenAttrs));
  });

  it("createTokenWithFreezeAuthMetadataTx: upstream + clearMintAuth + createMetadata (freeze signer)", () => {
    const upstream: Instruction[] = [dummyIx(10, 3), dummyIx(11, 4)];
    const clearMintAuth: Instruction = dummyIx(12, 5);
    const params: TxCreateTokenWithFreezeAuthMetadataParams = {
      payer,
      mint,
      initialMintAuthority: auth,
      freezeAuthority: freeze,
      decimals: 9,
      name: "Name",
      symbol: "SYM",
      image: "https://i",
      description: "desc",
      immutable: false,
      mintInitializeInstructions: upstream,
      clearMintAuthorityInstruction: clearMintAuth,
    };
    const ixs = client.createTokenWithFreezeAuthMetadataTx(params);
    const ix0 = ixs[0]!;
    const ix1 = ixs[1]!;
    const ix2 = ixs[2]!;
    const ix3 = ixs[3]!;
    const up0 = upstream[0]!;
    const up1 = upstream[1]!;
    expect(ixs.length).toBe(4);
    // order: upstream[0], upstream[1], clear, createMd
    expect(Buffer.from(ix0.programId)).toEqual(Buffer.from(up0.programId));
    expect(Buffer.from(ix1.programId)).toEqual(Buffer.from(up1.programId));
    expect(Buffer.from(ix2.programId)).toEqual(
      Buffer.from(clearMintAuth.programId),
    );
    const goldenMd = hexToBytes(fixtures.CreateMetadata);
    expect(Buffer.from(ix3.data)).toEqual(Buffer.from(goldenMd));
  });
});
