import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Login } from "./Login";

describe("Login", () => {
  afterEach(() => vi.unstubAllGlobals());

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
});

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}
