import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AssetFile, Collaborator } from "../types/domain";
import { ReviewRequestDialog } from "./ReviewRequestDialog";

const file: AssetFile = {
  id: "hero",
  name: "Hero.uasset",
  path: "Content/Characters/Hero.uasset",
  kind: "Unreal asset",
  size: "24 MB",
  revision: 8,
  statuses: ["Up to Date"],
  previewTone: "model",
  dependencies: [],
  source: "backend",
};

const collaborators: Collaborator[] = [
  { user_id: "alice-id", username: "alice", display_name: "Alice", role: "write", is_admin: false },
  { user_id: "bob-id", username: "bob", display_name: "Bob", role: "write", is_admin: false },
];

describe("ReviewRequestDialog", () => {
  it("requires a submitted asset and at least one other reviewer", () => {
    const onSubmit = vi.fn();
    render(
      <ReviewRequestDialog
        open
        file={file}
        collaborators={collaborators}
        currentUser="alice"
        loadingCollaborators={false}
        onSubmit={onSubmit}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Request Review" })).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: /Bob/ }));
    fireEvent.click(screen.getByRole("button", { name: "Request Review" }));
    expect(onSubmit).toHaveBeenCalledWith({
      title: "Hero.uasset r8",
      description: "",
      reviewers: ["bob-id"],
    });
  });

  it("does not offer the caller as a reviewer of their own request", () => {
    render(
      <ReviewRequestDialog
        open
        file={file}
        collaborators={collaborators}
        currentUser="alice"
        loadingCollaborators={false}
        onSubmit={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(screen.queryByRole("checkbox", { name: /Alice/ })).not.toBeInTheDocument();
  });
});
