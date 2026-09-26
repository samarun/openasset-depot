import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ShelfSummary } from "../types/domain";
import { Shelves } from "./Shelves";

function shelf(overrides: Partial<ShelfSummary> = {}): ShelfSummary {
  return {
    changelist_id: "cl-1",
    description: "Blocking pass for the lobby",
    changelist_status: "pending",
    user_id: "user-1",
    owner: "alice",
    file_count: 2,
    total_bytes: 2_621_440,
    shelved_at: new Date().toISOString(),
    ...overrides,
  };
}

const handlers = { onRefresh: vi.fn(), onRestore: vi.fn(), onDiscard: vi.fn() };

describe("Shelves", () => {
  it("separates the caller's shelves from teammates' and only offers actions on its own", () => {
    const mine = shelf();
    const theirs = shelf({ changelist_id: "cl-2", owner: "bob", description: "Lookdev wip" });
    render(<Shelves shelves={[mine, theirs]} currentUser="alice" loading={false} {...handlers} />);

    expect(screen.getByText("Blocking pass for the lobby")).toBeInTheDocument();
    expect(screen.getByText("Lookdev wip")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /Restore Shelf/ })).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: /Restore Shelf/ }));
    expect(handlers.onRestore).toHaveBeenCalledWith(mine);
  });

  it("explains what shelving is for when there is nothing parked", () => {
    render(<Shelves shelves={[]} currentUser="alice" loading={false} {...handlers} />);

    expect(screen.getByText("Nothing shelved")).toBeInTheDocument();
    expect(screen.getByText(/without submitting it/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Restore Shelf/ })).not.toBeInTheDocument();
  });

  it("shows file counts and a human size so a shelf can be judged before restoring it", () => {
    render(
      <Shelves
        shelves={[shelf({ file_count: 1, total_bytes: 512 })]}
        currentUser="alice"
        loading={false}
        {...handlers}
      />,
    );

    expect(screen.getByText(/1 file · 512 B/)).toBeInTheDocument();
  });

  it("disables shelf actions while another operation is running", () => {
    render(<Shelves shelves={[shelf()]} currentUser="alice" loading={false} busy {...handlers} />);

    expect(screen.getByRole("button", { name: /Restore Shelf/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Discard/ })).toBeDisabled();
  });
});
