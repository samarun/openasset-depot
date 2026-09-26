import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ReviewRequest } from "../types/domain";
import { Reviews } from "./Reviews";

function request(overrides: Partial<ReviewRequest> = {}): ReviewRequest {
  return {
    id: "rev-1",
    path: "Content/Characters/Hero.uasset",
    revision_number: 3,
    requested_by: "alice-id",
    requester: "alice",
    title: "Hero silhouette pass",
    description: "",
    state: "open",
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    closed_at: null,
    reviewers: [
      {
        reviewer_user_id: "bob-id",
        reviewer: "bob",
        decision: "pending",
        note: null,
        decided_at: null,
      },
    ],
    ...overrides,
  };
}

const handlers = {
  onRefresh: vi.fn(),
  onCreate: vi.fn(),
  onDecide: vi.fn(),
  onClose: vi.fn(),
  onOpenAsset: vi.fn(),
};

describe("Reviews", () => {
  it("lets an assigned reviewer approve without offering close", () => {
    render(
      <Reviews
        requests={[request()]}
        currentUser="bob"
        loading={false}
        {...handlers}
      />,
    );

    fireEvent.click(screen.getAllByRole("button", { name: "Approve" })[0]);
    expect(handlers.onDecide).toHaveBeenCalledWith(expect.objectContaining({ id: "rev-1" }), "approved");
    expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
  });

  it("lets the requester close an open review but not decide it", () => {
    render(
      <Reviews
        requests={[request()]}
        currentUser="alice"
        loading={false}
        {...handlers}
      />,
    );

    expect(screen.queryByRole("button", { name: "Approve" })).not.toBeInTheDocument();
    fireEvent.click(screen.getAllByRole("button", { name: "Close" })[0]);
    expect(handlers.onClose).toHaveBeenCalledWith(expect.objectContaining({ id: "rev-1" }));
  });

  it("explains the empty queue instead of looking like a loading failure", () => {
    render(<Reviews requests={[]} currentUser="alice" loading={false} {...handlers} />);
    expect(screen.getByText("No reviews yet")).toBeInTheDocument();
  });
});
