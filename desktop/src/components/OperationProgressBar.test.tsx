import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { OperationProgressBar } from "./OperationProgressBar";

describe("OperationProgressBar", () => {
  it("explains creative-host operations with a useful percentage", () => {
    render(
      <OperationProgressBar
        progress={{
          operationId: "operation-1",
          command: "checkout",
          phase: "working",
          message: "Reserving Scenes/Lobby.blend",
          completed: 42,
          total: 100,
        }}
      />,
    );

    expect(screen.getByText("Checking out asset")).toBeInTheDocument();
    expect(screen.getByText("Reserving Scenes/Lobby.blend")).toBeInTheDocument();
    expect(screen.getByText("42%")).toBeInTheDocument();
  });

  it("shows a running file count and current path while a paged sync is still discovering work", () => {
    render(
      <OperationProgressBar
        progress={{
          operationId: "operation-2",
          command: "sync",
          phase: "transferring",
          message: "Downloading Content/Maps/Main.umap",
          completed: 70,
          total: 100,
          path: "Content/Maps/Main.umap",
          filesCompleted: 12,
          bytesCompleted: 5_242_880,
        }}
      />,
    );

    expect(
      screen.getByText("12 files · 5.0 MB · Content/Maps/Main.umap"),
    ).toBeInTheDocument();
  });

  it("shows totals only once the CLI reports them", () => {
    render(
      <OperationProgressBar
        progress={{
          operationId: "operation-3",
          command: "submit",
          phase: "uploading",
          message: "Uploading 3 files",
          completed: 62,
          total: 100,
          path: "Comp/Shot010.nk",
          filesCompleted: 0,
          filesTotal: 3,
          bytesCompleted: 0,
          bytesTotal: 2_048,
        }}
      />,
    );

    expect(screen.getByText("0 of 3 files · 0 B of 2.0 KB · Comp/Shot010.nk")).toBeInTheDocument();
  });

  it("reports throughput and a coarse time-remaining band once two samples exist", () => {
    vi.useFakeTimers();
    try {
      const base = {
        operationId: "operation-5",
        command: "sync" as const,
        phase: "transferring",
        message: "Downloading Content/Maps/Main.umap",
        completed: 30,
        total: 100,
        bytesTotal: 10_485_760,
      };
      const { rerender } = render(
        <OperationProgressBar progress={{ ...base, bytesCompleted: 1_048_576 }} />,
      );
      vi.advanceTimersByTime(2_000);
      rerender(<OperationProgressBar progress={{ ...base, bytesCompleted: 3_145_728 }} />);

      expect(screen.getByText("1.0 MB/s · a few seconds left")).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("omits throughput while a single sample makes the rate unknowable", () => {
    render(
      <OperationProgressBar
        progress={{
          operationId: "operation-6",
          command: "sync",
          phase: "transferring",
          message: "Downloading",
          completed: 10,
          total: 100,
          bytesCompleted: 1_048_576,
          bytesTotal: 10_485_760,
        }}
      />,
    );

    expect(screen.queryByText(/\/s/)).not.toBeInTheDocument();
  });

  it("can be dismissed without cancelling the operation", () => {
    const onDismiss = vi.fn();
    render(
      <OperationProgressBar
        progress={{
          operationId: "operation-4",
          command: "sync",
          phase: "transferring",
          message: "Syncing",
          completed: 30,
          total: 100,
        }}
        onDismiss={onDismiss}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Hide progress/ }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("names shelving the same way host panels do", () => {
    render(
      <OperationProgressBar
        progress={{
          operationId: "operation-6",
          command: "shelve",
          phase: "working",
          message: "Uploading parked changes",
          completed: 40,
          total: 100,
        }}
      />,
    );

    expect(screen.getByText("Shelving changes")).toBeInTheDocument();
  });
});
