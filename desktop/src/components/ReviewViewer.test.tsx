import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFiles } from "../data/mockData";
import type { ReviewComment } from "../types/domain";
import { frameAtTime, ReviewViewer, reviewFrameRate, reviewPlayerFileName } from "./ReviewViewer";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ReviewViewer", () => {
  it("uses the served proxy type when an FBX revision has a GLB review proxy", () => {
    expect(reviewPlayerFileName("AnimatedHero.fbx", "model/gltf-binary")).toBe("review.glb");
    expect(reviewPlayerFileName("AnimatedHero.glb", "application/vnd.autodesk.fbx")).toBe("review.fbx");
    expect(reviewPlayerFileName("AnimatedHero.glb", "application/octet-stream")).toBe("AnimatedHero.glb");
  });

  it("uses exact proxy timebase metadata and never assumes 24 fps", () => {
    const frameRate = reviewFrameRate({
      blob: new Blob(),
      contentType: "model/gltf-binary",
      source: "asset",
      frameRateNumerator: 24_000,
      frameRateDenominator: 1_001,
      startFrame: 1_001,
    });
    expect(frameRate).toBeCloseTo(23.976, 3);
    expect(frameAtTime(1_000, frameRate, 1_001)).toBe(1_025);
    expect(frameAtTime(1_000, undefined, 0)).toBeUndefined();
  });

  it("opens revision feedback and posts a comment", async () => {
    Object.defineProperty(URL, "createObjectURL", { configurable: true, value: vi.fn(() => "blob:review") });
    Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: vi.fn() });
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    const created: ReviewComment = {
      id: "comment-1",
      path: mockFiles[0].path,
      revision_number: mockFiles[0].revision,
      author_user_id: "user-1",
      author: "Artist One",
      body: "Ease the landing pose.",
      created_at: "2026-06-30T10:00:00Z",
    };
    const createComment = vi.fn(async () => created);

    render(
      <ReviewViewer
        file={mockFiles[0]}
        open
        onClose={vi.fn()}
        loadMedia={async () => ({
          blob: new Blob(["preview"], { type: "image/png" }),
          contentType: "image/png",
          source: "preview",
        })}
        loadComments={async () => []}
        createComment={createComment}
        resolveComment={vi.fn(async (comment) => comment)}
      />,
    );

    expect(await screen.findByRole("dialog", { name: `Review ${mockFiles[0].name}` })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Pen" })).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("Leave clear, actionable feedback…"), {
      target: { value: "Ease the landing pose." },
    });
    fireEvent.click(screen.getByRole("button", { name: "Post Comment" }));

    await waitFor(() => expect(createComment).toHaveBeenCalledWith(
      mockFiles[0],
      expect.objectContaining({ body: "Ease the landing pose." }),
    ));
    expect(await screen.findByText("Ease the landing pose.")).toBeInTheDocument();
  });
});
