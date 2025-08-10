// Arch Token Metadata – TypeScript SDK (client-side helpers)
// - PDA helpers
// - Instruction builders with validation mirroring Rust SDK
// - Transaction builders (return arrays of instructions)

import { findProgramAddress, Pubkey, systemProgram } from "./serde/pubkey.js";

export type AccountMeta = {
  pubkey: Pubkey;
  isSigner: boolean;
  isWritable: boolean;
};

export type Instruction = {
  programId: Pubkey;
  accounts: AccountMeta[];
  data: Uint8Array;
};

// Constants mirrored from on-chain program
export const NAME_MAX_LEN = 256;
export const SYMBOL_MAX_LEN = 16;
export const IMAGE_MAX_LEN = 512;
export const DESCRIPTION_MAX_LEN = 512;
export const MAX_KEY_LENGTH = 64;
export const MAX_VALUE_LENGTH = 240;
export const MAX_ATTRIBUTES = 32;

// Borsh encoding helpers minimal
// Note: Here we rely on Rust fixtures to cross-check correctness. For now, we use a minimal encoder
// to build instruction byte arrays in an equivalent way (Variant index + fields serialized).

function encodeString(s: string): Uint8Array {
  const enc = new TextEncoder();
  const bytes = enc.encode(s);
  const len = new Uint8Array(4);
  new DataView(len.buffer).setUint32(0, bytes.length, true);
  const out = new Uint8Array(4 + bytes.length);
  out.set(len, 0);
  out.set(bytes, 4);
  return out;
}

function encodeOptionString(s: string | undefined | null): Uint8Array {
  if (s == null) return new Uint8Array([0]);
  const inner = encodeString(s);
  const out = new Uint8Array(1 + inner.length);
  out[0] = 1;
  out.set(inner, 1);
  return out;
}

function concat(parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

function encodeVecTupleStringString(
  items: Array<[string, string]>
): Uint8Array {
  const len = new Uint8Array(4);
  new DataView(len.buffer).setUint32(0, items.length, true);
  const rest = items.map(([k, v]) =>
    concat([encodeString(k), encodeString(v)])
  );
  return concat([len, ...rest]);
}

// Instruction variant tags must match Rust ordering
const IX_CREATE_METADATA = 0;
const IX_UPDATE_METADATA = 1;
const IX_CREATE_ATTRIBUTES = 2;
const IX_REPLACE_ATTRIBUTES = 3;
const IX_TRANSFER_AUTHORITY = 4;
const IX_MAKE_IMMUTABLE = 5;

/**
 * Client-side helpers for Arch Token Metadata.
 * - PDA helpers (metadata / attributes)
 * - Instruction builders with client-side validation
 * - Transaction builders that compose upstream system/token instructions
 */
export class TokenMetadataClient {
  readonly programId: Pubkey;
  constructor(programId: Pubkey) {
    this.programId = programId;
  }

  // PDA helpers
  /** Derive the metadata PDA for a given mint. */
  metadataPda(mint: Pubkey): Pubkey {
    const seed = new TextEncoder().encode("metadata");
    return findProgramAddress([seed, mint], this.programId)[0];
  }

  /** Derive the attributes PDA for a given mint. */
  attributesPda(mint: Pubkey): Pubkey {
    const seed = new TextEncoder().encode("attributes");
    return findProgramAddress([seed, mint], this.programId)[0];
  }

  // Validation
  private validateMetadataFields(
    name: string,
    symbol: string,
    image: string,
    description: string
  ) {
    if (name.length > NAME_MAX_LEN) throw new Error("name too long");
    if (symbol.length > SYMBOL_MAX_LEN) throw new Error("symbol too long");
    if (image.length > IMAGE_MAX_LEN) throw new Error("image too long");
    if (description.length > DESCRIPTION_MAX_LEN)
      throw new Error("description too long");
  }
  private validateOptionalMetadataFields(params: {
    name?: string;
    symbol?: string;
    image?: string;
    description?: string;
  }) {
    if (params.name && params.name.length > NAME_MAX_LEN)
      throw new Error("name too long");
    if (params.symbol && params.symbol.length > SYMBOL_MAX_LEN)
      throw new Error("symbol too long");
    if (params.image && params.image.length > IMAGE_MAX_LEN)
      throw new Error("image too long");
    if (params.description && params.description.length > DESCRIPTION_MAX_LEN)
      throw new Error("description too long");
  }
  private validateAttributes(data: Array<[string, string]>) {
    if (data.length > MAX_ATTRIBUTES) throw new Error("too many attributes");
    for (const [k, v] of data) {
      if (k.length > MAX_KEY_LENGTH) throw new Error("attribute key too long");
      if (v.length > MAX_VALUE_LENGTH)
        throw new Error("attribute value too long");
    }
  }

  // Instruction builders
  /** Build a CreateMetadata instruction. */
  createMetadataIx(params: CreateMetadataParams): Instruction {
    const metadataPda = this.metadataPda(params.mint);
    this.validateMetadataFields(
      params.name,
      params.symbol,
      params.image,
      params.description
    );
    const variant = new Uint8Array([IX_CREATE_METADATA]);
    const body = concat([
      encodeString(params.name),
      encodeString(params.symbol),
      encodeString(params.image),
      encodeString(params.description),
      new Uint8Array([params.immutable ? 1 : 0]),
    ]);
    return {
      programId: this.programId,
      accounts: [
        { pubkey: params.payer, isSigner: true, isWritable: true },
        { pubkey: systemProgram(), isSigner: false, isWritable: false },
        { pubkey: params.mint, isSigner: false, isWritable: false },
        { pubkey: metadataPda, isSigner: false, isWritable: true },
        {
          pubkey: params.mintOrFreezeAuthority,
          isSigner: true,
          isWritable: false,
        },
      ],
      data: concat([variant, body]),
    };
  }

  /** Build an UpdateMetadata instruction. */
  updateMetadataIx(params: UpdateMetadataParams): Instruction {
    const metadataPda = this.metadataPda(params.mint);
    this.validateOptionalMetadataFields(params);
    const variant = new Uint8Array([IX_UPDATE_METADATA]);
    const body = concat([
      encodeOptionString(params.name),
      encodeOptionString(params.symbol),
      encodeOptionString(params.image),
      encodeOptionString(params.description),
    ]);
    return {
      programId: this.programId,
      accounts: [
        { pubkey: metadataPda, isSigner: false, isWritable: true },
        { pubkey: params.updateAuthority, isSigner: true, isWritable: false },
      ],
      data: concat([variant, body]),
    };
  }

  /** Build a CreateAttributes instruction. */
  createAttributesIx(params: CreateAttributesParams): Instruction {
    const metadataPda = this.metadataPda(params.mint);
    const attributesPda = this.attributesPda(params.mint);
    this.validateAttributes(params.data);
    const variant = new Uint8Array([IX_CREATE_ATTRIBUTES]);
    const body = encodeVecTupleStringString(params.data);
    return {
      programId: this.programId,
      accounts: [
        { pubkey: params.payer, isSigner: true, isWritable: true },
        { pubkey: systemProgram(), isSigner: false, isWritable: false },
        { pubkey: params.mint, isSigner: false, isWritable: false },
        { pubkey: attributesPda, isSigner: false, isWritable: true },
        { pubkey: params.updateAuthority, isSigner: true, isWritable: false },
        { pubkey: metadataPda, isSigner: false, isWritable: false },
      ],
      data: concat([variant, body]),
    };
  }

  /** Build a ReplaceAttributes instruction. */
  replaceAttributesIx(params: ReplaceAttributesParams): Instruction {
    const metadataPda = this.metadataPda(params.mint);
    const attributesPda = this.attributesPda(params.mint);
    this.validateAttributes(params.data);
    const variant = new Uint8Array([IX_REPLACE_ATTRIBUTES]);
    const body = encodeVecTupleStringString(params.data);
    return {
      programId: this.programId,
      accounts: [
        { pubkey: attributesPda, isSigner: false, isWritable: true },
        { pubkey: params.updateAuthority, isSigner: true, isWritable: false },
        { pubkey: metadataPda, isSigner: false, isWritable: false },
      ],
      data: concat([variant, body]),
    };
  }

  /** Build a TransferAuthority instruction. */
  transferAuthorityIx(params: TransferAuthorityParams): Instruction {
    const metadataPda = this.metadataPda(params.mint);
    const variant = new Uint8Array([IX_TRANSFER_AUTHORITY]);
    const body = new Uint8Array(params.newAuthority);
    return {
      programId: this.programId,
      accounts: [
        { pubkey: metadataPda, isSigner: false, isWritable: true },
        {
          pubkey: params.currentUpdateAuthority,
          isSigner: true,
          isWritable: false,
        },
      ],
      data: concat([variant, body]),
    };
  }

  /** Build a MakeImmutable instruction. */
  makeImmutableIx(params: MakeImmutableParams): Instruction {
    const metadataPda = this.metadataPda(params.mint);
    const variant = new Uint8Array([IX_MAKE_IMMUTABLE]);
    return {
      programId: this.programId,
      accounts: [
        { pubkey: metadataPda, isSigner: false, isWritable: true },
        {
          pubkey: params.currentUpdateAuthority,
          isSigner: true,
          isWritable: false,
        },
      ],
      data: variant,
    };
  }

  // Transaction helpers (metadata-only compositions)
  /** One-instruction Vec wrapper for CreateAttributes. */
  createAttributesTx(params: CreateAttributesParams): Instruction[] {
    return [this.createAttributesIx(params)];
  }
  /** One-instruction Vec wrapper for ReplaceAttributes. */
  replaceAttributesTx(params: ReplaceAttributesParams): Instruction[] {
    return [this.replaceAttributesIx(params)];
  }
  /** One-instruction Vec wrapper for MakeImmutable. */
  makeImmutableTx(params: MakeImmutableParams): Instruction[] {
    return [this.makeImmutableIx(params)];
  }
  /** Transfer authority then immediately update metadata (two instructions). */
  transferAuthorityThenUpdateTx(
    params: TransferAuthorityThenUpdateParams
  ): Instruction[] {
    const transfer = this.transferAuthorityIx({
      mint: params.mint,
      currentUpdateAuthority: params.currentUpdateAuthority,
      newAuthority: params.newAuthority,
    });
    const update = this.updateMetadataIx({
      mint: params.mint,
      updateAuthority: params.newAuthority,
      name: params.name,
      symbol: params.symbol,
      image: params.image,
      description: params.description,
    });
    return [transfer, update];
  }

  // Transaction builders that compose external mint/system instructions with our metadata instructions.
  // Callers provide the mint initialization instructions built elsewhere.

  /** Upstream mint init instructions + CreateMetadata. */
  createTokenWithMetadataTx(
    params: TxCreateTokenWithMetadataParams
  ): Instruction[] {
    const createMd = this.createMetadataIx({
      payer: params.payer,
      mint: params.mint,
      mintOrFreezeAuthority: params.mintAuthority,
      name: params.name,
      symbol: params.symbol,
      image: params.image,
      description: params.description,
      immutable: params.immutable,
    });
    return [...params.mintInitializeInstructions, createMd];
  }

  /** Same as createTokenWithMetadataTx plus returning PDAs. */
  createTokenWithMetadataTxWithPdas(params: TxCreateTokenWithMetadataParams): {
    instructions: Instruction[];
    pdas: DerivedPdas;
  } {
    const mdPda = this.metadataPda(params.mint);
    const instructions = this.createTokenWithMetadataTx(params);
    return {
      instructions,
      pdas: { metadataPda: mdPda, attributesPda: undefined },
    };
  }

  /** Upstream mint init instructions + CreateMetadata + CreateAttributes. */
  createTokenWithMetadataAndAttributesTx(
    params: TxCreateTokenWithMetadataAndAttributesParams
  ): Instruction[] {
    const createMd = this.createMetadataIx({
      payer: params.payer,
      mint: params.mint,
      mintOrFreezeAuthority: params.mintAuthority,
      name: params.name,
      symbol: params.symbol,
      image: params.image,
      description: params.description,
      immutable: params.immutable,
    });
    const createAttrs = this.createAttributesIx({
      payer: params.payer,
      mint: params.mint,
      updateAuthority: params.mintAuthority,
      data: params.attributes,
    });
    return [...params.mintInitializeInstructions, createMd, createAttrs];
  }

  /** Same as createTokenWithMetadataAndAttributesTx plus returning PDAs. */
  createTokenWithMetadataAndAttributesTxWithPdas(
    params: TxCreateTokenWithMetadataAndAttributesParams
  ): { instructions: Instruction[]; pdas: DerivedPdas } {
    const mdPda = this.metadataPda(params.mint);
    const attrsPda = this.attributesPda(params.mint);
    const instructions = this.createTokenWithMetadataAndAttributesTx(params);
    return {
      instructions,
      pdas: { metadataPda: mdPda, attributesPda: attrsPda },
    };
  }

  /** Upstream init, then clear MintTokens authority, then CreateMetadata signed by freeze authority. */
  createTokenWithFreezeAuthMetadataTx(
    params: TxCreateTokenWithFreezeAuthMetadataParams
  ): Instruction[] {
    const createMd = this.createMetadataIx({
      payer: params.payer,
      mint: params.mint,
      mintOrFreezeAuthority: params.freezeAuthority,
      name: params.name,
      symbol: params.symbol,
      image: params.image,
      description: params.description,
      immutable: params.immutable,
    });
    return [
      ...params.mintInitializeInstructions,
      params.clearMintAuthorityInstruction,
      createMd,
    ];
  }

  // Upstream APL Token helpers (data-only; caller provides accounts/signers)
  /** SystemProgram create_account to allocate a mint account (Mint::LEN bytes). */
  private systemCreateAccountIx(
    payer: Pubkey,
    newAccount: Pubkey,
    lamports: bigint,
    space: bigint,
    owner: Pubkey
  ): Instruction {
    // system program: discriminant=0, then lamports u64 LE, space u64 LE, owner 32B
    const tag = new Uint8Array(4);
    const dv = new DataView(tag.buffer);
    dv.setUint32(0, 0, true);
    const lam = new Uint8Array(8);
    new DataView(lam.buffer).setBigUint64(0, lamports, true);
    const sp = new Uint8Array(8);
    new DataView(sp.buffer).setBigUint64(0, space, true);
    const data = concat([tag, lam, sp, new Uint8Array(owner)]);
    return {
      programId: systemProgram(),
      accounts: [
        { pubkey: payer, isSigner: true, isWritable: true },
        { pubkey: newAccount, isSigner: true, isWritable: true },
      ],
      data,
    };
  }

  /** Convenience: create mint account with correct space for Mint. */
  createMintAccountIx(
    payer: Pubkey,
    mint: Pubkey,
    tokenProgramId: Pubkey,
    minAccountLamports: bigint
  ): Instruction {
    // size = apl_token::state::Mint::LEN = 82
    const mintSpace = 82n;
    return this.systemCreateAccountIx(
      payer,
      mint,
      minAccountLamports,
      mintSpace,
      tokenProgramId
    );
  }
  /** Token initialize_mint2 instruction data. */
  tokenInitializeMint2Ix(
    tokenProgramId: Pubkey,
    mint: Pubkey,
    mintAuthority: Pubkey,
    freezeAuthority: Pubkey | undefined,
    decimals: number
  ): Instruction {
    const tag = new Uint8Array([18]);
    const dec = new Uint8Array([decimals & 0xff]);
    const body = concat([
      dec,
      new Uint8Array(mintAuthority),
      freezeAuthority
        ? concat([new Uint8Array([1]), new Uint8Array(freezeAuthority)])
        : new Uint8Array([0]),
    ]);
    return {
      programId: tokenProgramId,
      accounts: [{ pubkey: mint, isSigner: false, isWritable: true }],
      data: concat([tag, body]),
    };
  }

  /** Token set_authority(MintTokens) instruction data. */
  tokenSetMintAuthorityIx(
    tokenProgramId: Pubkey,
    mint: Pubkey,
    currentAuthority: Pubkey,
    newAuthority: Pubkey | undefined
  ): Instruction {
    const tag = new Uint8Array([6]);
    const authorityTypeMintTokens = new Uint8Array([0]);
    const newAuthBytes = newAuthority
      ? concat([new Uint8Array([1]), new Uint8Array(newAuthority)])
      : new Uint8Array([0]);
    const data = concat([tag, authorityTypeMintTokens, newAuthBytes]);
    return {
      programId: tokenProgramId,
      accounts: [
        { pubkey: mint, isSigner: false, isWritable: true },
        { pubkey: currentAuthority, isSigner: true, isWritable: false },
      ],
      data,
    };
  }
}

/** Arch Token Metadata program id as Pubkey. */
export function metadataProgramId(): Pubkey {
  return new Uint8Array(Buffer.from("arch-metadata000000000000000000")).slice(
    0,
    32
  ) as Pubkey;
}
/** APL Token program id as Pubkey. */
export function tokenProgramId(): Pubkey {
  return new Uint8Array(Buffer.from("apl-token00000000000000000000000")).slice(
    0,
    32
  ) as Pubkey;
}

// Minimal stand-in for System Program id
// System program helper exported for convenience
export { systemProgram } from "./serde/pubkey.js";

// Params
export interface CreateMetadataParams {
  payer: Pubkey;
  mint: Pubkey;
  mintOrFreezeAuthority: Pubkey;
  name: string;
  symbol: string;
  image: string;
  description: string;
  immutable: boolean;
}
export interface UpdateMetadataParams {
  mint: Pubkey;
  updateAuthority: Pubkey;
  name?: string;
  symbol?: string;
  image?: string;
  description?: string;
}
export interface CreateAttributesParams {
  payer: Pubkey;
  mint: Pubkey;
  updateAuthority: Pubkey;
  data: Array<[string, string]>;
}
export interface ReplaceAttributesParams {
  mint: Pubkey;
  updateAuthority: Pubkey;
  data: Array<[string, string]>;
}
export interface TransferAuthorityParams {
  mint: Pubkey;
  currentUpdateAuthority: Pubkey;
  newAuthority: Pubkey;
}
export interface MakeImmutableParams {
  mint: Pubkey;
  currentUpdateAuthority: Pubkey;
}

export interface TransferAuthorityThenUpdateParams {
  mint: Pubkey;
  currentUpdateAuthority: Pubkey;
  newAuthority: Pubkey;
  name?: string;
  symbol?: string;
  image?: string;
  description?: string;
}

// Derived PDA info returned by tx builders
export interface DerivedPdas {
  metadataPda: Pubkey;
  attributesPda?: Pubkey;
}

// Transaction params for composing external mint init instructions with metadata flows
export interface TxCreateTokenWithMetadataParams {
  payer: Pubkey;
  mint: Pubkey;
  mintAuthority: Pubkey;
  freezeAuthority?: Pubkey;
  decimals: number;
  name: string;
  symbol: string;
  image: string;
  description: string;
  immutable: boolean;
  // Provided by caller (e.g., SystemProgram create_account + Token initialize_mint2)
  mintInitializeInstructions: Instruction[];
}

export interface TxCreateTokenWithMetadataAndAttributesParams
  extends Omit<TxCreateTokenWithMetadataParams, "mintInitializeInstructions"> {
  attributes: Array<[string, string]>;
  mintInitializeInstructions: Instruction[];
}

export interface TxCreateTokenWithFreezeAuthMetadataParams {
  payer: Pubkey;
  mint: Pubkey;
  initialMintAuthority: Pubkey;
  freezeAuthority: Pubkey;
  decimals: number;
  name: string;
  symbol: string;
  image: string;
  description: string;
  immutable: boolean;
  mintInitializeInstructions: Instruction[];
  clearMintAuthorityInstruction: Instruction;
}
