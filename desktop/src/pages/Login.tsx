import { Archive, Eye, LogIn } from "lucide-react";
import { FormEvent, useState } from "react";
import { friendlyApiError, OpenAssetApiClient } from "../api/client";
import type { UserSession } from "../types/domain";

interface LoginProps {
  onLogin: (session: UserSession) => void;
}

export function Login({ onLogin }: LoginProps) {
  const [serverUrl, setServerUrl] = useState(readInitialServerUrl);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setLoading(true);
    setError(undefined);
    try {
      const client = new OpenAssetApiClient(serverUrl);
      const session = await client.login(username, password);
      sessionStorage.setItem("oad.session", JSON.stringify(session));
      localStorage.setItem("oad.serverUrl", serverUrl);
      onLogin(session);
    } catch (err) {
      setError(friendlyApiError(err));
    } finally {
      setLoading(false);
    }
  }

  function previewMockUi() {
    localStorage.setItem("oad.serverUrl", serverUrl);
    onLogin({
      token: "mock-preview-token",
      username: "demo.artist",
      serverUrl,
      isAdmin: false,
    });
  }

  return (
    <main className="login-shell">
      <section className="login-panel">
        <div className="brand-lockup login-brand">
          <span className="brand-mark">
            <Archive size={24} />
          </span>
          <div>
            <strong>OpenAsset</strong>
            <span>Depot</span>
          </div>
        </div>
        <h1>Sign in</h1>
        <form className="form-stack" onSubmit={submit}>
          <label>
            Server
            <input value={serverUrl} onChange={(event) => setServerUrl(event.target.value)} />
          </label>
          <label>
            Username
            <input value={username} onChange={(event) => setUsername(event.target.value)} autoComplete="username" />
          </label>
          <label>
            Password
            <input
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              type="password"
              autoComplete="current-password"
            />
          </label>
          {error && <p className="form-error">{error}</p>}
          <button className="primary-button fill-button" type="submit" disabled={loading}>
            <LogIn size={17} />
            {loading ? "Signing in" : "Sign in"}
          </button>
          {import.meta.env.DEV && (
            <button className="secondary-button fill-button" type="button" onClick={previewMockUi}>
              <Eye size={17} />
              Preview Demo Mode
            </button>
          )}
        </form>
      </section>
      <section className="login-aside">
        <h2>Creative production control without the clutter.</h2>
        <p>Locks, changelists, sync health, reviews, and stage safety in one calm workspace.</p>
      </section>
    </main>
  );
}

function readInitialServerUrl(): string {
  const savedUrl = localStorage.getItem("oad.serverUrl");
  const configuredUrl = import.meta.env.VITE_OPENASSET_API_URL;
  const runningInTauri = "__TAURI_INTERNALS__" in window;

  if (savedUrl && savedUrl !== "http://127.0.0.1:8080") {
    return savedUrl;
  }

  if (runningInTauri) {
    return configuredUrl ?? "http://127.0.0.1:18080";
  }

  return configuredUrl ?? window.location.origin;
}
