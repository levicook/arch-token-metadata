import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { TokenMetadataClient, TokenMetadataReader } from "../src/index.js";
import { Pubkey, systemProgram } from "../src/serde/pubkey.js";

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
      new URL("./fixtures/metadata_instructions.json", import.meta.url),
    ).toString(),
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

  it("ComputeBudget helpers match Rust fixtures", () => {
    const cb = fixtures.ComputeBudget;
    const cbPid = hexToBytes(cb.ProgramId) as Pubkey;
    // Access helpers via dynamic any to avoid TS surface drift
    const req = (client as any).requestHeapFrameIx(64 * 1024);
    const set = (client as any).setComputeUnitLimitIx(12_000);
    expect(Buffer.from(req.programId)).toEqual(Buffer.from(cbPid));
    expect(Buffer.from(set.programId)).toEqual(Buffer.from(cbPid));
    expect(Buffer.from(req.data)).toEqual(
      Buffer.from(hexToBytes(cb.RequestHeapFrame_64k)),
    );
    expect(Buffer.from(set.data)).toEqual(
      Buffer.from(hexToBytes(cb.SetComputeUnitLimit_12000)),
    );
  });
});

describe("PDA and system/program id parity with Rust fixtures", () => {
  const fixtures = JSON.parse(
    readFileSync(
      new URL("./fixtures/metadata_instructions.json", import.meta.url),
    ).toString(),
  );
  const programId = hexToBytes(fixtures.ProgramId) as Pubkey;
  const client = new TokenMetadataClient(programId);

  it("system program id matches", () => {
    const sysHex = fixtures.SystemProgram;
    expect(Buffer.from(systemProgram())).toEqual(
      Buffer.from(hexToBytes(sysHex)),
    );
  });

  it("metadata/attributes PDA match for given mints", () => {
    for (const row of fixtures.PdaSamples) {
      const mint = hexToBytes(row.mint) as Pubkey;
      const md = client.metadataPda(mint);
      const attrs = client.attributesPda(mint);
      expect(Buffer.from(md)).toEqual(Buffer.from(hexToBytes(row.metadata)));
      expect(Buffer.from(attrs)).toEqual(
        Buffer.from(hexToBytes(row.attributes)),
      );
    }
  });
});

describe("reader decodes packed account fixtures and supports round-trip re-encode sanity", () => {
  const fixtures = JSON.parse(
    readFileSync(
      new URL("./fixtures/metadata_instructions.json", import.meta.url),
    ).toString(),
  );

  // Minimal mock implementing AccountReader
  class MockReader {
    private map: Map<string, { data: Uint8Array; owner: Uint8Array }> =
      new Map();
    constructor(
      entries: Array<{
        pubkey: Uint8Array;
        data: Uint8Array;
        owner: Uint8Array;
      }>,
    ) {
      for (const e of entries) {
        this.map.set(Buffer.from(e.pubkey).toString("hex"), {
          data: e.data,
          owner: e.owner,
        });
      }
    }
    async getMultipleAccounts(pubkeys: Uint8Array[]) {
      return pubkeys.map((pk) => {
        const v = this.map.get(Buffer.from(pk).toString("hex"));
        return v
          ? { data: v.data, owner: v.owner }
          : { data: null, owner: null };
      });
    }
  }

  it("decodes metadata + attributes packed accounts", async () => {
    const programId = hexToBytes(fixtures.ProgramId) as Pubkey;
    const client = new TokenMetadataClient(programId);
    const mint = hexToBytes(fixtures.Sample.mint) as Pubkey;
    const metadataPda = hexToBytes(fixtures.PdaSamples[0].metadata) as Pubkey;
    const attributesPda = hexToBytes(
      fixtures.PdaSamples[0].attributes,
    ) as Pubkey;
    const mdPacked = hexToBytes(fixtures.Sample.metadata_account);
    const atPacked = hexToBytes(fixtures.Sample.attributes_account);

    const mock = new MockReader([
      { pubkey: metadataPda, data: mdPacked, owner: programId },
      { pubkey: attributesPda, data: atPacked, owner: programId },
    ]);

    const reader = new TokenMetadataReader(programId, mock as any);

    const details = await reader.getTokenDetails(mint);
    expect(details.metadata).toBeTruthy();
    expect(details.attributes).toBeTruthy();

    // Basic field integrity
    expect(details.metadata!.name).toBe("Name");
    expect(details.metadata!.symbol).toBe("SYM");
    expect(details.metadata!.image).toBe("https://i");
    expect(details.metadata!.description).toBe("desc");

    // Build instructions from decoded values to ensure stable encoding paths
    const cm = client.createMetadataIx({
      payer: Buffer.alloc(32) as Pubkey,
      mint,
      mintOrFreezeAuthority: Buffer.alloc(32) as Pubkey,
      name: details.metadata!.name,
      symbol: details.metadata!.symbol,
      image: details.metadata!.image,
      description: details.metadata!.description,
      immutable: details.metadata!.update_authority === undefined,
    });
    expect(cm.data[0]).toBe(0); // variant tag sanity

    const ra = client.replaceAttributesIx({
      mint,
      updateAuthority: Buffer.alloc(32) as Pubkey,
      data: details.attributes!.data,
    });
    expect(ra.data[0]).toBe(3); // variant tag sanity
  });

  it("batch decodes metadata and attributes for multiple mints", async () => {
    const programId = hexToBytes(fixtures.ProgramId) as Pubkey;
    const reader = new TokenMetadataReader(programId, {
      async getMultipleAccounts(pubkeys: Uint8Array[]) {
        const md1 = hexToBytes(fixtures.Sample.metadata_account);
        const at1 = hexToBytes(fixtures.Sample.attributes_account);
        const md2 = hexToBytes(fixtures.Sample2.metadata_account);
        const at2 = hexToBytes(fixtures.Sample2.attributes_account);
        const owner = hexToBytes(fixtures.ProgramId);
        // derive PDAs expected order: [md(mint1), md(mint2)] or [at(mint1), at(mint2)] in callers below
        return pubkeys.map((pk, idx) => {
          const hex = Buffer.from(pk).toString("hex");
          // naive mapping using fixture PDAs
          if (hex === fixtures.PdaSamples[0].metadata)
            return { data: md1, owner };
          if (hex === fixtures.PdaSamples[1].metadata)
            return { data: md2, owner };
          if (hex === fixtures.PdaSamples[0].attributes)
            return { data: at1, owner };
          if (hex === fixtures.PdaSamples[1].attributes)
            return { data: at2, owner };
          return { data: null, owner: null } as any;
        });
      },
    } as any);

    const mint1 = hexToBytes(fixtures.Sample.mint) as Pubkey;
    const mint2 = hexToBytes(fixtures.Sample2.mint) as Pubkey;

    const mdBatch = await reader.getTokenMetadataBatch([mint1, mint2]);
    const atBatch = await reader.getTokenMetadataAttributesBatch([
      mint1,
      mint2,
    ]);
    expect(mdBatch.length).toBe(2);
    expect(atBatch.length).toBe(2);
    expect(mdBatch[0]?.name).toBe("Name");
    expect(mdBatch[1]?.symbol).toBe("SYM");
    expect(atBatch[0]?.data.length).toBeGreaterThan(0);
    expect(atBatch[1]?.data.length).toBeGreaterThan(0);
  });
});
