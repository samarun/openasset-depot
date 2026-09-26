-- Impact analysis asks the dependency graph the reverse question: not "what does
-- this file need" but "what needs this file". The existing indexes only cover
-- lookups by source_file, so a reverse lookup scanned the whole stream.
CREATE INDEX idx_dependency_edges_target
    ON asset_dependency_edges (stream_id, target_file);
