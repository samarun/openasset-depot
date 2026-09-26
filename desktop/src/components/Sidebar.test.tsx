import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  it("keeps admin navigation out of artist sessions while preserving settings", () => {
    const onNavigate = vi.fn();
    render(
      <Sidebar
        activeView="home"
        connectionState="connected"
        isAdmin={false}
        onNavigate={onNavigate}
      />,
    );

    expect(screen.queryByRole("button", { name: "Admin" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reviews" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(onNavigate).toHaveBeenCalledWith("settings");
  });

  it("shows admin navigation only for an admin session", () => {
    render(
      <Sidebar
        activeView="home"
        connectionState="connected"
        isAdmin
        onNavigate={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Admin" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reviews" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Shelves" })).toBeInTheDocument();
  });
});
