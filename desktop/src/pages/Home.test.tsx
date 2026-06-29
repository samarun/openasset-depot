import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { mockFiles, mockWorkspace } from "../data/mockData";
import { Home } from "./Home";

describe("Home", () => {
  it("presents an asset-first studio workspace instead of KPI cards", () => {
    render(
      <Home
        workspace={mockWorkspace}
        files={mockFiles}
        changelists={[]}
        locks={[]}
        currentUser="demo.artist"
        onNavigate={vi.fn()}
        onSelectFile={vi.fn()}
        onSync={vi.fn()}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: mockWorkspace.name })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Continue Working" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "My Work" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sync Latest" })).toBeInTheDocument();
    expect(screen.queryByText("Workspace Health")).not.toBeInTheDocument();
  });
});
