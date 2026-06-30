import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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
});
