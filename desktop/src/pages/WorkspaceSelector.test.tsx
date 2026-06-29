import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceSelector } from "./WorkspaceSelector";

describe("WorkspaceSelector", () => {
  it("lets a fresh admin create the first depot, stream, and workspace", async () => {
    const onCreateDepot = vi.fn(async () => ({ id: "depot-1", name: "Feature" }));
    const onCreateStream = vi.fn(async () => ({ id: "stream-1", depot_id: "depot-1", name: "main" }));
    const onCreate = vi.fn(async () => undefined);
    render(
      <WorkspaceSelector
        workspaces={[]}
        depots={[]}
        streams={[]}
        loading={false}
        isAdmin
        onRefresh={vi.fn()}
        onCreateDepot={onCreateDepot}
        onCreateStream={onCreateStream}
        onCreate={onCreate}
        onSelect={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Start Studio Setup" }));
    fireEvent.change(screen.getByLabelText("Depot Name"), { target: { value: "Feature" } });
    fireEvent.change(screen.getByLabelText("Workspace Name"), { target: { value: "artist-main" } });
    fireEvent.change(screen.getByLabelText("Local Folder"), { target: { value: "/projects/feature" } });
    fireEvent.click(screen.getByRole("button", { name: "Create and Open Workspace" }));

    await waitFor(() => expect(onCreate).toHaveBeenCalledWith({
      name: "artist-main",
      depot: "depot-1",
      stream: "stream-1",
      local_path: "/projects/feature",
    }));
  });
});
