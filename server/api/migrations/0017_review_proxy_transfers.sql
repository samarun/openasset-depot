-- Resumable review-proxy uploads and exact source timebase metadata.

-- Upload purposes isolate resumable sessions that share a depot path. A scene
-- submit and its generated review proxy may be in flight at the same time, but
-- must never resume one another's bytes.
ALTER TABLE upload_sessions
    ADD COLUMN purpose TEXT NOT NULL DEFAULT 'content'
    CHECK (purpose IN ('content', 'review_proxy'));

DROP INDEX idx_upload_sessions_open_target;

CREATE UNIQUE INDEX idx_upload_sessions_open_target
    ON upload_sessions (user_id, workspace_id, depot_path, purpose)
    WHERE finalized_at IS NULL;

-- Frame rates are stored as a rational number so common studio rates such as
-- 24000/1001 and 30000/1001 do not lose precision. Legacy proxies keep these
-- columns NULL and the review UI omits frame labels rather than guessing 24 fps.
ALTER TABLE revision_review_proxies
    ADD COLUMN frame_rate_numerator INTEGER,
    ADD COLUMN frame_rate_denominator INTEGER,
    ADD COLUMN start_frame INTEGER;

ALTER TABLE revision_review_proxies
    ADD CONSTRAINT revision_review_proxies_timebase_complete CHECK (
        (frame_rate_numerator IS NULL
            AND frame_rate_denominator IS NULL
            AND start_frame IS NULL)
        OR
        (frame_rate_numerator > 0
            AND frame_rate_denominator > 0
            AND start_frame >= 0)
    );
