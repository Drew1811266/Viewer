PRAGMA foreign_keys = ON;

CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at_ms INTEGER NOT NULL
);

CREATE TABLE operation_batches (
  batch_id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  completed_at_ms INTEGER
);

CREATE TABLE operation_items (
  operation_id TEXT PRIMARY KEY,
  batch_id TEXT NOT NULL REFERENCES operation_batches(batch_id),
  entity_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  state TEXT NOT NULL,
  source_path TEXT NOT NULL,
  destination_path TEXT,
  temporary_path TEXT,
  expected_size INTEGER,
  expected_hash BLOB,
  conflict_policy TEXT NOT NULL,
  error_code TEXT,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX operation_items_incomplete
ON operation_items(state)
WHERE state NOT IN ('completed', 'failed');

INSERT INTO schema_migrations(version, applied_at_ms) VALUES (1, 0);
