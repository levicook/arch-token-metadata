import { sha256 } from "@noble/hashes/sha256";
import { secp256k1 } from "@noble/curves/secp256k1";

export type Pubkey = Uint8Array; // 32 bytes

export const MAX_SEED_LENGTH = 32;
export const MAX_SEEDS = 16;

export function systemProgram(): Pubkey {
  const tmp = new Uint8Array(32);
  tmp[31] = 1;
  return tmp as Pubkey;
}

export function findProgramAddress(
  seeds: Array<Uint8Array>,
  programId: Pubkey,
): [Pubkey, number] {
  if (seeds.length > MAX_SEEDS) throw new Error("Max seeds exceeded");
  let nonce = 255;
  while (nonce !== 0) {
    const seedsWithNonce = [...seeds, new Uint8Array([nonce])];
    try {
      const address = createProgramAddress(seedsWithNonce, programId);
      return [address, nonce];
    } catch (e) {
      if (e instanceof TypeError) throw e;
      if (
        e instanceof Error &&
        e.message === "Invalid seeds, address must fall off the curve"
      ) {
        nonce--;
        continue;
      }
      throw e;
    }
  }
  throw new Error("Unable to find a viable program address nonce");
}

function createProgramAddress(
  seeds: Array<Uint8Array>,
  programId: Pubkey,
): Pubkey {
  if (seeds.length > MAX_SEEDS) throw new Error("Max seeds exceeded");
  let buffer = new Uint8Array(0);
  for (const seed of seeds) {
    if (seed.length > MAX_SEED_LENGTH)
      throw new Error("Max seed length exceeded");
    const concat = new Uint8Array(buffer.length + seed.length);
    concat.set(buffer, 0);
    concat.set(seed, buffer.length);
    buffer = concat;
  }
  const withProgram = new Uint8Array(buffer.length + programId.length);
  withProgram.set(buffer, 0);
  withProgram.set(programId, buffer.length);

  const hash = sha256(withProgram);
  if (isOnCurve(hash))
    throw new Error("Invalid seeds, address must fall off the curve");
  return hash;
}

function isOnCurve(pubkey: Pubkey): boolean {
  try {
    // noble expects 33/65, but will throw on invalid sizes, which we treat as off-curve
    secp256k1.ProjectivePoint.fromHex(pubkey);
    return true;
  } catch (_) {
    return false;
  }
}
