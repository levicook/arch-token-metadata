import { describe, it, expect } from "vitest";
import {
  TokenMetadataClient,
  type Instruction,
  type TxCreateTokenWithMetadataAndAttributesParams,
} from "../dist/index.js";
import { Pubkey } from "../dist/serde/pubkey.js";

function programIdFromString(s: string): Pubkey {
  return new Uint8Array(Buffer.from(s)).slice(0, 32) as Pubkey;
}

function u32ToLeBytes(n: number): Uint8Array {
  const buf = new Uint8Array(4);
  new DataView(buf.buffer).setUint32(0, n >>> 0, true);
  return buf;
}

describe("compute budget helpers (TS SDK)", () => {
  const programId = programIdFromString("arch-metadata000000000000000000");
  const client = new TokenMetadataClient(programId);

  it("setComputeUnitLimitIx encodes discriminant=1 and u32 LE units", () => {
    // @ts-expect-no-error: method exists at runtime
    const ix: Instruction = (client as any).setComputeUnitLimitIx(12_345);
    const cbPid = programIdFromString("ComputeBudget1111111111111111111");
    expect(Buffer.from(ix.programId)).toEqual(Buffer.from(cbPid));
    expect(ix.accounts.length).toBe(0);
    const expected = new Uint8Array([
      ...u32ToLeBytes(1),
      ...u32ToLeBytes(12_345),
    ]);
    expect(Buffer.from(ix.data)).toEqual(Buffer.from(expected));
  });

  it("requestHeapFrameIx encodes discriminant=0 and u32 LE bytes; enforces 1024 multiple", () => {
    // @ts-expect-no-error: method exists at runtime
    const ix: Instruction = (client as any).requestHeapFrameIx(64 * 1024);
    const cbPid = programIdFromString("ComputeBudget1111111111111111111");
    expect(Buffer.from(ix.programId)).toEqual(Buffer.from(cbPid));
    expect(ix.accounts.length).toBe(0);
    const expected = new Uint8Array([
      ...u32ToLeBytes(0),
      ...u32ToLeBytes(64 * 1024),
    ]);
    expect(Buffer.from(ix.data)).toEqual(Buffer.from(expected));

    expect(() => (client as any).requestHeapFrameIx(123)).toThrow();
  });
});

describe("withBudget variants prepend compute budget instructions", () => {
  const programId = programIdFromString("arch-metadata000000000000000000");
  const client = new TokenMetadataClient(programId);
  const payer = Buffer.alloc(32, 1) as Pubkey;
  const mint = Buffer.alloc(32, 2) as Pubkey;
  const auth = Buffer.alloc(32, 3) as Pubkey;

  it("createTokenWithMetadataAndAttributesTxWithBudget: budget first", () => {
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
      attributes: [["k", "v"]],
      mintInitializeInstructions: [],
    };
    // @ts-expect-no-error: method exists at runtime
    const ixs = (
      client as any
    ).createTokenWithMetadataAndAttributesTxWithBudget(params, {
      units: 10_000,
      heapBytes: 64 * 1024,
    }) as Instruction[];
    expect(ixs.length).toBeGreaterThanOrEqual(3);
    const cbPid = programIdFromString("ComputeBudget1111111111111111111");
    const ix0 = ixs[0]!;
    const ix1 = ixs[1]!;
    expect(Buffer.from(ix0.programId)).toEqual(Buffer.from(cbPid));
    expect(Buffer.from(ix1.programId)).toEqual(Buffer.from(cbPid));
  });
});
