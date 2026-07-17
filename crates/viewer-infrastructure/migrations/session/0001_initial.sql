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
  favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
  image_width INTEGER CHECK (image_width > 0),
  image_height INTEGER CHECK (image_height > 0),
  image_status INTEGER NOT NULL DEFAULT 0 CHECK (image_status IN (0, 1, 2)),
  text_status INTEGER NOT NULL DEFAULT 0 CHECK (text_status IN (0, 1, 2, 3, 4))
);

CREATE INDEX IF NOT EXISTS nodes_parent ON nodes(parent_entity_id, name);
CREATE INDEX IF NOT EXISTS nodes_kind ON nodes(kind);
CREATE INDEX IF NOT EXISTS nodes_review_state ON nodes(review_state);
CREATE INDEX IF NOT EXISTS nodes_favorite ON nodes(favorite);
CREATE INDEX IF NOT EXISTS nodes_size ON nodes(size);
CREATE INDEX IF NOT EXISTS nodes_image_dimensions ON nodes(image_width, image_height);

CREATE VIRTUAL TABLE IF NOT EXISTS text_fts USING fts5(
  entity_id UNINDEXED,
  relative_path,
  body,
  tokenize = 'trigram'
);
