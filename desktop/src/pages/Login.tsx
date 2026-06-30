import {
  Archive,
  ArrowRight,
  Boxes,
  CheckCircle2,
  CloudCog,
  Eye,
  Fingerprint,
  LogIn,
  ShieldCheck,
  Sparkles,
  UserPlus,
} from "lucide-react";
import { FormEvent, useState } from "react";
import { friendlyApiError, OpenAssetApiClient } from "../api/client";
import type { SignupStatus, UserSession } from "../types/domain";

interface LoginProps {
  onLogin: (session: UserSession) => void;
}

type AuthMode = "login" | "signup";

export function Login({ onLogin }: LoginProps) {
  const [serverUrl, setServerUrl] = useState(readInitialServerUrl);
  const [mode, setMode] = useState<AuthMode>("login");
  const [signupStatus, setSignupStatus] = useState<SignupStatus>();
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirmation, setPasswordConfirmation] = useState("");
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [checkingSignup, setCheckingSignup] = useState(false);

  async function selectMode(nextMode: AuthMode) {
    setMode(nextMode);
    setError(undefined);
    if (nextMode !== "signup") return;
    setCheckingSignup(true);
    try {
      setSignupStatus(await new OpenAssetApiClient(serverUrl).signupStatus());
    } catch (statusError) {
      setSignupStatus(undefined);
      setError(friendlyApiError(statusError));
    } finally {
      setCheckingSignup(false);
    }
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    setLoading(true);
    setError(undefined);
    try {
      const client = new OpenAssetApiClient(serverUrl);
      if (mode === "signup") {
        if (!signupStatus?.enabled) {
          throw new Error("Self-signup is disabled. Ask a studio administrator for an account.");
        }
        if (password !== passwordConfirmation) {
          throw new Error("Passwords do not match.");
        }
        await client.signup(username, password, displayName.trim() || undefined);
      }
      const session = await client.login(username, password);
      sessionStorage.setItem("oad.session", JSON.stringify(session));
      localStorage.setItem("oad.serverUrl", serverUrl);
      onLogin(session);
    } catch (submitError) {
      setError(friendlyApiError(submitError));
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

  const signupDisabled = mode === "signup" && (!signupStatus?.enabled || checkingSignup);

  return (
    <main className="login-shell">
      <div className="login-backdrop login-backdrop-one" />
      <div className="login-backdrop login-backdrop-two" />
      <section className="login-stage">
        <section className="login-panel">
          <div className="brand-lockup login-brand">
            <span className="brand-mark"><Archive size={24} /></span>
            <div><strong>OpenAsset</strong><span>Depot</span></div>
          </div>

          <div className="login-heading">
            <span className="eyebrow">Studio access</span>
            <h1>{mode === "login" ? "Welcome back" : "Create your account"}</h1>
            <p>
              {mode === "login"
                ? "Step into the shared production workspace."
                : signupStatus?.first_user
                  ? "This first account will become the studio administrator."
                  : "Your administrator grants depot access after signup."}
            </p>
          </div>

          <div className="auth-tabs" role="tablist" aria-label="Authentication mode">
            <button
              className={mode === "login" ? "is-active" : ""}
              type="button"
              role="tab"
              aria-selected={mode === "login"}
              onClick={() => void selectMode("login")}
            >
              <LogIn size={16} /> Sign in
            </button>
            <button
              className={mode === "signup" ? "is-active" : ""}
              type="button"
              role="tab"
              aria-selected={mode === "signup"}
              onClick={() => void selectMode("signup")}
            >
              <UserPlus size={16} /> Sign up
            </button>
          </div>

          <form className="form-stack auth-form" onSubmit={submit}>
            <label>
              Server
              <input value={serverUrl} onChange={(event) => setServerUrl(event.target.value)} required />
            </label>
            {mode === "signup" && (
              <label>
                Display name
                <input
                  value={displayName}
                  onChange={(event) => setDisplayName(event.target.value)}
                  placeholder="How teammates will know you"
                  autoComplete="name"
                />
              </label>
            )}
            <label>
              Username
              <input
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                autoComplete="username"
                minLength={3}
                required
              />
            </label>
            <label>
              Password
              <input
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                type="password"
                autoComplete={mode === "login" ? "current-password" : "new-password"}
                minLength={8}
                required
              />
            </label>
            {mode === "signup" && (
              <label>
                Confirm password
                <input
                  value={passwordConfirmation}
                  onChange={(event) => setPasswordConfirmation(event.target.value)}
                  type="password"
                  autoComplete="new-password"
                  minLength={8}
                  required
                />
              </label>
            )}
            {mode === "signup" && signupStatus && !signupStatus.enabled && (
              <div className="auth-message">
                <ShieldCheck size={17} />
                Self-signup is closed. Ask your studio administrator to create an account.
              </div>
            )}
            {error && <p className="form-error">{error}</p>}
            <button className="primary-button fill-button auth-submit" type="submit" disabled={loading || signupDisabled}>
              {mode === "login" ? <LogIn size={17} /> : <UserPlus size={17} />}
              {loading ? "Working…" : mode === "login" ? "Enter workspace" : "Create account"}
              {!loading && <ArrowRight size={17} />}
            </button>
            {import.meta.env.DEV && (
              <button className="secondary-button fill-button" type="button" onClick={previewMockUi}>
                <Eye size={17} /> Preview Demo Mode
              </button>
            )}
          </form>

          <div className="auth-security-note">
            <Fingerprint size={16} /> Passwords are Argon2 hashed; project files never store credentials.
          </div>
        </section>

        <aside className="login-aside">
          <div className="login-aside-top">
            <span className="login-product-pill"><Sparkles size={14} /> Built for creative pipelines</span>
            <div className="login-orbit" aria-hidden="true">
              <span><Boxes size={34} /></span>
              <i className="orbit-node orbit-node-one" />
              <i className="orbit-node orbit-node-two" />
              <i className="orbit-node orbit-node-three" />
            </div>
          </div>
          <div className="login-aside-copy">
            <span className="eyebrow">One source of truth</span>
            <h2>Move ambitious assets without losing the plot.</h2>
            <p>Version scenes, protect binary work, and keep every DCC workstation aligned with the depot.</p>
          </div>
          <div className="login-feature-grid">
            <span><CloudCog size={18} /><strong>Browser → depot</strong><small>Upload and submit from anywhere</small></span>
            <span><ShieldCheck size={18} /><strong>Safe locking</strong><small>Exclusive edits without collisions</small></span>
            <span><CheckCircle2 size={18} /><strong>Verified sync</strong><small>BLAKE3 integrity on every transfer</small></span>
          </div>
        </aside>
      </section>
    </main>
  );
}

function readInitialServerUrl(): string {
  const savedUrl = localStorage.getItem("oad.serverUrl");
  const configuredUrl = import.meta.env.VITE_OPENASSET_API_URL;
  const runningInTauri = "__TAURI_INTERNALS__" in window;

  if (savedUrl && savedUrl !== "http://127.0.0.1:8080") return savedUrl;
  if (runningInTauri) return configuredUrl ?? "http://127.0.0.1:18080";
  return configuredUrl ?? window.location.origin;
}
