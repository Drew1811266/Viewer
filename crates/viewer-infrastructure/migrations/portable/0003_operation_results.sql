ALTER TABLE operation_batches
ADD COLUMN state TEXT NOT NULL DEFAULT 'running'
CHECK (state IN ('running', 'completed'));

ALTER TABLE operation_batches
ADD COLUMN requested_count INTEGER NOT NULL DEFAULT 0
CHECK (requested_count >= 0);

ALTER TABLE operation_batches
ADD COLUMN completed_count INTEGER NOT NULL DEFAULT 0
CHECK (completed_count >= 0);

ALTER TABLE operation_batches
ADD COLUMN failed_count INTEGER NOT NULL DEFAULT 0
CHECK (failed_count >= 0);

ALTER TABLE operation_batches
ADD COLUMN skipped_count INTEGER NOT NULL DEFAULT 0
CHECK (skipped_count >= 0);

ALTER TABLE operation_batches
ADD COLUMN started_at_ms INTEGER
CHECK (started_at_ms IS NULL OR started_at_ms >= 0);

ALTER TABLE operation_items
ADD COLUMN result_code TEXT
CHECK (
  result_code IS NULL OR (
    length(result_code) BETWEEN 1 AND 64
    AND result_code NOT GLOB '*[^a-z0-9_]*'
  )
);

UPDATE operation_items
SET error_code = 'legacy_failure'
WHERE error_code IS NOT NULL
  AND (
    length(error_code) NOT BETWEEN 1 AND 64
    OR error_code GLOB '*[^a-z0-9_]*'
  );

UPDATE operation_items
SET result_code = CASE
  WHEN state = 'completed' THEN 'completed'
  WHEN state = 'failed' THEN COALESCE(error_code, 'failed')
  ELSE NULL
END;

UPDATE operation_batches
SET requested_count = (
      SELECT COUNT(*) FROM operation_items
      WHERE operation_items.batch_id = operation_batches.batch_id
    ),
    completed_count = (
      SELECT COUNT(*) FROM operation_items
      WHERE operation_items.batch_id = operation_batches.batch_id
        AND operation_items.state = 'completed'
    ),
    failed_count = (
      SELECT COUNT(*) FROM operation_items
      WHERE operation_items.batch_id = operation_batches.batch_id
        AND operation_items.state = 'failed'
    ),
    state = CASE WHEN completed_at_ms IS NULL THEN 'running' ELSE 'completed' END,
    started_at_ms = created_at_ms;

INSERT INTO schema_migrations(version, applied_at_ms) VALUES (3, 0);
