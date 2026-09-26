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

  it("supports signup and authenticated password changes", async () => {
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/api/auth/signup")) {
        return jsonResponse({ id: "user-1", username: "artist", is_admin: false });
      }
      return jsonResponse({ changed: true });
    });
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    await expect(client.signup("artist", "long-password", "Artist One")).resolves.toMatchObject({
      username: "artist",
    });
    await expect(client.changePassword("token", "long-password", "new-long-password")).resolves.toEqual({
      changed: true,
    });
    expect(fetcher).toHaveBeenNthCalledWith(
      2,
      "http://server/api/auth/change-password",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("submits browser files as multipart data without a JSON content type", async () => {
    const fetcher = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
      jsonResponse({ changelist_id: "change-1", revisions: [] }),
    );
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);
    const upload = new File(["asset data"], "Hero.txt", { type: "text/plain" });

    await client.submitChangelist("token", "workspace-1", "change-1", [
      { path: "Assets/Hero.txt", file: upload, action: "add" },
    ]);

    const request = fetcher.mock.calls[0][1] as RequestInit;
    expect(request.body).toBeInstanceOf(FormData);
    expect((request.headers as Headers).has("content-type")).toBe(false);
    expect((request.headers as Headers).get("authorization")).toBe("Bearer token");
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

  it("downloads an authenticated immutable revision preview", async () => {
    const fetcher = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => new Response("preview", {
      status: 200,
      headers: { "content-type": "image/png" },
    }));
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    const preview = await client.downloadPreview("token", "workspace-1", "Scenes/Shot.blend", 3);

    expect(preview?.size).toBe(7);
    expect(fetcher.mock.calls[0][0]).toContain(
      "/api/files/preview?workspace_id=workspace-1&path=Scenes%2FShot.blend&revision_number=3",
    );
    expect((fetcher.mock.calls[0][1]?.headers as Record<string, string>).authorization).toBe(
      "Bearer token",
    );
  });

  it("loads review media and creates revision-bound comments", async () => {
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("/api/reviews/media")) {
        return new Response("model", { status: 200, headers: { "content-type": "model/gltf-binary" } });
      }
      return jsonResponse({
        id: "comment-1",
        path: "Models/Hero.glb",
        revision_number: 4,
        author_user_id: "user-1",
        author: "Artist",
        body: "Adjust the arc",
        created_at: "2026-06-30T10:00:00Z",
      });
    });
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    const media = await client.downloadReviewMedia("token", "workspace-1", "Models/Hero.glb", 4);
    await client.createReviewComment("token", {
      workspace_id: "workspace-1",
      path: "Models/Hero.glb",
      revision_number: 4,
      body: "Adjust the arc",
      timecode_ms: 1250,
      frame_number: 30,
    });

    expect(media?.type).toBe("model/gltf-binary");
    expect(fetcher.mock.calls[0][0]).toContain("/api/reviews/media?");
    expect(fetcher).toHaveBeenNthCalledWith(
      2,
      "http://server/api/reviews/comments",
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("reads exact review-proxy timebase response headers", async () => {
    const fetcher = vi.fn(async () => new Response("proxy", {
      status: 200,
      headers: {
        "content-type": "model/gltf-binary",
        "x-review-frame-rate-numerator": "24000",
        "x-review-frame-rate-denominator": "1001",
        "x-review-start-frame": "1001",
      },
    }));
    const client = new OpenAssetApiClient("http://server", fetcher as unknown as typeof fetch);

    const proxy = await client.downloadReviewProxy(
      "token",
      "workspace-1",
      "Models/Hero.fbx",
      4,
    );

    expect(proxy?.blob.type).toBe("model/gltf-binary");
    expect(proxy).toMatchObject({
      frameRateNumerator: 24_000,
      frameRateDenominator: 1_001,
      startFrame: 1_001,
    });
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
