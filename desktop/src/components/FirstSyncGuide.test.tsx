import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Workspace } from "../types/domain";
import {
  FirstSyncGuide,
  completeFirstSyncGuide,
  isFirstSyncGuideComplete,
} from "./FirstSyncGuide";

const workspace: Workspace = {
  id: "workspace-1",
  depot_id: "depot-1",
  stream_id: "main",
  name: "Shot 120 Lighting",
  local_path: "/Projects/Shot120",
};

describe("FirstSyncGuide", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("names the workspace folder and offers sync as the only action", () => {
    const onSync = vi.fn();
    render(
      <FirstSyncGuide workspace={workspace} onSync={onSync} onDismiss={vi.fn()} />,
    );

    expect(screen.getByText(/\/Projects\/Shot120/)).toBeInTheDocument();
    const buttons = screen.getAllByRole("button");
    // Sync plus the dismiss control: later steps stay descriptive.
    expect(buttons).toHaveLength(2);

    fireEvent.click(screen.getByRole("button", { name: /Sync Latest/ }));
    expect(onSync).toHaveBeenCalledOnce();
  });

  it("records completion per workspace so the guide does not return", () => {
    expect(isFirstSyncGuideComplete(workspace.id)).toBe(false);

    completeFirstSyncGuide(workspace.id);

    expect(isFirstSyncGuideComplete(workspace.id)).toBe(true);
    expect(isFirstSyncGuideComplete("another-workspace")).toBe(false);
  });

  it("treats unreadable storage as onboarding not yet complete", () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("storage blocked");
    });
    try {
      expect(isFirstSyncGuideComplete(workspace.id)).toBe(false);
    } finally {
      getItem.mockRestore();
    }
  });
});
