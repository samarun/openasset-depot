import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { mockChangelists } from "../data/mockData";
import { SubmitDialog } from "./SubmitDialog";

describe("SubmitDialog", () => {
  it("runs validation on open and presents one submit sheet", async () => {
    const onValidate = vi.fn(async () => true);
    render(
      <SubmitDialog
        changelist={mockChangelists[1]}
        open
        validating={false}
        validationWarnings={[]}
        validationErrors={[]}
        onValidate={onValidate}
        onSubmit={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(1));
    expect(screen.getByLabelText("What changed?")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Submit 1 file" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Continue" })).not.toBeInTheDocument();
  });
});
