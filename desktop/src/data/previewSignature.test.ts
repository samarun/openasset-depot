import { describe, expect, it } from "vitest";
import { previewPatternClass, previewSignature } from "./previewSignature";

describe("previewSignature", () => {
  it("returns the same signature for the same path on every call", () => {
    const first = previewSignature("Content/Maps/Main.umap");
    const second = previewSignature("Content/Maps/Main.umap");

    expect(first).toEqual(second);
  });

  it("gives different assets different hues", () => {
    const main = previewSignature("Content/Maps/Main.umap");
    const ship = previewSignature("Content/Vehicles/HeroShip.uasset");

    expect(main.hue).not.toBe(ship.hue);
  });

  it("gives one extension a single pattern so a file type reads as a family", () => {
    // The pattern keys off the extension alone, so every .blend shares it
    // regardless of where the file sits in the depot.
    expect(previewPatternClass("Scenes/Lobby.blend")).toBe(previewPatternClass("Scenes/Attic.blend"));
    expect(previewPatternClass("Scenes/Lobby.blend")).toBe(previewPatternClass("deep/nested/Other.blend"));
  });

  it("spreads common extensions across the available patterns", () => {
    const patterns = new Set(
      ["a.blend", "a.uasset", "a.nk", "a.wav", "a.psd", "a.ma", "a.fbx", "a.umap"]
        .map(previewPatternClass),
    );

    expect(patterns.size).toBeGreaterThan(1);
  });

  it("keeps every derived value inside its renderable range", () => {
    for (const path of ["a.blend", "deep/nested/path/Asset.uasset", "no-extension", "Audio/Ambience.wav"]) {
      const { hue, accentHue, rotation, pattern } = previewSignature(path);
      expect(hue).toBeGreaterThanOrEqual(0);
      expect(hue).toBeLessThan(360);
      expect(accentHue).toBeGreaterThanOrEqual(0);
      expect(accentHue).toBeLessThan(360);
      expect(rotation).toBeGreaterThanOrEqual(-12);
      expect(rotation).toBeLessThanOrEqual(12);
      expect(pattern).toBeGreaterThanOrEqual(1);
      expect(pattern).toBeLessThanOrEqual(4);
    }
  });
});
