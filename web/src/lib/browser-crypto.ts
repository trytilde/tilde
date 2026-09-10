// Remote HTTP development origins lack SubtleCrypto/randomUUID, but still expose getRandomValues.
import { sha256 } from "@noble/hashes/sha2.js";

export function pkceChallenge(verifier: string): string {
  return btoa(String.fromCharCode(...sha256(new TextEncoder().encode(verifier))))
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replaceAll("=", "");
}

export function randomUUID(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
