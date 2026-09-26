import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SubmitDialog } from "../components/SubmitDialog";
import { mockChangelists, mockFiles, mockLocks, mockWorkspace } from "../data/mockData";
import { Home } from "../pages/Home";
import { Workspace } from "../pages/Workspace";

/**
 * Structural regression baseline for the three surfaces artists live in.
 *
 * These snapshots lock the rendered markup — element structure, ordering, and
 * the class names that drive every visual token — so an accidental layout or
 * styling regression fails CI instead of shipping. They run in jsdom, so they
 * assert structure rather than pixels; pair them with a manual look when
 * deliberately restyling, and update with `vitest -u`.
 *
 * Fixtures are the deterministic demo dataset, so output is stable across
 * machines and runs.
 */
describe("visual regression baseline", () => {
  it("renders My Work with pending sync and lock activity", () => {
    const { container } = render(
      <Home
        workspace={mockWorkspace}
        files={mockFiles}
        changelists={mockChangelists}
        locks={mockLocks}
        currentUser="demo.artist"
        onNavigate={vi.fn()}
        onSelectFile={vi.fn()}
        onSync={vi.fn()}
        onSubmit={vi.fn()}
      />,
    );

    expect(container.firstChild).toMatchSnapshot();
  });

  it("renders the asset browser with a selected file and inspector", () => {
    const { container } = render(
      <Workspace
        files={mockFiles}
        selectedFile={mockFiles[0]}
        onSelectFile={vi.fn()}
        onSync={vi.fn()}
        onChooseFiles={vi.fn()}
        onLock={vi.fn()}
        onUnlock={vi.fn()}
        onRevert={vi.fn()}
        onDelete={vi.fn()}
        onSubmit={vi.fn()}
        onReview={vi.fn()}
        onHistory={vi.fn()}
      />,
    );

    expect(container.firstChild).toMatchSnapshot();
  });

  it("renders the submit sheet with warnings surfaced inline", () => {
    const changelist = mockChangelists[0];
    const { baseElement } = render(
      <SubmitDialog
        changelist={changelist}
        open
        validating={false}
        validationWarnings={changelist.warnings}
        validationErrors={[]}
        onValidate={vi.fn(async () => true)}
        onSubmit={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(baseElement).toMatchSnapshot();
  });
});
