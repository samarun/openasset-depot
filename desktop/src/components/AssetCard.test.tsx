import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AssetFile } from "../types/domain";
import { AssetCard } from "./AssetCard";

const file: AssetFile = {
  id: "hero",
  name: "Hero.uasset",
  path: "Content/Characters/Hero.uasset",
  kind: "Unreal asset",
  size: "24 MB",
  revision: 8,
  statuses: ["Checked Out", "Ready to Submit"],
  previewTone: "model",
  dependencies: [],
  source: "backend",
};

describe("AssetCard", () => {
  it("exposes a clear selected state and keeps the status ahead of the path", () => {
    const onSelect = vi.fn();
    render(<AssetCard file={file} selected onSelect={onSelect} />);

    const card = screen.getByRole("button", { name: /Hero.uasset/ });
    expect(card).toHaveAttribute("aria-pressed", "true");
    expect(card).toHaveClass("is-selected");
    expect(screen.getByText("Checked Out")).toBeInTheDocument();

    fireEvent.click(card);
    expect(onSelect).toHaveBeenCalledWith(file);
  });

  it("shows a submitted revision preview when one is available", async () => {
    const previewFile = { ...file, previewAvailable: true };
    const loadPreview = vi.fn(async () => "data:image/png;base64,cHJldmlldw==");
    render(<AssetCard file={previewFile} onSelect={vi.fn()} loadPreview={loadPreview} />);

    expect(await screen.findByAltText("Hero.uasset preview")).toHaveAttribute(
      "src",
      "data:image/png;base64,cHJldmlldw==",
    );
    expect(loadPreview).toHaveBeenCalledWith(previewFile);
  });
});
