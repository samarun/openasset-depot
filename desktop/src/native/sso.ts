import { invoke } from "@tauri-apps/api/core";
import { OpenAssetApiClient } from "../api/client";
import type { UserSession } from "../types/domain";
import { isNativeDesktop } from "./workspaces";

interface SsoRedirect {
  code: string;
  state: string;
}

/**
 * Runs an SSO sign-in and returns the resulting session.
 *
 * The desktop app cannot receive an HTTPS redirect, so the provider is pointed
 * at a loopback listener the Tauri side opens for exactly one request. The
 * listener starts before the browser opens, otherwise a fast provider could
 * redirect before anything is there to answer.
 */
export async function signInWithSso(client: OpenAssetApiClient): Promise<UserSession> {
  if (!isNativeDesktop()) {
    throw new Error("Single sign-on requires the OpenAsset Depot desktop app.");
  }
  const { authorization_url } = await client.startSso();
  const redirect = invoke<SsoRedirect>("await_sso_redirect");
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(authorization_url);
  } catch (error) {
    // The listener would otherwise wait five minutes for a browser that never
    // opened, so surface the real failure immediately.
    void redirect.catch(() => undefined);
    throw new Error(
      `Could not open your browser to sign in: ${error instanceof Error ? error.message : error}`,
    );
  }
  const { code, state } = await redirect;
  return client.completeSso(code, state);
}
