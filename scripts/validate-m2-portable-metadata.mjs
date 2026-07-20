#!/usr/bin/env node

import { DatabaseSync } from 'node:sqlite'
import { lstat, readFile, readdir } from 'node:fs/promises'
import { basename, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const MANIFEST_SCHEMA_VERSION = 1
const MINIMUM_DATABASE_SCHEMA_VERSION = 2
const LATEST_DATABASE_SCHEMA_VERSION = 3
const APACHE_LICENSE = 'Apache-2.0'
const REQUIRED_FILES = new Set(['project.json', 'metadata.sqlite'])
const TRANSIENT_FILES = new Set([
  'metadata.sqlite-journal',
  'metadata.sqlite-wal',
  'metadata.sqlite-shm',
])
const EXPECTED_TABLES = {
  1: ['operation_batches', 'operation_items', 'schema_migrations'],
  2: ['markers', 'operation_batches', 'operation_items', 'project_metadata', 'schema_migrations'],
  3: ['markers', 'operation_batches', 'operation_items', 'project_metadata', 'schema_migrations'],
}
const BASE_EXPECTED_COLUMNS = {
  schema_migrations: ['version', 'applied_at_ms'],
  operation_batches: ['batch_id', 'kind', 'created_at_ms', 'completed_at_ms'],
  operation_items: [
    'operation_id',
    'batch_id',
    'entity_id',
    'kind',
    'state',
    'source_path',
    'destination_path',
    'temporary_path',
    'expected_size',
    'expected_hash',
    'conflict_policy',
    'error_code',
    'updated_at_ms',
  ],
  project_metadata: ['singleton', 'project_id', 'created_at_ms'],
  markers: [
    'marker_id',
    'relative_path',
    'kind',
    'review_state',
    'favorite',
    'evidence_size',
    'evidence_modified_ns',
    'content_hash',
    'updated_at_ms',
  ],
}
const EXPECTED_COLUMNS = {
  1: BASE_EXPECTED_COLUMNS,
  2: BASE_EXPECTED_COLUMNS,
  3: {
    ...BASE_EXPECTED_COLUMNS,
    operation_batches: [
      ...BASE_EXPECTED_COLUMNS.operation_batches,
      'state',
      'requested_count',
      'completed_count',
      'failed_count',
      'skipped_count',
      'started_at_ms',
    ],
    operation_items: [...BASE_EXPECTED_COLUMNS.operation_items, 'result_code'],
  },
}
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
const ENTITY_UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const OPERATION_KINDS = new Set([
  'rename',
  'copy',
  'move',
  'trash',
  'set_review_state',
  'set_favorite',
])
const OPERATION_STATES = new Set([
  'prepared',
  'staged',
  'fs_applied',
  'verified',
  'meta_committed',
  'index_synced',
  'completed',
  'failed',
])
const CONFLICT_POLICIES = new Set(['skip', 'keep_both', 'replace'])
const BATCH_STATES = new Set(['running', 'completed'])
const SKIPPED_RESULT_CODES = new Set(['conflict_skipped', 'cancelled', 'session_stale'])
const RESULT_CODES = new Set([
  'renamed',
  'copied',
  'moved',
  'moved_to_trash',
  'conflict_skipped',
  'cancelled',
  'session_stale',
  'source_missing',
  'destination_occupied',
  'permission_denied',
  'verification_failed',
  'projection_stale',
  'backend_unavailable',
  'invalid_target',
  'completed',
  'failed',
  'legacy_failure',
  'interrupted_before_verified_copy',
  'recovered_before_verified_copy',
  'recovered_before_rename',
  'recovered_before_direct_rename',
  'recovered_before_verified_cross_volume_move',
  'rolled_back_cross_volume_move_copy',
  'recovered_cross_volume_move_before_placement',
  'recovered_before_replace',
  'restored_interrupted_source',
])
const TERMINAL_OPERATION_STATES = new Set(['completed', 'failed'])
const EXPECTED_CUSTOM_SCHEMA_OBJECTS = {
  1: [['index', 'operation_items_incomplete', 'operation_items']],
  2: [
    ['index', 'markers_favorite', 'markers'],
    ['index', 'markers_review_state', 'markers'],
    ['index', 'operation_items_incomplete', 'operation_items'],
  ],
  3: [
    ['index', 'markers_favorite', 'markers'],
    ['index', 'markers_review_state', 'markers'],
    ['index', 'operation_items_incomplete', 'operation_items'],
  ],
}
const EXPECTED_CUSTOM_INDEX_SQL = new Map([
  [
    'operation_items_incomplete',
    "CREATE INDEX operation_items_incomplete ON operation_items(state) WHERE state NOT IN ('completed', 'failed')",
  ],
  ['markers_review_state', 'CREATE INDEX markers_review_state ON markers(review_state)'],
  ['markers_favorite', 'CREATE INDEX markers_favorite ON markers(favorite)'],
])
const BASE_INDEX_SIGNATURES = {
  schema_migrations: [],
  operation_batches: ['pk:1:0:batch_id'],
  operation_items: ['c:0:1:state', 'pk:1:0:operation_id'],
}
const EXPECTED_INDEX_SIGNATURES = {
  1: BASE_INDEX_SIGNATURES,
  2: {
    ...BASE_INDEX_SIGNATURES,
    project_metadata: ['u:1:0:project_id'],
    markers: [
      'c:0:0:favorite',
      'c:0:0:review_state',
      'pk:1:0:marker_id',
      'u:1:0:relative_path',
    ],
  },
  3: {
    ...BASE_INDEX_SIGNATURES,
    project_metadata: ['u:1:0:project_id'],
    markers: [
      'c:0:0:favorite',
      'c:0:0:review_state',
      'pk:1:0:marker_id',
      'u:1:0:relative_path',
    ],
  },
}

export async function validatePortableMetadata(
  projectPath,
  { license, databaseSchemaVersion = MINIMUM_DATABASE_SCHEMA_VERSION } = {},
) {
  if (license !== APACHE_LICENSE) {
    throw new Error('portable validation requires Apache-2.0 license metadata')
  }
  if (
    !Number.isInteger(databaseSchemaVersion) ||
    databaseSchemaVersion < MINIMUM_DATABASE_SCHEMA_VERSION ||
    databaseSchemaVersion > LATEST_DATABASE_SCHEMA_VERSION
  ) {
    throw new Error(`unsupported portable database target: ${databaseSchemaVersion}`)
  }
  const root = resolve(String(projectPath))
  await requireDirectoryWithoutSymlink(root, 'project root')
  const viewer = basename(root) === '.viewer' ? root : join(root, '.viewer')
  await requireDirectoryWithoutSymlink(viewer, '.viewer directory')

  const entries = await readdir(viewer, { withFileTypes: true })
  const names = new Set(entries.map((entry) => entry.name))
  for (const required of REQUIRED_FILES) {
    if (!names.has(required)) throw new Error(`missing portable artifact: ${required}`)
  }

  const backups = []
  const transientFiles = []
  for (const entry of entries) {
    const path = join(viewer, entry.name)
    const metadata = await lstat(path)
    if (metadata.isSymbolicLink()) {
      throw new Error(`symbolic link is forbidden in portable metadata: ${entry.name}`)
    }
    if (!metadata.isFile()) {
      throw new Error(`forbidden portable artifact: ${entry.name}`)
    }
    if (REQUIRED_FILES.has(entry.name)) continue
    if (TRANSIENT_FILES.has(entry.name)) {
      transientFiles.push(entry.name)
      continue
    }
    const backup = /^metadata\.sqlite\.v([1-9][0-9]*)\.bak$/.exec(entry.name)
    if (backup !== null) {
      const version = Number(backup[1])
      if (version < databaseSchemaVersion) {
        backups.push({ name: entry.name, version })
        continue
      }
      throw new Error(`unsupported portable migration backup version: ${version}`)
    }
    throw new Error(`forbidden portable artifact: ${entry.name}`)
  }
  backups.sort((left, right) => left.version - right.version)
  transientFiles.sort()

  const manifest = await readManifest(join(viewer, 'project.json'))
  const databasePath = join(viewer, 'metadata.sqlite')
  await validateSQLiteSidecars(viewer, transientFiles)
  const database = inspectDatabase(databasePath, databaseSchemaVersion)
  if (database.projectId !== manifest.projectId) {
    throw new Error('portable manifest and database project identities differ')
  }
  if (database.createdAtMs !== manifest.createdAtMs) {
    throw new Error('portable manifest and database creation times differ')
  }
  for (const backup of backups) {
    const backupDatabase = inspectDatabase(join(viewer, backup.name), backup.version)
    if (
      backup.version >= 2 &&
      (backupDatabase.projectId !== manifest.projectId ||
        backupDatabase.createdAtMs !== manifest.createdAtMs)
    ) {
      throw new Error('portable migration backup project identity differs')
    }
  }

  return {
    projectId: manifest.projectId,
    manifestSchemaVersion: MANIFEST_SCHEMA_VERSION,
    schemaVersion: databaseSchemaVersion,
    markerCount: database.markerCount,
    operationCount: database.operationCount,
    transientFiles,
    backups: backups.map((backup) => backup.name),
  }
}

async function requireDirectoryWithoutSymlink(path, label) {
  const metadata = await lstat(path).catch(() => null)
  if (metadata === null || metadata.isSymbolicLink() || !metadata.isDirectory()) {
    throw new Error(`${label} must be a real directory`)
  }
}

async function readManifest(path) {
  const metadata = await lstat(path)
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    throw new Error('portable manifest must be a regular file')
  }
  if (metadata.size > 64 * 1024) throw new Error('portable manifest exceeds its size bound')
  let manifest
  try {
    manifest = JSON.parse(await readFile(path, 'utf8'))
  } catch {
    throw new Error('portable manifest is not valid JSON')
  }
  if (manifest === null || Array.isArray(manifest) || typeof manifest !== 'object') {
    throw new Error('portable manifest is not an object')
  }
  const keys = Object.keys(manifest).sort()
  const expectedKeys = ['createdAtMs', 'projectId', 'schemaVersion']
  if (!sameValues(keys, expectedKeys)) throw new Error('portable manifest fields are invalid')
  if (manifest.schemaVersion !== MANIFEST_SCHEMA_VERSION) {
    throw new Error(`unsupported manifest schema version: ${manifest.schemaVersion}`)
  }
  if (!UUID.test(manifest.projectId)) throw new Error('portable manifest project ID is invalid')
  if (!isNonNegativeInteger(manifest.createdAtMs)) {
    throw new Error('portable manifest creation time is invalid')
  }
  return manifest
}

async function validateSQLiteSidecars(viewer, names) {
  for (const name of names) {
    const path = join(viewer, name)
    const metadata = await lstat(path)
    if (metadata.size !== 0) {
      throw new Error(`non-empty SQLite sidecar must be recovered before validation: ${name}`)
    }
  }
}

function inspectDatabase(path, expectedVersion) {
  let database
  try {
    database = new DatabaseSync(path, { readOnly: true })
    database.exec('PRAGMA query_only = ON')
    const journalMode = database.prepare('PRAGMA journal_mode').get()?.journal_mode
    if (journalMode !== 'delete') {
      throw new Error('portable database must use SQLite DELETE journal mode')
    }
    const integrity = database.prepare('PRAGMA integrity_check').get()
    if (integrity?.integrity_check !== 'ok') throw new Error('integrity check failed')

    const versions = database
      .prepare('SELECT version FROM schema_migrations ORDER BY version')
      .all()
      .map((row) => Number(row.version))
    const expectedVersions = Array.from({ length: expectedVersion }, (_, index) => index + 1)
    if (!sameValues(versions, expectedVersions)) {
      throw new Error(
        `unsupported database schema version: ${versions.at(-1) ?? 0}; expected ${expectedVersion}`,
      )
    }
    validateSchema(database, expectedVersion)
    validateOperationJournal(database, expectedVersion)

    let projectId = null
    let createdAtMs = null
    let markerCount = 0
    if (expectedVersion >= 2) {
      const projects = database.prepare(
        'SELECT singleton, project_id, created_at_ms FROM project_metadata',
      ).all()
      if (
        projects.length !== 1 ||
        Number(projects[0].singleton) !== 1 ||
        !UUID.test(projects[0].project_id) ||
        !isNonNegativeInteger(projects[0].created_at_ms)
      ) {
        throw new Error('portable project metadata row is invalid')
      }
      projectId = projects[0].project_id
      createdAtMs = Number(projects[0].created_at_ms)
      markerCount = validateMarkers(database)
    }
    const operationCount = Number(
      database.prepare('SELECT COUNT(*) AS count FROM operation_items').get().count,
    )
    return { projectId, createdAtMs, markerCount, operationCount }
  } catch (error) {
    if (error instanceof Error && /unsupported database schema version/.test(error.message)) {
      throw error
    }
    if (error instanceof Error && /^(unexpected|invalid|unsafe|portable)/i.test(error.message)) {
      throw error
    }
    throw new Error(`portable database is invalid: ${error instanceof Error ? error.message : 'unknown error'}`)
  } finally {
    database?.close()
  }
}

function validateSchema(database, version) {
  const tables = database
    .prepare(
      "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .all()
    .map((row) => row.name)
  const expected = EXPECTED_TABLES[version]
  if (!sameValues(tables, expected)) {
    const unexpected = tables.find((table) => !expected.includes(table)) ?? 'missing required table'
    throw new Error(`unexpected portable database table: ${unexpected}`)
  }
  for (const table of expected) {
    const columns = database
      .prepare(`PRAGMA table_info(${table})`)
      .all()
      .map((row) => row.name)
    if (!sameValues(columns, EXPECTED_COLUMNS[version][table])) {
      throw new Error(`unexpected portable database columns: ${table}`)
    }
    validateIndexSignatures(database, table, EXPECTED_INDEX_SIGNATURES[version][table])
  }
  validateCustomSchemaObjects(database, version)
  validateForeignKeys(database, expected)
}

function validateIndexSignatures(database, table, expected) {
  const actual = database
    .prepare(`PRAGMA index_list(${table})`)
    .all()
    .map((row) => {
      const columns = database
        .prepare(`PRAGMA index_info(${quotedIdentifier(row.name)})`)
        .all()
        .map((column) => column.name)
        .join(',')
      return `${row.origin}:${Number(row.unique)}:${Number(row.partial)}:${columns}`
    })
    .sort()
  if (!sameValues(actual, expected)) {
    throw new Error(`unexpected portable database index: ${table}`)
  }
}

function validateCustomSchemaObjects(database, version) {
  const rows = database
    .prepare(
      "SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE type IN ('index', 'view', 'trigger') AND name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )
    .all()
  const actual = rows.map((row) => [row.type, row.name, row.tbl_name])
  if (JSON.stringify(actual) !== JSON.stringify(EXPECTED_CUSTOM_SCHEMA_OBJECTS[version])) {
    throw new Error('unexpected portable database schema object')
  }
  for (const row of rows) {
    const expected = EXPECTED_CUSTOM_INDEX_SQL.get(row.name)
    if (expected === undefined || normalizeSchemaSql(row.sql) !== normalizeSchemaSql(expected)) {
      throw new Error(`unexpected portable database index definition: ${row.name}`)
    }
  }
}

function validateForeignKeys(database, tables) {
  for (const table of tables) {
    const actual = database
      .prepare(`PRAGMA foreign_key_list(${table})`)
      .all()
      .map((row) => [
        row.table,
        row.from,
        row.to,
        row.on_update,
        row.on_delete,
        row.match,
      ])
    const expected =
      table === 'operation_items'
        ? [['operation_batches', 'batch_id', 'batch_id', 'NO ACTION', 'NO ACTION', 'NONE']]
        : []
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      throw new Error(`unexpected portable database foreign key: ${table}`)
    }
  }
}

function quotedIdentifier(value) {
  return `"${String(value).replaceAll('"', '""')}"`
}

function normalizeSchemaSql(value) {
  return String(value)
    .trim()
    .toLowerCase()
    .replace(/\s+/g, ' ')
    .replace(/\s*([(),])\s*/g, '$1')
}

function validateOperationJournal(database, version) {
  const batches = new Map()
  for (const row of database.prepare('SELECT * FROM operation_batches').iterate()) {
    if (
      !UUID.test(row.batch_id) ||
      !OPERATION_KINDS.has(row.kind) ||
      !isNonNegativeInteger(row.created_at_ms) ||
      (row.completed_at_ms !== null &&
        (!isNonNegativeInteger(row.completed_at_ms) || row.completed_at_ms < row.created_at_ms))
    ) {
      throw new Error('invalid portable operation batch')
    }
    if (version >= 3) validateV3Batch(row)
    batches.set(row.batch_id, row)
  }

  const counts = new Map(
    [...batches.keys()].map((batchId) => [
      batchId,
      { items: 0, completedResults: 0, failedResults: 0, skippedResults: 0, maxUpdatedAtMs: 0 },
    ]),
  )
  for (const row of database.prepare('SELECT * FROM operation_items').iterate()) {
    if (
      !UUID.test(row.operation_id) ||
      !UUID.test(row.batch_id) ||
      !ENTITY_UUID.test(row.entity_id)
    ) {
      throw new Error('invalid portable operation identity')
    }
    if (!OPERATION_KINDS.has(row.kind) || !OPERATION_STATES.has(row.state)) {
      throw new Error('invalid portable operation state')
    }
    for (const value of [row.source_path, row.destination_path, row.temporary_path]) {
      if (value !== null) requireSafeRelativePath(value)
    }
    if (row.expected_size !== null && !isNonNegativeInteger(row.expected_size)) {
      throw new Error('invalid portable operation size')
    }
    if (row.expected_hash !== null && !isByteArray(row.expected_hash, 32)) {
      throw new Error('invalid portable operation hash')
    }
    if (!CONFLICT_POLICIES.has(row.conflict_policy)) {
      throw new Error('invalid portable conflict policy')
    }
    if (
      row.error_code !== null &&
      (typeof row.error_code !== 'string' ||
        row.error_code.length > 128 ||
        /[\\/\n\r]/.test(row.error_code))
    ) {
      throw new Error('invalid portable operation error code')
    }
    if (!isNonNegativeInteger(row.updated_at_ms)) {
      throw new Error('invalid portable operation update time')
    }
    if (version >= 3) {
      validateV3Operation(row)
      const batch = batches.get(row.batch_id)
      if (batch === undefined) throw new Error('invalid portable operation batch foreign key')
      if (batch.kind !== row.kind) throw new Error('invalid portable operation batch kind')
      const count = counts.get(row.batch_id)
      count.items += 1
      count.maxUpdatedAtMs = Math.max(count.maxUpdatedAtMs, row.updated_at_ms)
      if (row.state === 'completed' && SKIPPED_RESULT_CODES.has(row.result_code)) {
        count.skippedResults += 1
      } else if (row.state === 'completed') {
        count.completedResults += 1
      }
      if (row.state === 'failed') count.failedResults += 1
    }
  }

  if (version >= 3) {
    const foreignKeyFailures = database.prepare('PRAGMA foreign_key_check').all()
    if (foreignKeyFailures.length > 0) {
      throw new Error('invalid portable operation foreign key')
    }
    for (const [batchId, batch] of batches) {
      validateV3BatchCounts(batch, counts.get(batchId))
    }
  }
}

function validateV3Batch(row) {
  const counters = [
    row.requested_count,
    row.completed_count,
    row.failed_count,
    row.skipped_count,
  ]
  if (
    !BATCH_STATES.has(row.state) ||
    !counters.every(isNonNegativeInteger) ||
    row.requested_count < 1 ||
    row.requested_count > 10_000
  ) {
    throw new Error('invalid portable operation batch counters')
  }
  if (
    !isNonNegativeInteger(row.started_at_ms) ||
    row.started_at_ms < row.created_at_ms ||
    (row.completed_at_ms !== null && row.completed_at_ms < row.started_at_ms)
  ) {
    throw new Error('invalid portable operation batch lifecycle')
  }
  if (
    (row.state === 'completed' && row.completed_at_ms === null) ||
    (row.state === 'running' && row.completed_at_ms !== null)
  ) {
    throw new Error('invalid portable operation batch lifecycle')
  }
}

function validateV3Operation(row) {
  const terminal = TERMINAL_OPERATION_STATES.has(row.state)
  if (terminal && row.result_code === null) {
    throw new Error('invalid terminal operation result')
  }
  if (!terminal && row.result_code !== null) {
    throw new Error('invalid non-terminal operation result')
  }
  if (row.result_code !== null && !RESULT_CODES.has(row.result_code)) {
    throw new Error('invalid portable operation result code')
  }
  if (row.error_code !== null && !RESULT_CODES.has(row.error_code)) {
    throw new Error('invalid portable operation result code')
  }
  if (row.state === 'failed' && row.error_code !== row.result_code) {
    throw new Error('invalid failed operation result code')
  }
  if (row.state === 'completed' && row.error_code !== null) {
    throw new Error('invalid completed operation error code')
  }
}

function validateV3BatchCounts(batch, count) {
  const accounted = batch.completed_count + batch.failed_count + batch.skipped_count
  if (
    batch.requested_count !== count.items ||
    batch.completed_count !== count.completedResults ||
    batch.failed_count !== count.failedResults ||
    batch.skipped_count !== count.skippedResults ||
    accounted > batch.requested_count ||
    (batch.state === 'completed' &&
      (accounted !== batch.requested_count || batch.completed_at_ms < count.maxUpdatedAtMs))
  ) {
    throw new Error('invalid portable operation batch counters')
  }
}

function validateMarkers(database) {
  let count = 0
  for (const row of database.prepare('SELECT * FROM markers').iterate()) {
    count += 1
    if (!UUID.test(row.marker_id)) throw new Error('invalid portable marker identity')
    requireSafeRelativePath(row.relative_path)
    if (![0, 1, 2, 3, 4].includes(Number(row.kind))) {
      throw new Error('invalid portable marker kind')
    }
    if (row.review_state !== null && ![0, 1, 2].includes(Number(row.review_state))) {
      throw new Error('invalid review state in portable marker')
    }
    if (![0, 1].includes(Number(row.favorite))) {
      throw new Error('invalid favorite value in portable marker')
    }
    if (row.evidence_size !== null && !isNonNegativeInteger(row.evidence_size)) {
      throw new Error('invalid portable marker size evidence')
    }
    if (
      row.evidence_modified_ns !== null &&
      (typeof row.evidence_modified_ns !== 'string' || !/^-?[0-9]{1,40}$/.test(row.evidence_modified_ns))
    ) {
      throw new Error('invalid portable marker modified-time evidence')
    }
    if (row.content_hash !== null && !isByteArray(row.content_hash, 32)) {
      throw new Error('invalid portable marker content hash')
    }
    if (!isNonNegativeInteger(row.updated_at_ms)) {
      throw new Error('invalid portable marker update time')
    }
  }
  return count
}

function requireSafeRelativePath(value) {
  if (
    typeof value !== 'string' ||
    value.length === 0 ||
    value.includes('\0') ||
    value.includes('\\') ||
    value.startsWith('/') ||
    value.endsWith('/') ||
    value.split('/').some((part) =>
      part === '' || part === '.' || part === '..' || part.toLowerCase() === '.viewer'
    )
  ) {
    throw new Error('unsafe relative path in portable database')
  }
}

function isNonNegativeInteger(value) {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function isByteArray(value, length) {
  return (value instanceof Uint8Array || Buffer.isBuffer(value)) && value.byteLength === length
}

function sameValues(left, right) {
  return left.length === right.length && left.every((value, index) => value === right[index])
}

async function validateRepositoryPolicy() {
  const packageJson = JSON.parse(
    await readFile(new URL('../package.json', import.meta.url), 'utf8'),
  )
  if (packageJson.license !== APACHE_LICENSE) {
    throw new Error('Viewer package metadata must declare Apache-2.0')
  }
  return {
    license: packageJson.license,
    manifestSchemaVersion: MANIFEST_SCHEMA_VERSION,
    databaseSchemaVersion: LATEST_DATABASE_SCHEMA_VERSION,
  }
}

async function main() {
  const argument = process.argv[2]
  const policy = await validateRepositoryPolicy()
  if (argument === '--policy') {
    process.stdout.write(`${JSON.stringify(policy)}\n`)
    return
  }
  if (!argument) throw new Error('usage: validate-m2-portable-metadata.mjs <project-or-.viewer>')
  const result = await validatePortableMetadata(argument, {
    license: policy.license,
    databaseSchemaVersion: policy.databaseSchemaVersion,
  })
  process.stdout.write(`${JSON.stringify(result)}\n`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : 'portable validation failed'}\n`)
    process.exitCode = 1
  })
}
