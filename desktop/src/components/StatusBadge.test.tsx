import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusBadge } from "./StatusBadge";

describe("StatusBadge", () => {
  it("renders a status label", () => {
    render(<StatusBadge status="Needs Sync" />);
    expect(screen.getByText("Needs Sync")).toBeInTheDocument();
  });
});
