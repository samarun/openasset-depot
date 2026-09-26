import { CheckCircle2, MessageCircle, Plus, RefreshCcw, X } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import type { ReviewRequest } from "../types/domain";

interface ReviewsProps {
  requests: ReviewRequest[];
  currentUser?: string;
  loading: boolean;
  busy?: boolean;
  onRefresh: () => void;
  onCreate: () => void;
  onDecide: (request: ReviewRequest, decision: "approved" | "changes_requested") => void;
  onClose: (request: ReviewRequest) => void;
  onOpenAsset: (request: ReviewRequest) => void;
}

/**
 * Sign-off queue for submitted revisions.
 *
 * Distinct from Review & Annotate: that is markup on an immutable revision.
 * This page is the workflow that asks named people to approve those bytes.
 */
export function Reviews({
  requests,
  currentUser,
  loading,
  busy = false,
  onRefresh,
  onCreate,
  onDecide,
  onClose,
  onOpenAsset,
}: ReviewsProps) {
  const awaiting = requests.filter((request) => pendingFor(request, currentUser));
  const open = requests.filter((request) => request.state === "open");

  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">Reviews</span>
          <h1>Sign-off</h1>
        </div>
        <div className="button-row">
          <button className="secondary-button" type="button" onClick={onRefresh} disabled={loading}>
            <RefreshCcw size={17} />
            {loading ? "Refreshing" : "Refresh"}
          </button>
          <button className="primary-button" type="button" onClick={onCreate} disabled={busy}>
            <Plus size={17} />
            Request Review
          </button>
        </div>
      </header>
      {requests.length === 0 ? (
        <EmptyState
          icon={MessageCircle}
          title="No reviews yet"
          detail="Ask a teammate to sign off on a submitted revision. Approvals name the exact bytes, so a later submit needs a new review."
          action={
            <button className="primary-button" type="button" onClick={onCreate}>
              Request Review
            </button>
          }
        />
      ) : (
        <section className="content-grid two-columns">
          <ReviewSection
            title="Awaiting you"
            requests={awaiting}
            emptyCopy="Nothing waiting on you."
            currentUser={currentUser}
            busy={busy}
            onDecide={onDecide}
            onClose={onClose}
            onOpenAsset={onOpenAsset}
          />
          <ReviewSection
            title="Open on this stream"
            requests={open}
            emptyCopy="No open reviews."
            currentUser={currentUser}
            busy={busy}
            onDecide={onDecide}
            onClose={onClose}
            onOpenAsset={onOpenAsset}
          />
          <ReviewSection
            title="All reviews"
            requests={requests}
            emptyCopy="No reviews on this stream."
            currentUser={currentUser}
            busy={busy}
            onDecide={onDecide}
            onClose={onClose}
            onOpenAsset={onOpenAsset}
          />
        </section>
      )}
    </main>
  );
}

function ReviewSection({
  title,
  requests,
  emptyCopy,
  currentUser,
  busy,
  onDecide,
  onClose,
  onOpenAsset,
}: {
  title: string;
  requests: ReviewRequest[];
  emptyCopy: string;
  currentUser?: string;
  busy: boolean;
  onDecide: (request: ReviewRequest, decision: "approved" | "changes_requested") => void;
  onClose: (request: ReviewRequest) => void;
  onOpenAsset: (request: ReviewRequest) => void;
}) {
  return (
    <section className="panel">
      <header className="section-heading">
        <h2>{title}</h2>
        <span>{requests.length}</span>
      </header>
      {requests.length === 0 ? (
        <p className="subtle-copy">{emptyCopy}</p>
      ) : (
        <div className="card-list compact-cards">
          {requests.map((request) => (
            <article className="review-card" key={request.id}>
              <header className="card-header-row">
                <div>
                  <h3>{request.title}</h3>
                  <p>
                    {request.path} · r{request.revision_number} · {request.requester}
                  </p>
                </div>
                <strong className={`review-state is-${request.state}`}>{stateLabel(request.state)}</strong>
              </header>
              <div className="compact-list">
                {request.reviewers.map((reviewer) => (
                  <span className="compact-row" key={reviewer.reviewer_user_id}>
                    <span>{reviewer.reviewer}</span>
                    <span>{decisionLabel(reviewer.decision)}</span>
                  </span>
                ))}
              </div>
              <div className="review-status-row">
                <button className="ghost-button" type="button" onClick={() => onOpenAsset(request)}>
                  Open asset
                </button>
                {pendingFor(request, currentUser) && (
                  <>
                    <button
                      className="primary-button"
                      type="button"
                      disabled={busy}
                      onClick={() => onDecide(request, "approved")}
                    >
                      <CheckCircle2 size={16} />
                      Approve
                    </button>
                    <button
                      className="secondary-button"
                      type="button"
                      disabled={busy}
                      onClick={() => onDecide(request, "changes_requested")}
                    >
                      Request changes
                    </button>
                  </>
                )}
                {canClose(request, currentUser) && (
                  <button
                    className="ghost-button danger-text-button"
                    type="button"
                    disabled={busy}
                    onClick={() => onClose(request)}
                  >
                    <X size={16} />
                    Close
                  </button>
                )}
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

function pendingFor(request: ReviewRequest, currentUser?: string): boolean {
  if (!currentUser || request.state === "closed") return false;
  return request.reviewers.some(
    (reviewer) => reviewer.reviewer === currentUser && reviewer.decision === "pending",
  );
}

function canClose(request: ReviewRequest, currentUser?: string): boolean {
  return request.state !== "closed" && request.requester === currentUser;
}

function stateLabel(state: ReviewRequest["state"]): string {
  if (state === "changes_requested") return "Changes requested";
  return state.charAt(0).toUpperCase() + state.slice(1);
}

function decisionLabel(decision: ReviewRequest["reviewers"][number]["decision"]): string {
  if (decision === "changes_requested") return "Changes requested";
  return decision.charAt(0).toUpperCase() + decision.slice(1);
}
