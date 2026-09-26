import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

installMemoryStorage("localStorage");
installMemoryStorage("sessionStorage");

afterEach(() => {
  cleanup();
});

function installMemoryStorage(name: "localStorage" | "sessionStorage") {
  const values = new Map<string, string>();
  const storage: Storage = {
    get length() {
      return values.size;
    },
    clear() {
      values.clear();
    },
    getItem(key) {
      return values.get(key) ?? null;
    },
    key(index) {
      return [...values.keys()][index] ?? null;
    },
    removeItem(key) {
      values.delete(key);
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
  };

  Object.defineProperty(globalThis, name, {
    configurable: true,
    value: storage,
  });
}
