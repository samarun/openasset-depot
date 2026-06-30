import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Workspace } from "./Workspace";

describe("Workspace browser uploads", () => {
  it("passes selected browser files to the upload workflow", () => {
    const onChooseFiles = vi.fn();
    const { container } = render(
      <Workspace
        files={[]}
        onSelectFile={vi.fn()}
        onSync={vi.fn()}
        onChooseFiles={onChooseFiles}
        browserMode
        onLock={vi.fn()}
        onUnlock={vi.fn()}
        onRevert={vi.fn()}
        onDelete={vi.fn()}
        onSubmit={vi.fn()}
        onReview={vi.fn()}
        onHistory={vi.fn()}
      />,
    );
    const file = new File(["asset"], "asset.txt", { type: "text/plain" });
    const input = container.querySelector("input[type=file]") as HTMLInputElement;

    expect(screen.getByRole("button", { name: "Upload Files" })).toBeInTheDocument();
    fireEvent.change(input, { target: { files: [file] } });

    expect(onChooseFiles).toHaveBeenCalledWith([file]);
  });
});
