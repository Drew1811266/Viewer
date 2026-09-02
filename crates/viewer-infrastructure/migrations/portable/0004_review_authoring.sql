PRAGMA foreign_keys = ON;

CREATE TABLE review_authoring_streams (
  stream_id TEXT PRIMARY KEY CHECK (length(stream_id) = 36),
  project_id TEXT NOT NULL REFERENCES project_metadata(project_id)
    CHECK (length(project_id) = 36),
  production_scope BLOB NOT NULL CHECK (length(production_scope) <= 1024),
  authoring_seq INTEGER CHECK (authoring_seq IS NULL OR authoring_seq > 0),
  authoring_snapshot_id TEXT CHECK (
    authoring_snapshot_id IS NULL OR length(authoring_snapshot_id) = 36
  ),
  published_seq INTEGER CHECK (published_seq IS NULL OR published_seq > 0),
  published_snapshot_id TEXT CHECK (
    published_snapshot_id IS NULL OR length(published_snapshot_id) = 36
  ),
  updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0),
  CHECK ((authoring_seq IS NULL) = (authoring_snapshot_id IS NULL)),
  CHECK ((published_seq IS NULL) = (published_snapshot_id IS NULL)),
  CHECK (published_seq IS NULL OR authoring_seq IS NOT NULL),
  CHECK (published_seq IS NULL OR published_seq <= authoring_seq),
  FOREIGN KEY (stream_id, authoring_seq, authoring_snapshot_id)
    REFERENCES review_authoring_snapshots(stream_id, seq, snapshot_id),
  FOREIGN KEY (stream_id, published_seq, published_snapshot_id)
    REFERENCES review_authoring_snapshots(stream_id, seq, snapshot_id)
);

CREATE TABLE review_authoring_snapshots (
  stream_id TEXT NOT NULL CHECK (length(stream_id) = 36),
  seq INTEGER NOT NULL CHECK (seq > 0),
  snapshot_id TEXT NOT NULL CHECK (length(snapshot_id) = 36),
  command_id TEXT NOT NULL UNIQUE CHECK (length(command_id) = 36),
  parent_seq INTEGER CHECK (parent_seq IS NULL OR (parent_seq > 0 AND parent_seq < seq)),
  payload_digest BLOB NOT NULL CHECK (length(payload_digest) = 32),
  logical_state BLOB NOT NULL CHECK (length(logical_state) BETWEEN 2 AND 16777216),
  transition BLOB NOT NULL CHECK (length(transition) BETWEEN 2 AND 65536),
  generated_ids BLOB NOT NULL CHECK (length(generated_ids) BETWEEN 2 AND 65536),
  barrier_kind TEXT NOT NULL CHECK (
    barrier_kind IN ('none', 'archive', 'restore', 'migration')
  ),
  created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
  PRIMARY KEY (stream_id, seq),
  UNIQUE (stream_id, snapshot_id),
  UNIQUE (stream_id, seq, snapshot_id),
  FOREIGN KEY (stream_id) REFERENCES review_authoring_streams(stream_id),
  FOREIGN KEY (stream_id, parent_seq)
    REFERENCES review_authoring_snapshots(stream_id, seq)
);

CREATE TABLE review_materialization_jobs (
  stream_id TEXT NOT NULL CHECK (length(stream_id) = 36),
  target_seq INTEGER NOT NULL CHECK (target_seq > 0),
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'retryable', 'blocked')),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  lease_epoch INTEGER NOT NULL DEFAULT 0 CHECK (lease_epoch >= 0),
  next_attempt_at_ms INTEGER NOT NULL DEFAULT 0 CHECK (next_attempt_at_ms >= 0),
  error_code TEXT CHECK (
    error_code IS NULL OR (
      length(error_code) BETWEEN 1 AND 64
      AND error_code NOT GLOB '*[^a-z0-9_]*'
    )
  ),
  PRIMARY KEY (stream_id, target_seq),
  FOREIGN KEY (stream_id, target_seq)
    REFERENCES review_authoring_snapshots(stream_id, seq)
);

CREATE INDEX review_materialization_jobs_ready
ON review_materialization_jobs(status, next_attempt_at_ms, stream_id, target_seq);

CREATE TABLE review_evidence_action_cache (
  action_key BLOB PRIMARY KEY CHECK (length(action_key) = 32),
  base_evidence BLOB NOT NULL CHECK (length(base_evidence) BETWEEN 2 AND 4096),
  annotated_evidence BLOB CHECK (
    annotated_evidence IS NULL OR length(annotated_evidence) BETWEEN 2 AND 4096
  ),
  renderer_version TEXT NOT NULL CHECK (length(renderer_version) BETWEEN 1 AND 128),
  output_policy_version TEXT NOT NULL CHECK (length(output_policy_version) BETWEEN 1 AND 128),
  updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0)
);

INSERT INTO schema_migrations(version, applied_at_ms) VALUES (4, 0);
