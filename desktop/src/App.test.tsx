import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

describe("App", () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders the login screen without a stored session", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Sign in" })).toBeInTheDocument();
  });

  it("uses the compact demo status when the server cannot be reached", async () => {
    sessionStorage.setItem("oad.session", JSON.stringify({
      token: "token",
      username: "artist",
      serverUrl: "http://server",
    }));
    vi.stubGlobal("fetch", vi.fn(async () => jsonResponse({ message: "Bad gateway" }, 502)));

    render(<App />);

    expect(await screen.findByText("Demo Mode")).toBeInTheDocument();
    expect(screen.getByText("Server not connected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry Connection" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Settings" })).toBeInTheDocument();
  });

  it("shows only live data labels after a successful backend connection", async () => {
    sessionStorage.setItem("oad.session", JSON.stringify({
      token: "token",
      username: "artist",
      serverUrl: "http://server",
    }));
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/api/workspaces")) {
        return jsonResponse([{
          id: "workspace-1",
          depot_id: "depot-1",
          stream_id: "stream-1",
          name: "Live Workspace",
          local_path: "/Projects/Live",
        }]);
      }
      if (url.endsWith("/api/depots")) return jsonResponse([{ id: "depot-1", name: "Live Depot" }]);
      if (url.includes("/api/locks/page")) {
        return jsonResponse({ items: [], next_before_created_at: null, next_before_id: null });
      }
      if (url.endsWith("/api/streams") || url.endsWith("/api/filetypes") || url.endsWith("/api/sync/plan")) {
        return jsonResponse([]);
      }
      return jsonResponse({ message: "not found" }, 404);
    });
    vi.stubGlobal("fetch", fetcher);

    render(<App />);

    expect(await screen.findByRole("heading", { name: "Live Workspace" })).toBeInTheDocument();
    expect(await screen.findByText("Your asset workspace is ready")).toBeInTheDocument();
    expect(screen.queryByText("Demo Mode")).not.toBeInTheDocument();
    expect(screen.queryByText("Mock")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(fetcher).toHaveBeenCalledWith(
        "http://server/api/sync/plan",
        expect.objectContaining({ method: "POST" }),
      );
    });
  });
});

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}
