import assert from 'node:assert/strict'
import { DatabaseSync } from 'node:sqlite'
import { mkdtemp, mkdir, readFile, stat, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { validatePortableMetadata } from './validate-m2-portable-metadata.mjs'

const PROJECT_ID = '00000000-0000-4000-8000-000000000001'
const MARKER_ID = '00000000-0000-4000-8000-000000000002'

test('accepts only the current portable files, live SQLite sidecars, and a v1 backup', async () => {
  const root = await portableFixture()
  const viewer = join(root, '.viewer')
  await Promise.all([
    writeFile(join(viewer, 'metadata.sqlite-journal'), ''),
    writeFile(join(viewer, 'metadata.sqlite-wal'), ''),
    writeFile(join(viewer, 'metadata.sqlite-shm'), ''),
  ])
  createDatabase(join(viewer, 'metadata.sqlite.v1.bak'), 1)

  const result = await validatePortableMetadata(root, { license: 'Apache-2.0' })

  assert.equal(result.schemaVersion, 2)
  assert.equal(result.manifestSchemaVersion, 1)
  assert.equal(result.projectId, PROJECT_ID)
  assert.deepEqual(result.transientFiles.sort(), [
    'metadata.sqlite-journal',
    'metadata.sqlite-shm',
    'metadata.sqlite-wal',
  ])
})

test('rejects originals, text bodies, thumbnails, proxies, and unknown artifacts', async (t) => {
  for (const name of [
    'front.jpg',
    'prompt.txt',
    'thumbnail.bin',
    'preview-proxy.cache',
    'notes.json',
  ]) {
    await t.test(name, async () => {
      const root = await portableFixture()
      await writeFile(join(root, '.viewer', name), 'forbidden content')
      await assert.rejects(
        validatePortableMetadata(root, { license: 'Apache-2.0' }),
        /forbidden portable artifact/i,
      )
    })
  }
})

test('rejects symlinks even when their names resemble allowed SQLite sidecars', async () => {
  const root = await portableFixture()
  const outside = join(root, 'outside')
  await writeFile(outside, 'not a journal')
  await symlink(outside, join(root, '.viewer', 'metadata.sqlite-journal'))

  await assert.rejects(
    validatePortableMetadata(root, { license: 'Apache-2.0' }),
    /symbolic link/i,
  )
})

test('rejects unsupported manifest and database schema versions', async (t) => {
  await t.test('manifest', async () => {
    const root = await portableFixture()
    await writeFile(
      join(root, '.viewer', 'project.json'),
      JSON.stringify({ schemaVersion: 2, projectId: PROJECT_ID, createdAtMs: 1 }),
    )
    await assert.rejects(
      validatePortableMetadata(root, { license: 'Apache-2.0' }),
      /manifest schema version/i,
    )
  })

  await t.test('database', async () => {
    const root = await portableFixture({ databaseVersion: 1 })
    await assert.rejects(
      validatePortableMetadata(root, { license: 'Apache-2.0' }),
      /database schema version/i,
    )
  })
})

test('rejects invalid review values, absolute paths, hidden paths, and content tables', async (t) => {
  await t.test('review state', async () => {
    const root = await portableFixture()
    mutate(root, `INSERT INTO markers VALUES ('${MARKER_ID}', 'id/front.jpg', 1, 9, 0, 1, '1', NULL, 1)`)
    await assert.rejects(
      validatePortableMetadata(root, { license: 'Apache-2.0' }),
      /invalid review state/i,
    )
  })

  for (const relativePath of ['/Users/private/front.jpg', '../front.jpg', 'id/.viewer/front.jpg']) {
    await t.test(relativePath, async () => {
      const root = await portableFixture()
      mutate(
        root,
        `INSERT INTO markers VALUES ('${MARKER_ID}', ${sqlString(relativePath)}, 1, 0, 0, 1, '1', NULL, 1)`,
      )
      await assert.rejects(
        validatePortableMetadata(root, { license: 'Apache-2.0' }),
        /unsafe relative path/i,
      )
    })
  }

  await t.test('text body table', async () => {
    const root = await portableFixture()
    mutate(root, 'CREATE TABLE text_bodies(relative_path TEXT, body TEXT)')
    await assert.rejects(
      validatePortableMetadata(root, { license: 'Apache-2.0' }),
      /unexpected portable database table/i,
    )
  })
})

test('requires the gate to provide Apache-2.0 license metadata', async () => {
  const root = await portableFixture()
  await assert.rejects(validatePortableMetadata(root, {}), /Apache-2\.0 license metadata/i)
  await assert.rejects(
    validatePortableMetadata(root, { license: 'MIT' }),
    /Apache-2\.0 license metadata/i,
  )
})

test('defines one executable layered M2 gate with every milestone contract', async () => {
  const [packageText, gate, gateMetadata] = await Promise.all([
    readFile(new URL('../package.json', import.meta.url), 'utf8'),
    readFile(new URL('./run-m2-review-gate.sh', import.meta.url), 'utf8'),
    stat(new URL('./run-m2-review-gate.sh', import.meta.url)),
  ])
  assert.equal(JSON.parse(packageText).scripts['gate:m2'], './scripts/run-m2-review-gate.sh')
  assert.notEqual(gateMetadata.mode & 0o111, 0, 'M2 gate must be executable')
  for (const command of [
    'pnpm gate:m1',
    'node --test scripts/m2-portable-metadata.test.mjs',
    'node scripts/validate-m2-portable-metadata.mjs --policy',
    'cargo test --locked --test m2_portable_metadata',
    'cargo test --locked --test m2_session_projection',
    'cargo test --locked --test m2_derived_indexing',
    'cargo test --locked --test m2_search_queries',
    'cargo test --locked --test m2_browse_projections',
    'cargo test --locked -p viewer-desktop --test m2_desktop_runtime',
    'cargo test --locked -p viewer-desktop --test security_boundaries',
    './scripts/run-g3-scan-search-gate.sh',
  ]) {
    assert.ok(gate.includes(command), `M2 gate is missing: ${command}`)
  }
})

async function portableFixture({ databaseVersion = 2 } = {}) {
  const root = await mkdtemp(join(tmpdir(), 'viewer-m2-portable-'))
  const viewer = join(root, '.viewer')
  await mkdir(viewer)
  await writeFile(
    join(viewer, 'project.json'),
    `${JSON.stringify({ schemaVersion: 1, projectId: PROJECT_ID, createdAtMs: 1 })}\n`,
  )
  createDatabase(join(viewer, 'metadata.sqlite'), databaseVersion)
  return root
}

function createDatabase(path, version) {
  const database = new DatabaseSync(path)
  database.exec(`
    PRAGMA foreign_keys = ON;
    CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at_ms INTEGER NOT NULL);
    CREATE TABLE operation_batches(
      batch_id TEXT PRIMARY KEY,
      kind TEXT NOT NULL,
      created_at_ms INTEGER NOT NULL,
      completed_at_ms INTEGER
    );
    CREATE TABLE operation_items(
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
    INSERT INTO schema_migrations VALUES (1, 0);
  `)
  if (version >= 2) {
    database.exec(`
      CREATE TABLE project_metadata(
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        project_id TEXT NOT NULL UNIQUE,
        created_at_ms INTEGER NOT NULL
      );
      CREATE TABLE markers(
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
      INSERT INTO schema_migrations VALUES (2, 0);
      INSERT INTO project_metadata VALUES (1, '${PROJECT_ID}', 1);
    `)
  }
  if (version > 2) {
    database.exec(`INSERT INTO schema_migrations VALUES (${version}, 0)`)
  }
  database.close()
}

function mutate(root, sql) {
  const database = new DatabaseSync(join(root, '.viewer', 'metadata.sqlite'))
  database.exec(sql)
  database.close()
}

function sqlString(value) {
  return `'${value.replaceAll("'", "''")}'`
}
