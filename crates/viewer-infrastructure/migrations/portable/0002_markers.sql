CREATE TABLE project_metadata (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  project_id TEXT NOT NULL UNIQUE,
  created_at_ms INTEGER NOT NULL
);

CREATE TABLE markers (
  marker_id TEXT PRIMARY KEY,
  relative_path TEXT NOT NULL UNIQUE,
  kind INTEGER NOT NULL,
  review_state INTEGER,
  favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
  evidence_size INTEGER,
  evidence_modified_ns TEXT,
  content_hash BLOB,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX markers_review_state ON markers(review_state);
CREATE INDEX markers_favorite ON markers(favorite);

INSERT INTO schema_migrations(version, applied_at_ms) VALUES (2, 0);
