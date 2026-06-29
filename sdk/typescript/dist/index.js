export class OpenAssetClient {
    fetcher;
    baseUrl;
    token;
    constructor(baseUrl, token, fetcher = fetch) {
        this.fetcher = fetcher;
        this.baseUrl = baseUrl.replace(/\/$/, "");
        this.token = token;
    }
    setToken(token) {
        this.token = token;
    }
    async login(username, password) {
        const session = await this.request("/api/auth/login", {
            method: "POST",
            body: JSON.stringify({ username, password }),
        });
        this.token = session.token;
        return session;
    }
    listAdapters() {
        return this.request("/api/adapters", { token: this.requireToken() });
    }
    detectProject(request) {
        return this.request("/api/adapters/detect", {
            method: "POST",
            token: this.requireToken(),
            body: JSON.stringify(request),
        });
    }
    scanDependencies(request) {
        return this.request("/api/adapters/scan", {
            method: "POST",
            token: this.requireToken(),
            body: JSON.stringify(request),
        });
    }
    generatePreview(request) {
        return this.request("/api/adapters/preview", {
            method: "POST",
            token: this.requireToken(),
            body: JSON.stringify(request),
        });
    }
    extractMetadata(request) {
        return this.request("/api/adapters/metadata", {
            method: "POST",
            token: this.requireToken(),
            body: JSON.stringify(request),
        });
    }
    validateAdapterChangelist(request) {
        return this.request("/api/adapters/validate", {
            method: "POST",
            token: this.requireToken(),
            body: JSON.stringify(request),
        });
    }
    listDepots() {
        return this.authedRequest("/api/depots");
    }
    createDepot(name, description) {
        return this.authedRequest("/api/depots", {
            method: "POST",
            headers: this.idempotencyHeaders(),
            body: JSON.stringify({ name, description }),
        });
    }
    listStreams() {
        return this.authedRequest("/api/streams");
    }
    createStream(depot, name) {
        return this.authedRequest("/api/streams", {
            method: "POST",
            headers: this.idempotencyHeaders(),
            body: JSON.stringify({ depot, name }),
        });
    }
    listWorkspaces() {
        return this.authedRequest("/api/workspaces");
    }
    createWorkspace(request) {
        return this.authedRequest("/api/workspaces", {
            method: "POST",
            headers: this.idempotencyHeaders(),
            body: JSON.stringify(request),
        });
    }
    deleteWorkspace(workspaceId) {
        return this.authedRequest(`/api/workspaces/${workspaceId}`, {
            method: "DELETE",
        });
    }
    listLocksPage(query = {}) {
        const params = queryParams(query);
        return this.authedRequest(`/api/locks/page?${params}`);
    }
    async listLocks() {
        const locks = [];
        let before_created_at;
        let before_id;
        for (;;) {
            const page = await this.listLocksPage({ limit: 1000, before_created_at, before_id });
            locks.push(...page.items);
            const nextCreatedAt = page.next_before_created_at ?? undefined;
            const nextId = page.next_before_id ?? undefined;
            if (!nextCreatedAt || !nextId)
                return locks;
            if (nextCreatedAt === before_created_at && nextId === before_id) {
                throw new Error("Server returned a repeated lock-page cursor");
            }
            before_created_at = nextCreatedAt;
            before_id = nextId;
        }
    }
    listAuditPage(query = {}) {
        const params = queryParams(query);
        return this.authedRequest(`/api/audit?${params}`);
    }
    listDependenciesPage(query) {
        const params = queryParams(query);
        return this.authedRequest(`/api/dependencies?${params}`);
    }
    lockFile(workspaceId, path, reason) {
        return this.authedRequest("/api/files/lock", {
            method: "POST",
            body: JSON.stringify({ workspace_id: workspaceId, path, reason }),
        });
    }
    unlockFile(workspaceId, path) {
        return this.authedRequest("/api/files/unlock", {
            method: "POST",
            body: JSON.stringify({ workspace_id: workspaceId, path }),
        });
    }
    createChangelist(workspaceId, description) {
        return this.authedRequest("/api/changelists", {
            method: "POST",
            headers: this.idempotencyHeaders(),
            body: JSON.stringify({ workspace_id: workspaceId, description }),
        });
    }
    addFile(request) {
        return this.fileOperation("/api/files/add", request);
    }
    editFile(request) {
        return this.fileOperation("/api/files/edit", request);
    }
    deleteFile(request) {
        return this.fileOperation("/api/files/delete", request);
    }
    revertFile(request) {
        return this.fileOperation("/api/files/revert", request);
    }
    planSync(workspaceId, pathPrefix) {
        return this.authedRequest("/api/sync/plan", {
            method: "POST",
            body: JSON.stringify({ workspace_id: workspaceId, path_prefix: pathPrefix }),
        });
    }
    fileHistory(workspaceId, path, limit = 100) {
        const query = new URLSearchParams({
            workspace_id: workspaceId,
            path,
            limit: String(limit),
        });
        return this.authedRequest(`/api/files/history?${query.toString()}`);
    }
    submitFiles(workspaceId, changelistId, files) {
        if (files.length === 0)
            throw new Error("Submit requires at least one file");
        const form = new FormData();
        form.set("workspace_id", workspaceId);
        for (const file of files)
            form.append("file", file.blob, file.path);
        return this.authedRequest(`/api/changelists/${changelistId}/submit`, {
            method: "POST",
            body: form,
        });
    }
    fileOperation(endpoint, request) {
        return this.authedRequest(endpoint, {
            method: "POST",
            body: JSON.stringify(request),
        });
    }
    authedRequest(path, options = {}) {
        return this.request(path, { ...options, token: this.requireToken() });
    }
    idempotencyHeaders() {
        return { "idempotency-key": crypto.randomUUID() };
    }
    requireToken() {
        if (!this.token)
            throw new Error("OpenAssetClient requires a token for this endpoint");
        return this.token;
    }
    async request(path, options = {}) {
        const headers = new Headers(options.headers);
        if (!(options.body instanceof FormData))
            headers.set("content-type", "application/json");
        if (options.token)
            headers.set("authorization", `Bearer ${options.token}`);
        const response = await this.fetcher(`${this.baseUrl}${path}`, { ...options, headers });
        if (!response.ok) {
            const body = await response.text();
            throw new Error(`OpenAsset request failed with ${response.status}: ${body}`);
        }
        return (await response.json());
    }
}
function queryParams(query) {
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(query)) {
        if (value !== undefined && value !== null)
            params.set(key, String(value));
    }
    return params.toString();
}
