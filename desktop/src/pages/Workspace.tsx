import { Folder, FolderSearch, FolderTree, Grid2X2, List, PanelRight, Plus, RotateCcw, UploadCloud } from "lucide-react";
import { useMemo, useState } from "react";
import { AssetCard } from "../components/AssetCard";
import { EmptyState } from "../components/EmptyState";
import { FileInspector } from "../components/FileInspector";
import { FileRow } from "../components/FileRow";
import type { AssetFile } from "../types/domain";

interface WorkspaceProps {
  files: AssetFile[];
  selectedFile?: AssetFile;
  onSelectFile: (file: AssetFile) => void;
  onSync: () => void;
  onChooseFiles: () => void;
  onLock: (file: AssetFile) => void;
  onUnlock: (file: AssetFile) => void;
  onRevert: (file: AssetFile) => void;
  onDelete: (file: AssetFile) => void;
  onSubmit: () => void;
  onHistory: () => void;
  hasMoreFiles?: boolean;
  onLoadMore?: () => void;
  busy?: boolean;
}

export function Workspace({
  files,
  selectedFile,
  onSelectFile,
  onSync,
  onChooseFiles,
  onLock,
  onUnlock,
  onRevert,
  onDelete,
  onSubmit,
  onHistory,
  hasMoreFiles = false,
  onLoadMore,
  busy = false,
}: WorkspaceProps) {
  const [assetView, setAssetView] = useState<"artist" | "technical">("artist");
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const folders = useMemo(() => Array.from(new Set(files.map((file) => file.path.split("/")[0]))), [files]);

  function inspectFile(file: AssetFile) {
    onSelectFile(file);
    setInspectorOpen(true);
  }

  return (
    <main className="workspace-layout">
      <aside className="folder-pane">
        <header className="section-heading">
          <h2>Project Files</h2>
        </header>
        {folders.length > 0 ? (
          <nav className="folder-list" aria-label="Project folders">
            {folders.map((folder) => (
              <button key={folder} type="button">
                <Folder size={16} />
                {folder}
              </button>
            ))}
          </nav>
        ) : (
          <div className="folder-empty">
            <FolderTree size={20} />
            <span>Project folders appear after the first submit.</span>
          </div>
        )}
      </aside>
      <section className="file-browser">
        <header className="page-heading compact-heading">
          <div>
            <span className="eyebrow">Workspace</span>
            <h1>Asset Browser</h1>
          </div>
          <div className="segmented-control" aria-label="Workspace view">
            <button className={assetView === "artist" ? "is-active" : ""} onClick={() => setAssetView("artist")}>
              <Grid2X2 size={16} />
              Asset
            </button>
            <button
              className={assetView === "technical" ? "is-active" : ""}
              onClick={() => setAssetView("technical")}
            >
              <List size={16} />
              Path
            </button>
          </div>
        </header>
        <div className="button-row toolbar-row">
          <button className="primary-button" type="button" onClick={onSync} disabled={busy}>
            <UploadCloud size={16} />
            Sync Latest
          </button>
          <button
            className="secondary-button"
            type="button"
            onClick={onChooseFiles}
            disabled={busy}
          >
            <Plus size={16} />
            Add Files
          </button>
          <button
            className="ghost-button"
            type="button"
            onClick={() => selectedFile && onRevert(selectedFile)}
            disabled={!selectedFile || busy || !selectedFile.statuses.some((status) =>
              status === "Ready to Submit" || status === "New File" || status === "Marked for Delete"
            )}
          >
            <RotateCcw size={16} />
            Revert Intent
          </button>
          <button
            className="secondary-button inspector-trigger"
            type="button"
            onClick={() => setInspectorOpen(true)}
            disabled={!selectedFile}
          >
            <PanelRight size={16} />
            Inspector
          </button>
        </div>
        {files.length === 0 ? (
          <EmptyState
            icon={FolderSearch}
            title="No versioned assets yet"
            detail="Refresh the workspace after the first file is submitted."
            action={(
              <button className="primary-button" type="button" onClick={onSync} disabled={busy}>
                <UploadCloud size={16} />
                Refresh Workspace
              </button>
            )}
          />
        ) : assetView === "artist" ? (
          <div className="asset-grid browser-grid">
            {files.map((file) => (
              <AssetCard
                key={file.id}
                file={file}
                selected={selectedFile?.id === file.id}
                onSelect={inspectFile}
              />
            ))}
          </div>
        ) : (
          <div className="file-table">
            {files.map((file) => (
              <FileRow key={file.id} file={file} selected={selectedFile?.id === file.id} onSelect={inspectFile} />
            ))}
          </div>
        )}
        {hasMoreFiles && (
          <button className="secondary-button load-more-button" type="button" onClick={onLoadMore} disabled={busy}>
            Load More Assets
          </button>
        )}
      </section>
      <FileInspector
        file={selectedFile}
        open={inspectorOpen}
        onClose={() => setInspectorOpen(false)}
        onSubmit={onSubmit}
        onHistory={onHistory}
        onLock={onLock}
        onUnlock={onUnlock}
        onRevert={onRevert}
        onDelete={onDelete}
        busy={busy}
      />
    </main>
  );
}
