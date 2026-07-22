import assert from 'node:assert/strict'
import { DatabaseSync } from 'node:sqlite'
import { mkdtemp, mkdir, readFile, stat, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { validatePortableMetadata } from './validate-m2-portable-metadata.mjs'

const PROJECT_ID = '00000000-0000-4000-8000-000000000001'
const MARKER_ID = '00000000-0000-4000-8000-000000000002'
const BATCH_ID = '00000000-0000-4000-8000-000000000003'
const OPERATION_IDS = [
  '00000000-0000-4000-8000-000000000004',
  '00000000-0000-4000-8000-000000000005',
  '00000000-0000-4000-8000-000000000006',
]
const ENTITY_IDS = [
  '00000000-0000-4000-8000-000000000007',
  '00000000-0000-4000-8000-000000000008',
  '00000000-0000-4000-8000-000000000009',
]
const LICENSE = 'Apache-2.0'
const MIGRATIONS = await Promise.all([
  readFile(new URL('../crates/viewer-infrastructure/migrations/portable/0001_initial.sql', import.meta.url), 'utf8'),
  readFile(new URL('../crates/viewer-infrastructure/migrations/portable/0002_markers.sql', import.meta.url), 'utf8'),
  readFile(new URL('../crates/viewer-infrastructure/migrations/portable/0003_operation_results.sql', import.meta.url), 'utf8'),
])

if (process.env.VIEWER_M3_LIVE_FIXTURE) {
  await writePortableFixture(process.env.VIEWER_M3_LIVE_FIXTURE)
}

const validateV3 = (root, options = {}) =>
  validatePortableMetadata(root, {
    license: LICENSE,
    databaseSchemaVersion: 3,
    ...options,
  })

test('accepts only schema v3, exact SQLite sidecars and every retained prior backup', async () => {
  const root = await portableFixture()
  const viewer = join(root, '.viewer')
  await Promise.all([
    writeFile(join(viewer, 'metadata.sqlite-journal'), ''),
    writeFile(join(viewer, 'metadata.sqlite-wal'), ''),
    writeFile(join(viewer, 'metadata.sqlite-shm'), ''),
  ])
  createDatabase(join(viewer, 'metadata.sqlite.v1.bak'), 1, { withRows: false })
  createDatabase(join(viewer, 'metadata.sqlite.v2.bak'), 2, { withRows: false })

  const result = await validateV3(root)

  assert.equal(result.schemaVersion, 3)
  assert.equal(result.manifestSchemaVersion, 1)
  assert.equal(result.projectId, PROJECT_ID)
  assert.equal(result.markerCount, 1)
  assert.equal(result.operationCount, 3)
  assert.deepEqual(result.backups, ['metadata.sqlite.v1.bak', 'metadata.sqlite.v2.bak'])
  assert.deepEqual(result.transientFiles.sort(), [
    'metadata.sqlite-journal',
    'metadata.sqlite-shm',
    'metadata.sqlite-wal',
  ])
})

test('accepts canonical versionless UUID entity IDs derived from filesystem identity', async () => {
  const root = await portableFixture()
  mutate(
    root,
    `UPDATE operation_items
     SET entity_id = '00000000-0100-0012-0000-00000024ba10'
     WHERE operation_id = '${OPERATION_IDS[0]}'`,
  )

  const result = await validateV3(root)

  assert.equal(result.operationCount, 3)
})

test('accepts a terminal copy result with a registered cleanup obligation', async () => {
  const root = await portableFixture()
  mutate(
    root,
    `UPDATE operation_items
     SET temporary_path = 'selected/.viewer-copy-obligation.part'
     WHERE operation_id = '${OPERATION_IDS[1]}'`,
  )

  const result = await validateV3(root)

  assert.equal(result.operationCount, 3)
})

test('rejects content disguised as an allowed SQLite sidecar', async (t) => {
  for (const [name, contents] of [
    ['metadata.sqlite-journal', Buffer.from([0xff, 0xd8, 0xff, 0xe0, 0, 16])],
    ['metadata.sqlite-wal', Buffer.from('private prompt text')],
    ['metadata.sqlite-shm', Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a])],
  ]) {
    await t.test(name, async () => {
      const root = await portableFixture()
      await writeFile(join(root, '.viewer', name), contents)
      await assert.rejects(validateV3(root), /SQLite sidecar/i)
    })
  }
})

test('rejects a portable database left in WAL journal mode', async () => {
  const root = await portableFixture()
  const path = join(root, '.viewer', 'metadata.sqlite')
  const database = new DatabaseSync(path)
  assert.equal(database.prepare('PRAGMA journal_mode = WAL').get().journal_mode, 'wal')
  database.close()

  await assert.rejects(validateV3(root), /DELETE journal mode/i)
})

test('rejects originals, text bodies, thumbnails, proxies and unknown portable artifacts', async (t) => {
  for (const name of [
    'front.jpg',
    'prompt.txt',
    'thumbnail.bin',
    'preview-proxy.cache',
    'originals',
    'notes.json',
  ]) {
    await t.test(name, async () => {
      const root = await portableFixture()
      await writeFile(join(root, '.viewer', name), 'forbidden content')
      await assert.rejects(validateV3(root), /forbidden portable artifact/i)
    })
  }
})

test('rejects every symlink including names that resemble allowed SQLite files', async (t) => {
  for (const name of ['metadata.sqlite-wal', 'metadata.sqlite.v2.bak']) {
    await t.test(name, async () => {
      const root = await portableFixture()
      const outside = join(root, 'outside')
      await writeFile(outside, 'not portable metadata')
      await symlink(outside, join(root, '.viewer', name))
      await assert.rejects(validateV3(root), /symbolic link/i)
    })
  }
})

test('requires the current schema and rejects current or future-version backups', async (t) => {
  await t.test('current v2 database', async () => {
    const root = await portableFixture({ databaseVersion: 2 })
    await assert.rejects(validateV3(root), /database schema version/i)
  })
  for (const version of [3, 4]) {
    await t.test(`v${version} backup`, async () => {
      const root = await portableFixture()
      createDatabase(join(root, '.viewer', `metadata.sqlite.v${version}.bak`), version, {
        withRows: false,
      })
      await assert.rejects(validateV3(root), /backup version|forbidden portable artifact/i)
    })
  }
})

test('rejects unknown tables and columns even when they contain plausible local data', async (t) => {
  await t.test('text body table', async () => {
    const root = await portableFixture()
    mutate(root, 'CREATE TABLE text_bodies(relative_path TEXT, body TEXT)')
    await assert.rejects(validateV3(root), /unexpected portable database table/i)
  })
  await t.test('cache path column', async () => {
    const root = await portableFixture()
    mutate(root, 'ALTER TABLE markers ADD COLUMN cache_path TEXT')
    await assert.rejects(validateV3(root), /unexpected portable database columns/i)
  })
  await t.test('unknown view', async () => {
    const root = await portableFixture()
    mutate(root, 'CREATE VIEW marker_paths AS SELECT relative_path FROM markers')
    await assert.rejects(validateV3(root), /schema object/i)
  })
  await t.test('missing operation batch foreign key', async () => {
    const root = await portableFixture()
    mutate(
      root,
      `PRAGMA foreign_keys = OFF;
       ALTER TABLE operation_items RENAME TO operation_items_old;
       CREATE TABLE operation_items (
         operation_id TEXT PRIMARY KEY,
         batch_id TEXT NOT NULL,
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
         updated_at_ms INTEGER NOT NULL,
         result_code TEXT CHECK (
           result_code IS NULL OR (
             length(result_code) BETWEEN 1 AND 64
             AND result_code NOT GLOB '*[^a-z0-9_]*'
           )
         )
       );
       INSERT INTO operation_items SELECT * FROM operation_items_old;
       DROP TABLE operation_items_old;
       CREATE INDEX operation_items_incomplete
       ON operation_items(state)
       WHERE state NOT IN ('completed', 'failed');`,
    )
    await assert.rejects(validateV3(root), /foreign key/i)
  })
})

test('rejects exact v3 column-definition and check-constraint drift', async (t) => {
  const drifts = [
    {
      label: 'column type',
      table: 'markers',
      from: 'kind INTEGER NOT NULL',
      to: 'kind TEXT NOT NULL',
      error: /column definition/i,
    },
    {
      label: 'not-null flag',
      table: 'markers',
      from: 'updated_at_ms INTEGER NOT NULL',
      to: 'updated_at_ms INTEGER',
      error: /column definition/i,
    },
    {
      label: 'default value',
      table: 'markers',
      from: 'favorite INTEGER NOT NULL DEFAULT 0',
      to: 'favorite INTEGER NOT NULL DEFAULT 1',
      error: /column definition/i,
    },
    {
      label: 'primary key flag',
      apply: removeSchemaMigrationPrimaryKey,
      error: /column definition/i,
    },
    {
      label: 'v3 batch lifecycle check',
      table: 'operation_batches',
      from: "CHECK (state IN ('running', 'completed'))",
      to: "CHECK (state IN ('queued', 'running', 'completed'))",
      error: /check constraint/i,
    },
    {
      label: 'v3 batch counter check',
      table: 'operation_batches',
      from: 'CHECK (requested_count >= 0)',
      to: 'CHECK (requested_count >= -1)',
      error: /check constraint/i,
    },
    {
      label: 'v3 item result check',
      table: 'operation_items',
      from: 'length(result_code) BETWEEN 1 AND 64',
      to: 'length(result_code) BETWEEN 0 AND 64',
      error: /check constraint/i,
    },
  ]

  for (const drift of drifts) {
    await t.test(drift.label, async () => {
      const root = await portableFixture({ withRows: false })
      if (drift.apply) {
        drift.apply(root)
      } else {
        rewriteTableSchema(root, drift.table, drift.from, drift.to)
      }
      await assert.rejects(validateV3(root), drift.error)
    })
  }
})

test('rejects unknown enums, impossible terminal transitions and unreviewed result codes', async (t) => {
  await t.test('operation state', async () => {
    const root = await portableFixture()
    mutate(root, `UPDATE operation_items SET state = 'invented' WHERE operation_id = '${OPERATION_IDS[0]}'`)
    await assert.rejects(validateV3(root), /operation state/i)
  })
  await t.test('conflict policy', async () => {
    const root = await portableFixture()
    mutate(root, `UPDATE operation_items SET conflict_policy = 'overwrite' WHERE operation_id = '${OPERATION_IDS[0]}'`)
    await assert.rejects(validateV3(root), /conflict policy/i)
  })
  await t.test('terminal item missing result', async () => {
    const root = await portableFixture()
    mutate(root, `UPDATE operation_items SET result_code = NULL WHERE operation_id = '${OPERATION_IDS[0]}'`)
    await assert.rejects(validateV3(root), /terminal operation result/i)
  })
  await t.test('non-terminal item with result', async () => {
    const root = await portableFixture({ batchState: 'running' })
    mutate(
      root,
      `UPDATE operation_items SET state = 'prepared', result_code = 'completed' WHERE operation_id = '${OPERATION_IDS[0]}'`,
    )
    await assert.rejects(validateV3(root), /non-terminal operation result/i)
  })
  await t.test('unknown stable-shaped result', async () => {
    const root = await portableFixture()
    mutate(root, `UPDATE operation_items SET result_code = 'surprise' WHERE operation_id = '${OPERATION_IDS[0]}'`)
    await assert.rejects(validateV3(root), /result code/i)
  })
})

test('rejects counter, batch-state, batch-kind and foreign-key inconsistencies', async (t) => {
  await t.test('counter total', async () => {
    const root = await portableFixture()
    mutate(root, `UPDATE operation_batches SET completed_count = 2 WHERE batch_id = '${BATCH_ID}'`)
    await assert.rejects(validateV3(root), /operation batch counters/i)
  })
  await t.test('completed and skipped counters exchanged', async () => {
    const root = await portableFixture()
    mutate(
      root,
      `UPDATE operation_batches SET completed_count = 2, skipped_count = 0 WHERE batch_id = '${BATCH_ID}'`,
    )
    await assert.rejects(validateV3(root), /operation batch counters/i)
  })
  await t.test('completed batch timestamp', async () => {
    const root = await portableFixture()
    mutate(root, `UPDATE operation_batches SET completed_at_ms = NULL WHERE batch_id = '${BATCH_ID}'`)
    await assert.rejects(validateV3(root), /operation batch lifecycle/i)
  })
  await t.test('item kind differs from batch', async () => {
    const root = await portableFixture()
    mutate(root, `UPDATE operation_items SET kind = 'move' WHERE operation_id = '${OPERATION_IDS[0]}'`)
    await assert.rejects(validateV3(root), /operation batch kind/i)
  })
  await t.test('orphan item', async () => {
    const root = await portableFixture()
    mutate(root, `PRAGMA foreign_keys = OFF; UPDATE operation_items SET batch_id = '00000000-0000-4000-8000-000000000099' WHERE operation_id = '${OPERATION_IDS[0]}'`)
    await assert.rejects(validateV3(root), /foreign key|operation batch/i)
  })
})

test('rejects absolute, traversal, reserved and cache paths from every portable path field', async (t) => {
  for (const [field, value] of [
    ['source_path', '/Users/private/front.jpg'],
    ['destination_path', '../outside.jpg'],
    ['temporary_path', 'id/.viewer/copy.part'],
    ['source_path', '/Users/a/Library/Caches/Viewer/session/front.jpg'],
  ]) {
    await t.test(`${field}: ${value}`, async () => {
      const root = await portableFixture()
      mutate(root, `UPDATE operation_items SET ${field} = ${sqlString(value)} WHERE operation_id = '${OPERATION_IDS[0]}'`)
      await assert.rejects(validateV3(root), /unsafe relative path/i)
    })
  }
})

test('requires reviewed Apache-2.0 license metadata', async () => {
  const root = await portableFixture()
  await assert.rejects(
    validatePortableMetadata(root, { databaseSchemaVersion: 3 }),
    /Apache-2\.0 license metadata/i,
  )
  await assert.rejects(validateV3(root, { license: 'MIT' }), /Apache-2\.0 license metadata/i)
})

test('defines one executable M3 gate containing every inherited and new contract', async () => {
  const [packageText, gate, gateMetadata] = await Promise.all([
    readFile(new URL('../package.json', import.meta.url), 'utf8'),
    readFile(new URL('./run-m3-organization-gate.sh', import.meta.url), 'utf8'),
    stat(new URL('./run-m3-organization-gate.sh', import.meta.url)),
  ])
  assert.equal(JSON.parse(packageText).scripts['gate:m3'], './scripts/run-m3-organization-gate.sh')
  assert.notEqual(gateMetadata.mode & 0o111, 0, 'M3 gate must be executable')
  for (const command of [
    'pnpm gate:m2',
    'pnpm --dir ui test',
    'pnpm --dir ui build',
    'cargo fmt --all -- --check',
    'cargo clippy --locked --workspace --all-targets -- -D warnings',
    'cargo test --locked --workspace',
    './scripts/run-g2-file-transaction-gate.sh',
    './scripts/run-g3-scan-search-gate.sh',
    'node --test scripts/repository-policy.test.mjs',
    './scripts/check-locked-dependencies.sh',
    './scripts/check-tauri-security.sh',
    'node scripts/check-scope-coverage.mjs',
    'node --test scripts/m3-portable-metadata.test.mjs',
    'node scripts/validate-m2-portable-metadata.mjs',
  ]) {
    assert.ok(gate.includes(command), `M3 gate is missing: ${command}`)
  }
  for (const suite of [
    'm3_command_service',
    'm3_file_commands',
    'm3_operation_journal',
    'm3_operation_projections',
    'm3_rename_preflight',
    'm3_undo',
    'm3_watcher_runtime',
    'm3_desktop_runtime',
    'm3_drag_export',
    'm3_compare_budget',
    'm3_readonly_lifecycle',
  ]) {
    assert.ok(gate.includes(suite), `M3 gate is missing suite: ${suite}`)
  }
})

export async function portableFixture({
  databaseVersion = 3,
  batchState = 'completed',
  withRows = databaseVersion >= 3,
} = {}) {
  const root = await mkdtemp(join(tmpdir(), 'viewer-m3-portable-'))
  await writePortableFixture(root, { databaseVersion, batchState, withRows })
  return root
}

export async function writePortableFixture(
  root,
  { databaseVersion = 3, batchState = 'completed', withRows = databaseVersion >= 3 } = {},
) {
  const viewer = join(root, '.viewer')
  await mkdir(viewer)
  await writeFile(
    join(viewer, 'project.json'),
    `${JSON.stringify({ schemaVersion: 1, projectId: PROJECT_ID, createdAtMs: 1 })}\n`,
  )
  createDatabase(join(viewer, 'metadata.sqlite'), databaseVersion, {
    withRows,
    batchState,
  })
  return root
}

function createDatabase(path, version, { withRows = false, batchState = 'completed' } = {}) {
  const database = new DatabaseSync(path)
  database.exec('PRAGMA foreign_keys = ON;')
  for (const migration of MIGRATIONS.slice(0, version)) database.exec(migration)
  if (version >= 2) {
    database.exec(`INSERT INTO project_metadata VALUES (1, '${PROJECT_ID}', 1);`)
  }
  if (withRows) insertValidRows(database, batchState)
  database.close()
}

function insertValidRows(database, batchState) {
  database.exec(`
    INSERT INTO markers VALUES (
      '${MARKER_ID}', 'id/front.jpg', 1, 0, 1, 10, '1', NULL, 2
    );
    INSERT INTO operation_batches(
      batch_id, kind, created_at_ms, completed_at_ms, state, requested_count,
      completed_count, failed_count, skipped_count, started_at_ms
    ) VALUES (
      '${BATCH_ID}', 'copy', 3, ${batchState === 'completed' ? 10 : 'NULL'},
      '${batchState}', 3, 1, 1, 1, 3
    );
    INSERT INTO operation_items VALUES
      ('${OPERATION_IDS[0]}', '${BATCH_ID}', '${ENTITY_IDS[0]}', 'copy', 'completed',
       'id/front.jpg', 'selected/front.jpg', NULL, 10, zeroblob(32), 'skip', NULL, 7,
       'completed'),
      ('${OPERATION_IDS[1]}', '${BATCH_ID}', '${ENTITY_IDS[1]}', 'copy', 'failed',
       'id/back.jpg', 'selected/back.jpg', NULL, NULL, NULL, 'skip', 'permission_denied', 8,
       'permission_denied'),
      ('${OPERATION_IDS[2]}', '${BATCH_ID}', '${ENTITY_IDS[2]}', 'copy', 'completed',
       'id/side.jpg', 'selected/side.jpg', NULL, NULL, NULL, 'skip', NULL, 9,
       'conflict_skipped');
  `)
}

function mutate(root, sql) {
  const database = new DatabaseSync(join(root, '.viewer', 'metadata.sqlite'))
  database.exec('PRAGMA ignore_check_constraints = ON;')
  database.exec(sql)
  database.close()
}

function rewriteTableSchema(root, table, from, to) {
  const database = new DatabaseSync(join(root, '.viewer', 'metadata.sqlite'))
  database.enableDefensive(false)
  database.exec('PRAGMA writable_schema = ON;')
  const result = database
    .prepare("UPDATE sqlite_schema SET sql = replace(sql, ?, ?) WHERE type = 'table' AND name = ?")
    .run(from, to, table)
  database.exec('PRAGMA writable_schema = OFF;')
  database.close()
  assert.equal(result.changes, 1, `${table} schema drift fixture did not match`)
}

function removeSchemaMigrationPrimaryKey(root) {
  const database = new DatabaseSync(join(root, '.viewer', 'metadata.sqlite'))
  database.exec(`
    ALTER TABLE schema_migrations RENAME TO schema_migrations_old;
    CREATE TABLE schema_migrations (
      version INTEGER,
      applied_at_ms INTEGER NOT NULL
    );
    INSERT INTO schema_migrations SELECT version, applied_at_ms FROM schema_migrations_old;
    DROP TABLE schema_migrations_old;
  `)
  database.close()
}

function sqlString(value) {
  return `'${value.replaceAll("'", "''")}'`
}
