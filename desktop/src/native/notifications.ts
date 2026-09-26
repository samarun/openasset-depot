import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { isNativeDesktop } from "./workspaces";

/** Operations shorter than this finish while the artist is still watching. */
const LONG_OPERATION_MS = 10_000;

/**
 * Notifies the artist that a long-running transfer finished.
 *
 * Quick operations stay silent: a notification for something the artist just
 * watched complete is noise. Permission is requested lazily on first use so the
 * app never prompts before it has something worth saying.
 */
export async function notifyOperationComplete(
  title: string,
  body: string,
  elapsedMs: number,
): Promise<void> {
  if (!isNativeDesktop() || elapsedMs < LONG_OPERATION_MS) return;
  if (document.visibilityState === "visible" && document.hasFocus()) return;

  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      granted = (await requestPermission()) === "granted";
    }
    if (granted) sendNotification({ title, body });
  } catch {
    // Notifications are a courtesy; never let them fail an operation.
  }
}
