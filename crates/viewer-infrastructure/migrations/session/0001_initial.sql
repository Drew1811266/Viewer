PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS nodes (
  entity_id TEXT PRIMARY KEY,
  parent_entity_id TEXT REFERENCES nodes(entity_id) ON DELETE CASCADE,
  relative_path TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  kind INTEGER NOT NULL,
  size INTEGER NOT NULL,
  modified_ns TEXT NOT NULL,
  review_state INTEGER,
  favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1))
);

CREATE INDEX IF NOT EXISTS nodes_parent ON nodes(parent_entity_id, name);
CREATE INDEX IF NOT EXISTS nodes_kind ON nodes(kind);

CREATE VIRTUAL TABLE IF NOT EXISTS text_fts USING fts5(
  entity_id UNINDEXED,
  relative_path,
  body,
  tokenize = 'trigram'
);
