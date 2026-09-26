-- Review requests turn the existing per-revision comment thread into a workflow
-- with named reviewers and a recorded decision.
--
-- Comments already exist (0010) and stay independent: a comment is feedback on a
-- revision whether or not anyone asked for a review.

CREATE TABLE review_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    revision_id UUID NOT NULL REFERENCES file_revisions(id) ON DELETE CASCADE,
    stream_id UUID NOT NULL REFERENCES streams(id) ON DELETE CASCADE,
    requested_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 200),
    description TEXT NOT NULL DEFAULT '' CHECK (char_length(description) <= 4000),
    -- `changes_requested` is still an open review: the artist is expected to act
    -- on it. Only `approved` and `closed` are terminal.
    state TEXT NOT NULL DEFAULT 'open'
        CHECK (state IN ('open', 'approved', 'changes_requested', 'closed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at TIMESTAMPTZ,
    closed_by UUID REFERENCES users(id) ON DELETE SET NULL
);

-- One live review per revision, so two leads cannot open competing reviews of
-- the same immutable asset version.
CREATE UNIQUE INDEX uq_review_requests_live_revision
    ON review_requests (revision_id)
    WHERE state IN ('open', 'changes_requested');

CREATE INDEX idx_review_requests_stream_state
    ON review_requests (stream_id, state, created_at DESC);

CREATE TABLE review_request_reviewers (
    review_request_id UUID NOT NULL REFERENCES review_requests(id) ON DELETE CASCADE,
    reviewer_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    decision TEXT NOT NULL DEFAULT 'pending'
        CHECK (decision IN ('pending', 'approved', 'changes_requested')),
    note TEXT CHECK (note IS NULL OR char_length(note) <= 2000),
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (review_request_id, reviewer_user_id)
);

-- Answers "what is waiting on me", which is the query every artist runs first.
CREATE INDEX idx_review_reviewers_pending
    ON review_request_reviewers (reviewer_user_id, decision);
