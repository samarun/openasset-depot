import { afterEach, describe, expect, it, vi } from "vitest";
import { createUuid } from "./uuid";

afterEach(() => vi.unstubAllGlobals());

describe("createUuid", () => {
  it("uses native randomUUID when available", () => {
    vi.stubGlobal("crypto", { randomUUID: () => "11111111-1111-4111-8111-111111111111" });
    expect(createUuid()).toBe("11111111-1111-4111-8111-111111111111");
  });

  it("works when an HTTP origin does not expose randomUUID", () => {
    vi.stubGlobal("crypto", {
      getRandomValues: (values: Uint8Array) => {
        values.fill(0xab);
        return values;
      },
    });
    expect(createUuid()).toBe("abababab-abab-4bab-abab-abababababab");
  });
});
