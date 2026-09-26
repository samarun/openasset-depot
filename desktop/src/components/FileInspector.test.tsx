import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AssetFile, DependencyImpact } from "../types/domain";
import { FileInspector } from "./FileInspector";

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

const handlers = {
  onHistory: vi.fn(),
  onLock: vi.fn(),
  onUnlock: vi.fn(),
  onRevert: vi.fn(),
  onDelete: vi.fn(),
  onSubmit: vi.fn(),
  onReview: vi.fn(),
};

function impact(overrides: Partial<DependencyImpact> = {}): DependencyImpact {
  return {
    path: file.path,
    required_by: [],
    depends_on: [],
    required_by_count: 0,
    depends_on_count: 0,
    missing_count: 0,
    truncated: false,
    last_scanned_at: "2026-08-25T10:00:00Z",
    ...overrides,
  };
}

function edge(
  path: string,
  dependency_status: "present" | "missing" | "external" | "unknown" = "present",
) {
  return {
    path,
    dependency_type: "reference" as const,
    dependency_status,
    adapter_name: "nuke",
    confidence: 1,
    scan_time: "2026-08-25T10:00:00Z",
    in_depot: dependency_status !== "missing" && dependency_status !== "external",
  };
}

describe("FileInspector dependency impact", () => {
  it("leads with how many other assets would be affected by a change", async () => {
    const loadImpact = vi.fn(async () =>
      impact({
        required_by: [edge("Shots/010/comp.nk"), edge("Shots/020/comp.nk")],
        required_by_count: 2,
        depends_on: [
          edge("Plates/hero.exr"),
          edge("Roto/hero.nk"),
          edge("Missing/plate.exr", "missing"),
        ],
        depends_on_count: 3,
        missing_count: 1,
      }),
    );
    render(<FileInspector file={file} open {...handlers} loadImpact={loadImpact} />);

    expect(await screen.findByText("2 assets use this file")).toBeInTheDocument();
    expect(screen.getByText("Shots/010/comp.nk")).toBeInTheDocument();
    expect(
      screen.getByText("It references 3 other files, 1 of which could not be found."),
    ).toBeInTheDocument();
    expect(screen.getByText("Plates/hero.exr")).toBeInTheDocument();
    expect(loadImpact).toHaveBeenCalledWith(file);
  });

  it("collapses a long dependent list behind a count so the panel stays readable", async () => {
    const dependents = ["a", "b", "c", "d", "e", "f"].map((name) => edge(`Shots/${name}.nk`));
    const loadImpact = vi.fn(async () =>
      impact({ required_by: dependents, required_by_count: dependents.length }),
    );
    render(<FileInspector file={file} open {...handlers} loadImpact={loadImpact} />);

    const more = await screen.findByRole("button", { name: "Show 2 more" });
    expect(screen.queryByText("Shots/f.nk")).not.toBeInTheDocument();

    fireEvent.click(more);
    expect(screen.getByText("Shots/f.nk")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Show \d+ more/ })).not.toBeInTheDocument();
  });

  it("says an asset was never scanned rather than implying nothing depends on it", async () => {
    const loadImpact = vi.fn(async () => impact({ last_scanned_at: undefined }));
    render(<FileInspector file={file} open {...handlers} loadImpact={loadImpact} />);

    expect(await screen.findByText(/Not scanned yet/)).toBeInTheDocument();
    expect(screen.queryByText("Nothing else references this file")).not.toBeInTheDocument();
  });

  it("reports a failed lookup instead of showing a reassuring empty result", async () => {
    const loadImpact = vi.fn(async () => {
      throw new Error("offline");
    });
    render(<FileInspector file={file} open {...handlers} loadImpact={loadImpact} />);

    expect(await screen.findByText("Could not reach the dependency graph.")).toBeInTheDocument();
  });

  it("omits the section entirely when impact cannot be looked up, such as in demo mode", () => {
    render(<FileInspector file={file} open {...handlers} />);

    expect(screen.queryByText("Dependency impact")).not.toBeInTheDocument();
  });
});

describe("FileInspector review request", () => {
  it("offers request review for a submitted revision", () => {
    const onRequestReview = vi.fn();
    render(<FileInspector file={file} open {...handlers} onRequestReview={onRequestReview} />);

    fireEvent.click(screen.getByRole("button", { name: "Request Review" }));
    expect(onRequestReview).toHaveBeenCalledWith(file);
  });

  it("hides request review for an unsubmitted local file", () => {
    render(
      <FileInspector
        file={{ ...file, revision: 0 }}
        open
        {...handlers}
        onRequestReview={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Request Review" })).not.toBeInTheDocument();
  });
});
