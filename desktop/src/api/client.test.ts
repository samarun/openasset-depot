import { describe, expect, it, vi } from "vitest";
import { ApiError, friendlyApiError, isConnectionError, OpenAssetApiClient } from "./client";

describe("OpenAssetApiClient", () => {
  it("logs in and returns a session", async () => {
    const fetcher = vi.fn(async () => jsonResponse({ token: "abc", user: { username: "maya", is_admin: true } }));
    const client = new OpenAssetApiClient("http://server/", fetcher as unknown as typeof fetch);

    await expect(client.login("maya", "password")).resolves.toEqual({
      token: "abc",
      username: "maya",
      serverUrl: "http://server",
      isAdmin: true,
    });
    expect(fetcher).toHaveBeenCalledWith(
      "http://server/api/auth/login",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("maps API errors to friendly messages", async () => {
    const fetcher = vi.fn(async () =>
      jsonResponse({ code: "conflict", message: "file is already locked" }, 409),
    );
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    await expect(client.listLocks("token")).rejects.toBeInstanceOf(ApiError);
    try {
      await client.listLocks("token");
    } catch (error) {
      expect(friendlyApiError(error)).toContain("another artist");
    }
  });

  it("collects lock pages without repeating cursors", async () => {
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("before_id=lock-1")) {
        return jsonResponse({
          items: [{ id: "lock-2", user_id: "user", depot_path: "B.blend", state: "active" }],
          next_before_created_at: null,
          next_before_id: null,
        });
      }
      return jsonResponse({
        items: [{ id: "lock-1", user_id: "user", depot_path: "A.blend", state: "active" }],
        next_before_created_at: "2026-06-27T12:00:00Z",
        next_before_id: "lock-1",
      });
    });
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    const locks = await client.listLocks("token");

    expect(locks.map((lock) => lock.id)).toEqual(["lock-1", "lock-2"]);
    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  it("recognizes direct and gateway connection failures", () => {
    expect(isConnectionError(new TypeError("Failed to fetch"))).toBe(true);
    expect(isConnectionError(new ApiError("Bad gateway", 502))).toBe(true);
    expect(isConnectionError(new ApiError("Validation failed", 400))).toBe(false);
  });

  it("calls adapter endpoints with typed request bodies", async () => {
    const fetcher = vi.fn(async () => jsonResponse([]));
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    await client.detectProject("token", "/show/game", ["Game.uproject"]);

    expect(fetcher).toHaveBeenCalledWith(
      "http://server/api/adapters/detect",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ root_path: "/show/game", relative_paths: ["Game.uproject"] }),
      }),
    );
  });

  it("calls sync, lock, and history endpoints with workspace context", async () => {
    const fetcher = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => jsonResponse([]));
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    await client.planSync("token", "workspace-1");
    await client.lockFile("token", "workspace-1", "Content/Hero.uasset", "Lookdev");
    await client.fileHistory("token", "workspace-1", "Content/Hero.uasset", 25);

    expect(fetcher).toHaveBeenNthCalledWith(
      1,
      "http://server/api/sync/plan",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ workspace_id: "workspace-1" }),
      }),
    );
    expect(fetcher).toHaveBeenNthCalledWith(
      2,
      "http://server/api/files/lock",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          workspace_id: "workspace-1",
          path: "Content/Hero.uasset",
          reason: "Lookdev",
        }),
      }),
    );
    expect(fetcher.mock.calls[2][0]).toContain(
      "/api/files/history?workspace_id=workspace-1&path=Content%2FHero.uasset&limit=25",
    );
  });

  it("creates depots and streams with idempotency keys", async () => {
    const fetcher = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
      jsonResponse({ id: "created", name: "main", depot_id: "depot-1" }),
    );
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    await client.createDepot("token", "Cinematic", "Feature production");
    await client.createStream("token", "main", "depot-1");

    expect(fetcher).toHaveBeenNthCalledWith(
      1,
      "http://server/api/depots",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ name: "Cinematic", description: "Feature production" }),
        headers: expect.objectContaining({}),
      }),
    );
    expect((fetcher.mock.calls[0][1]?.headers as Headers).get("idempotency-key")).toBeTruthy();
    expect(fetcher).toHaveBeenNthCalledWith(
      2,
      "http://server/api/streams",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ name: "main", depot: "depot-1" }),
      }),
    );
    expect((fetcher.mock.calls[1][1]?.headers as Headers).get("idempotency-key")).toBeTruthy();
  });

  it("creates and removes workspaces through the lifecycle endpoints", async () => {
    let requestNumber = 0;
    const fetcher = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => {
      requestNumber += 1;
      return requestNumber === 1
        ? jsonResponse({
            id: "workspace-1",
            depot_id: "depot-1",
            stream_id: "stream-1",
            name: "shots",
            local_path: "/show/shots",
          })
        : jsonResponse({
            id: "workspace-1",
            deleted: true,
            released_locks: 1,
            abandoned_changelists: 1,
          });
    });
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    await client.createWorkspace("token", {
      name: "shots",
      depot: "depot-1",
      stream: "stream-1",
      local_path: "/show/shots",
    });
    await client.deleteWorkspace("token", "workspace-1");

    expect(fetcher).toHaveBeenNthCalledWith(
      1,
      "http://server/api/workspaces",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          name: "shots",
          depot: "depot-1",
          stream: "stream-1",
          local_path: "/show/shots",
        }),
      }),
    );
    expect((fetcher.mock.calls[0][1]?.headers as Headers).get("idempotency-key")).toBeTruthy();
    expect(fetcher).toHaveBeenNthCalledWith(
      2,
      "http://server/api/workspaces/workspace-1",
      expect.objectContaining({ method: "DELETE" }),
    );
  });
});

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}
