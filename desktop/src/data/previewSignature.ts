import type { CSSProperties } from "react";

/** Number of distinct geometric overlays a generated placeholder can use. */
const PATTERN_VARIANTS = 4;

export interface PreviewSignature {
  /** Primary hue, unique per depot path. */
  hue: number;
  /** Companion hue derived from the file extension so a type reads as a family. */
  accentHue: number;
  rotation: number;
  /** 1-based overlay variant, used to pick a CSS geometry. */
  pattern: number;
}

/**
 * Derives a stable visual signature for an asset that has no real thumbnail.
 *
 * The output is deliberately abstract: it must never be mistaken for a render
 * of the actual file. It is deterministic so an asset looks the same on every
 * machine and across sessions, and the accent hue keys off the extension so
 * that all `.blend` files share a family resemblance while individual files
 * stay distinguishable.
 */
export function previewSignature(path: string): PreviewSignature {
  const extension = extensionOf(path);
  const pathHash = stableHash(path);
  const extensionHash = stableHash(extension);

  return {
    hue: pathHash % 360,
    accentHue: (pathHash + 40 + (extensionHash % 90)) % 360,
    rotation: (pathHash % 24) - 12,
    pattern: (extensionHash % PATTERN_VARIANTS) + 1,
  };
}

/** Maps a signature onto the custom properties consumed by `.asset-preview`. */
export function previewSignatureStyle(path: string): CSSProperties {
  const { hue, accentHue, rotation, pattern } = previewSignature(path);
  return {
    "--preview-hue": hue,
    "--preview-accent-hue": accentHue,
    "--preview-rotation": `${rotation}deg`,
    "--preview-pattern": pattern,
  } as CSSProperties;
}

export function previewPatternClass(path: string): string {
  return `asset-preview-pattern-${previewSignature(path).pattern}`;
}

function extensionOf(path: string): string {
  const fileName = path.split("/").at(-1) ?? path;
  const dot = fileName.lastIndexOf(".");
  return dot >= 0 ? fileName.slice(dot + 1).toLowerCase() : "";
}

function stableHash(value: string): number {
  let hash = 0;
  for (const character of value) hash = (hash * 31 + character.charCodeAt(0)) | 0;
  return Math.abs(hash);
}
