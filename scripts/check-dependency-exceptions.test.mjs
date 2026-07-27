import assert from 'node:assert/strict'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  validateDependencyFiles,
  validateDenyRegistrations,
  validateExceptions,
} from './check-dependency-exceptions.mjs'

test('rejects expired and ownerless exceptions', () => {
  assert.throws(
    () => validateExceptions([{
      id: 'RUSTSEC-1',
      reason: 'transitive',
      chain: 'tauri > unic',
      owner: '',
      created: '2026-07-23',
      reviewAfter: '2026-07-22',
      removeWhen: 'upstream removes dependency',
    }], new Date('2026-07-23T00:00:00Z')),
    /owner|expired/,
  )
})

const validException = {
  id: 'RUSTSEC-1',
  reason: 'transitive',
  chain: 'tauri > unic',
  owner: 'Viewer maintainers',
  created: '2026-07-23',
  reviewAfter: '2026-10-23',
  removeWhen: 'upstream removes dependency',
}

test('rejects an exception when any required field is missing', () => {
  for (const field of Object.keys(validException)) {
    const incomplete = { ...validException }
    delete incomplete[field]
    assert.throws(
      () => validateExceptions([incomplete], new Date('2026-07-23T00:00:00Z')),
      new RegExp(field),
    )
  }
})

test('rejects duplicate exception IDs', () => {
  assert.throws(
    () => validateExceptions(
      [validException, { ...validException }],
      new Date('2026-07-23T00:00:00Z'),
    ),
    /duplicate.*RUSTSEC-1/i,
  )
})

test('rejects invalid calendar dates', () => {
  assert.throws(
    () => validateExceptions(
      [{ ...validException, reviewAfter: '2026-02-30' }],
      new Date('2026-07-23T00:00:00Z'),
    ),
    /reviewAfter.*valid date/i,
  )
})

test('rejects a review date before today', () => {
  assert.throws(
    () => validateExceptions(
      [{ ...validException, reviewAfter: '2026-07-22' }],
      new Date('2026-07-23T23:59:59Z'),
    ),
    /expired/i,
  )
})

test('requires every deny advisory ignore to be registered', () => {
  assert.throws(
    () => validateDenyRegistrations(
      [{ ...validException }],
      `[advisories]
ignore = [
  { id = "RUSTSEC-1" },
  { id = "RUSTSEC-2" },
]
`,
    ),
    /RUSTSEC-2.*not registered/i,
  )
})

test('requires every registered advisory to exist in deny.toml', () => {
  assert.throws(
    () => validateDenyRegistrations(
      [{ ...validException }, { ...validException, id: 'RUSTSEC-2' }],
      `[advisories]
ignore = [
  { id = "RUSTSEC-1" },
]
`,
    ),
    /RUSTSEC-2.*not present/i,
  )
})

test('accepts matching deny advisory ignores and registrations', () => {
  assert.doesNotThrow(() => validateDenyRegistrations(
    [{ ...validException }],
    `[advisories]
ignore = [
  { id = "RUSTSEC-1" },
]

[licenses]
allow = ["RUSTSEC-UNRELATED"]
`,
  ))
})

test('does not count commented advisory IDs as active deny ignores', () => {
  assert.throws(
    () => validateDenyRegistrations(
      [{ ...validException }],
      `[advisories]
ignore = [
  # { id = "RUSTSEC-1", reason = "disabled # exception" },
]
`,
    ),
    /RUSTSEC-1.*not present/i,
  )
})

test('reads dependency files and rejects an unregistered deny ignore', async (t) => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-dependency-exceptions-'))
  t.after(() => rm(directory, { recursive: true, force: true }))

  const exceptionsPath = join(directory, 'exceptions.json')
  const denyPath = join(directory, 'deny.toml')
  await writeFile(exceptionsPath, `${JSON.stringify([validException])}\n`)
  await writeFile(denyPath, `[advisories]
ignore = [
  { id = "RUSTSEC-1" },
  { id = "RUSTSEC-2" },
]
`)

  await assert.rejects(
    validateDependencyFiles({
      exceptionsPath,
      denyPath,
      today: new Date('2026-07-23T00:00:00Z'),
    }),
    /RUSTSEC-2.*not registered/i,
  )
})
