import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Login } from "./Login";

const signInWithSso = vi.hoisted(() => vi.fn());
vi.mock("../native/sso", () => ({ signInWithSso }));

describe("Login", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    signInWithSso.mockReset();
    delete tauriGlobals().__TAURI_INTERNALS__;
  });

  it("creates an enabled signup account and signs it in", async () => {
    const onLogin = vi.fn();
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.endsWith("/api/auth/signup") && (!init?.method || init.method === "GET")) {
        return jsonResponse({ enabled: true, first_user: false });
      }
      if (url.endsWith("/api/auth/signup")) {
        return jsonResponse({ id: "user-1", username: "artist", is_admin: false });
      }
      return jsonResponse({ token: "token", user: { username: "artist", is_admin: false } });
    }));

    render(<Login onLogin={onLogin} />);
    fireEvent.click(screen.getByRole("tab", { name: "Sign up" }));
    await screen.findByLabelText("Display name");
    fireEvent.change(screen.getByLabelText("Display name"), { target: { value: "Artist One" } });
    fireEvent.change(screen.getByLabelText("Username"), { target: { value: "artist" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "long-password" } });
    fireEvent.change(screen.getByLabelText("Confirm password"), { target: { value: "long-password" } });
    await waitFor(() => expect(screen.getByRole("button", { name: "Create account" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Create account" }));

    await waitFor(() => expect(onLogin).toHaveBeenCalledWith(expect.objectContaining({ username: "artist" })));
  });

  it("blocks sign in while the server cannot be reached", async () => {
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).endsWith("/ready")) throw new TypeError("network error");
      return jsonResponse({ token: "token", user: { username: "artist", is_admin: false } });
    }));

    render(<Login onLogin={vi.fn()} />);

    await screen.findByText("Cannot reach this server");
    expect(screen.getByRole("button", { name: /Enter workspace/ })).toBeDisabled();
  });

  it("enables sign in once /ready answers", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => jsonResponse({ status: "ready" })));

    render(<Login onLogin={vi.fn()} />);

    await screen.findByText("Server reachable");
    expect(screen.getByRole("button", { name: /Enter workspace/ })).toBeEnabled();
  });

  it("signs in through the configured identity provider", async () => {
    tauriGlobals().__TAURI_INTERNALS__ = {};
    const onLogin = vi.fn();
    signInWithSso.mockResolvedValue({
      token: "sso-token",
      username: "artist",
      serverUrl: "http://127.0.0.1:8080",
      isAdmin: false,
    });
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).endsWith("/api/auth/sso")) {
        return jsonResponse({ enabled: true, provider: "studio.okta.com" });
      }
      return jsonResponse({ status: "ready" });
    }));

    render(<Login onLogin={onLogin} />);
    fireEvent.click(await screen.findByRole("button", { name: /Continue with studio.okta.com/ }));

    await waitFor(() => expect(onLogin).toHaveBeenCalledWith(expect.objectContaining({ token: "sso-token" })));
  });

  it("hides single sign-on when the server has no provider configured", async () => {
    tauriGlobals().__TAURI_INTERNALS__ = {};
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).endsWith("/api/auth/sso")) return jsonResponse({ enabled: false, provider: null });
      return jsonResponse({ status: "ready" });
    }));

    render(<Login onLogin={vi.fn()} />);

    await screen.findByText("Server reachable");
    expect(screen.queryByRole("button", { name: /Continue with/ })).toBeNull();
  });

  it("reports the reason an interrupted single sign-on failed", async () => {
    tauriGlobals().__TAURI_INTERNALS__ = {};
    signInWithSso.mockRejectedValue(new Error("Timed out waiting for the sign-in redirect."));
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).endsWith("/api/auth/sso")) {
        return jsonResponse({ enabled: true, provider: "studio.okta.com" });
      }
      return jsonResponse({ status: "ready" });
    }));

    render(<Login onLogin={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: /Continue with studio.okta.com/ }));

    await screen.findByText("Timed out waiting for the sign-in redirect.");
  });
});

/** Lets a test pose as the native app, which is how `isNativeDesktop` decides. */
function tauriGlobals(): Record<string, unknown> {
  return window as unknown as Record<string, unknown>;
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}
