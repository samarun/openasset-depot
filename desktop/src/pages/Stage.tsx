import { Radio, RotateCcw, ShieldCheck, StepForward, UploadCloud } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { StatusBadge } from "../components/StatusBadge";
import type { StageState } from "../types/domain";

interface StageProps {
  stage?: StageState;
  onOpenWorkspace: () => void;
}

export function Stage({ stage, onOpenWorkspace }: StageProps) {
  if (!stage) {
    return (
      <main className="page">
        <header className="page-heading">
          <div><span className="eyebrow">Stage</span><h1>Live Readiness</h1></div>
        </header>
        <EmptyState
          icon={Radio}
          title="No stage is configured"
          detail="Choose a workspace before preparing render nodes for rehearsal."
          action={<button className="primary-button" type="button" onClick={onOpenWorkspace}>Open Workspace</button>}
        />
      </main>
    );
  }

  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">Stage</span>
          <h1>Live Readiness</h1>
        </div>
        <div className="button-row">
          <button className="primary-button" type="button">
            <ShieldCheck size={17} />
            Mark Rehearsal Safe
          </button>
          <button className="secondary-button" type="button">
            <StepForward size={17} />
            Mark Live Approved
          </button>
          <button className="ghost-button" type="button">
            <RotateCcw size={17} />
            Rollback Stage
          </button>
        </div>
      </header>
      <section className="stage-hero">
        <div>
          <h2>{stage.stageRevision}</h2>
          <p>Live approval: {stage.liveApproval}</p>
        </div>
        <button className="secondary-button" type="button">
          <UploadCloud size={17} />
          Sync Nodes
        </button>
      </section>
      <section className="content-grid two-columns">
        <div className="panel">
          <header className="section-heading">
            <h2>Render nodes</h2>
          </header>
          <div className="compact-list">
            {stage.renderNodes.map((node) => (
              <span key={node.name} className="compact-row">
                <span>{node.name}</span>
                <span>
                  <StatusBadge status={node.status} />
                  {node.lastSync}
                </span>
              </span>
            ))}
          </div>
        </div>
        <div className="panel">
          <header className="section-heading">
            <h2>Validation</h2>
          </header>
          <div className="compact-list">
            {stage.validations.map((validation) => (
              <span className="compact-row" key={`${validation.code}-${validation.path}`}>
                <span>{validation.path}</span>
                <span>{validation.message}</span>
              </span>
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}
