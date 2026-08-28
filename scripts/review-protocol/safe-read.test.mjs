import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import fs from 'node:fs/promises'
import { syncBuiltinESMExports } from 'node:module'
import { link, mkdir, mkdtemp, open, readFile, readdir, rename, rm, symlink, truncate, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { blake3Hex, createBlake3Hasher } from './blake3.mjs'
import { openSafeProject } from './safe-read.mjs'

async function project(t) {
  const root = await mkdtemp(path.join(tmpdir(), 'viewer-safe-reader-'))
  t.after(() => rm(root, { recursive: true, force: true }))
  const repository = path.join(root, '.viewer/reviews')
  await mkdir(path.join(repository, 'states'), { recursive: true })
  return { root, repository, reader: await openSafeProject(root) }
}

test('reads bounded repository JSON without creating or writing project files', async t => {
  const { root, repository, reader } = await project(t)
  await writeFile(path.join(repository, 'index.json'), '{"ok":true}')
  const before = await readdir(root, { recursive: true })
  const result = await reader.readRepositoryJson('index.json', 11)
  assert.deepEqual(result.data, { ok: true })
  assert.equal(result.bytes.toString(), '{"ok":true}')
  await assert.rejects(reader.readRepositoryJson('index.json', 10), /size limit/)
  assert.deepEqual(await readdir(root, { recursive: true }), before)
  await writeFile(path.join(repository, 'states/bad.json'), '{bad')
  await assert.rejects(reader.readRepositoryJson('states/bad.json', 64), /valid JSON/)
})

test('rejects traversal, metadata aliases, symlinks, hardlinks and nonregular files', async t => {
  const { root, repository, reader } = await project(t)
  await writeFile(path.join(root, 'source.bin'), 'abc')
  await writeFile(path.join(repository, 'index.json'), '{}')
  for (const location of ['../source.bin', '/etc/passwd', 'states//a', './index.json', 'states/../index.json', 'states\\a', 'C:a', 'states/\0a', '.Viewer/index.json']) {
    await assert.rejects(reader.readRepositoryJson(location, 64), /path|location/)
    assert.notEqual((await reader.checkSource(location, { sizeBytes: 3, blake3: blake3Hex(Buffer.from('abc')) })).status, 'match')
  }
  for (const location of ['.viewer/reviews/index.json', '.VIEWER/reviews/index.json']) {
    assert.equal((await reader.checkSource(location, { sizeBytes: 2, blake3: blake3Hex(Buffer.from('{}')) })).status, 'unverified')
  }
  await symlink(path.join(repository, 'index.json'), path.join(repository, 'link.json'))
  await symlink(repository, path.join(repository, 'linked'))
  await link(path.join(repository, 'index.json'), path.join(repository, 'hard.json'))
  execFileSync('mkfifo', [path.join(repository, 'pipe')])
  for (const location of ['link.json', 'linked/index.json', 'hard.json', 'states', 'pipe']) {
    await assert.rejects(reader.readRepositoryJson(location, 64), /symbolic|regular|link/)
  }
})

test('revalidates the root and parent identity on every read', async t => {
  const { root, repository, reader } = await project(t)
  await writeFile(path.join(repository, 'states/a.json'), '{}')
  await rename(path.join(repository, 'states'), path.join(repository, 'old-states'))
  await symlink(path.join(repository, 'old-states'), path.join(repository, 'states'))
  await assert.rejects(reader.readRepositoryJson('states/a.json', 64), /symbolic/)
  await rename(root, `${root}-moved`)
  t.after(() => rm(`${root}-moved`, { recursive: true, force: true }))
  await mkdir(root)
  await assert.rejects(reader.readRepositoryJson('index.json', 64), /changed|identity/)
})

test('source checks distinguish content, missing, unverified and directory failures', async t => {
  const { root, reader } = await project(t)
  const fingerprint = { sizeBytes: 3, blake3: blake3Hex(Buffer.from('abc')), modifiedNs: '0' }
  await writeFile(path.join(root, 'source'), 'abc')
  assert.equal((await reader.checkSource('source', fingerprint)).status, 'match', 'mtime alone is not a content version')
  await writeFile(path.join(root, 'source'), 'xyz')
  assert.equal((await reader.checkSource('source', fingerprint)).status, 'changed')
  assert.equal((await reader.checkSource('missing', fingerprint)).status, 'missing')
  assert.equal((await reader.checkSource('source', { ...fingerprint, blake3: null })).status, 'unverified')
  assert.equal((await reader.checkSource('.viewer', fingerprint)).status, 'unverified')
  await mkdir(path.join(root, 'directory'))
  assert.equal((await reader.checkSource('directory', fingerprint)).status, 'unreadable')
})

test('rejects repository and source replacement during reads, even with identical bytes', async t => {
  const { root, repository, reader } = await project(t)
  const probe = await open(path.join(root, 'probe'), 'w+')
  const prototype = Object.getPrototypeOf(probe)
  const original = prototype.read
  await probe.close()
  for (const source of [false, true]) {
    const destination = source ? path.join(root, 'source') : path.join(repository, 'states/a.json')
    await writeFile(destination, '{}')
    await writeFile(`${destination}.replacement`, '{}')
    let changed = false
    const mock = t.mock.method(prototype, 'read', async function (...args) {
      const result = await original.apply(this, args)
      if (!changed) { changed = true; await rename(`${destination}.replacement`, destination) }
      return result
    })
    if (source) assert.equal((await reader.checkSource('source', { sizeBytes: 2, blake3: blake3Hex(Buffer.from('{}')) })).status, 'unverified')
    else await assert.rejects(reader.readRepositoryJson('states/a.json', 64), /changed|identity/)
    mock.mock.restore()
    assert.ok(changed)
  }
})

test('atomic index publication retains one complete opened version while immutable objects reject replacement', async t => {
  const { repository, reader } = await project(t)
  const file = path.join(repository, 'index.json')
  await writeFile(file, '{"version":1}')
  await writeFile(`${file}.next`, '{"version":2}')
  const probe = await open(file, 'r')
  const prototype = Object.getPrototypeOf(probe)
  const original = prototype.read
  await probe.close()
  let replaced = false
  const hook = t.mock.method(prototype, 'read', async function (...args) {
    const result = await original.apply(this, args)
    if (!replaced) { replaced = true; await rename(`${file}.next`, file) }
    return result
  })
  try { assert.deepEqual((await reader.readRepositoryJson('index.json', 64)).data, { version: 1 }) }
  finally { hook.mock.restore() }
  assert.deepEqual((await reader.readRepositoryJson('index.json', 64)).data, { version: 2 })
})

test('directory substitution between checks cannot publish bytes from outside the project', async t => {
  // Genuine filesystem calls; the hooks only schedule directory moves between
  // individual async checks. Include a parent above the selected project root.
  for (const aboveRoot of [false, true]) {
    const owned = await mkdtemp(path.join(tmpdir(), 'viewer-directory-race-'))
    const original = { ...fs }
    const projectRoot = path.join(owned, 'parent/project')
    const outside = path.join(owned, 'outside')
    const repository = path.join(projectRoot, '.viewer/reviews')
    await mkdir(repository, { recursive: true })
    const redirected = aboveRoot ? path.join(outside, 'project/.viewer/reviews') : outside
    await mkdir(redirected, { recursive: true })
    await writeFile(path.join(repository, 'index.json'), '{"origin":"inside"}')
    await writeFile(path.join(redirected, 'index.json'), '{"origin":"outside"}')
    const canonical = await original.realpath(projectRoot)
    const target = aboveRoot ? path.dirname(canonical) : path.join(canonical, '.viewer/reviews')
    const parked = `${target}-parked`
    const leaf = path.join(canonical, '.viewer/reviews/index.json')
    const reader = await openSafeProject(projectRoot)
    let moved = false
    const restore = async () => {
      if (moved) {
        await original.unlink(target)
        await original.rename(parked, target)
        moved = false
      }
    }
    let directories = 0
    let leaves = 0
    const statHook = t.mock.method(fs, 'lstat', async (file, ...args) => {
      const result = await original.lstat(file, ...args)
      if (file === target && [1, 3].includes(++directories)) {
        await original.rename(target, parked)
        await original.symlink(outside, target)
        moved = true
      }
      if (file === leaf && ++leaves === 2) await restore()
      return result
    })
    const openHook = t.mock.method(fs, 'open', async (file, ...args) => {
      const result = await original.open(file, ...args)
      if (file === leaf) await restore()
      return result
    })
    syncBuiltinESMExports()
    try {
      const result = await reader.readRepositoryJson('index.json', 64).catch(error => error)
      if (result instanceof Error) assert.match(result.message, /changed|symbolic|identity/)
      else assert.deepEqual(result.data, { origin: 'inside' })
      assert.ok(directories > 0)
    } finally {
      statHook.mock.restore()
      openHook.mock.restore()
      syncBuiltinESMExports()
      await restore()
      await original.rm(owned, { recursive: true, force: true })
    }
  }
})

test('verifies PNG digest, byte count and bounded pixel dimensions', async t => {
  const { repository, reader } = await project(t)
  const png = await readFile(new URL('../../tests/fixtures/images/alpha.png', import.meta.url))
  await mkdir(path.join(repository, 'evidence'))
  const expected = { blake3: blake3Hex(png), sizeBytes: png.length, width: png.readUInt32BE(16), height: png.readUInt32BE(20) }
  const location = `evidence/${expected.blake3}.png`
  await writeFile(path.join(repository, location), png)
  assert.deepEqual((await reader.readRepositoryPng(location, expected)).bytes, png)
  await assert.rejects(reader.readRepositoryPng(location, { ...expected, width: expected.width + 1 }), /dimensions/)
  await assert.rejects(reader.readRepositoryPng(location, { ...expected, blake3: 'f'.repeat(64) }), /digest/)
  const bomb = Buffer.from(png)
  bomb.writeUInt32BE(0xffffffff, 16)
  await writeFile(path.join(repository, 'evidence/bomb.png'), bomb)
  await assert.rejects(reader.readRepositoryPng('evidence/bomb.png', { ...expected, blake3: blake3Hex(bomb), width: 0xffffffff }), /pixel|dimensions/)
  await truncate(path.join(repository, location), 64 * 1024 * 1024 + 1)
  await assert.rejects(reader.readRepositoryPng(location, expected), /size limit/)
})

test('large source hashing reuses one bounded IO buffer and never concatenates source bytes', async t => {
  const { root, reader } = await project(t)
  const size = 32 * 1024 * 1024
  const file = await open(path.join(root, 'large'), 'w+')
  await file.truncate(size)
  const prototype = Object.getPrototypeOf(file)
  const original = prototype.read
  await file.close()
  const hash = createBlake3Hasher()
  const zero = Buffer.alloc(65536)
  for (let i = 0; i < size; i += zero.length) hash.update(zero)
  const buffers = new Set()
  let reads = 0
  const readHook = t.mock.method(prototype, 'read', async function (buffer, ...args) {
    assert.ok(buffer.byteLength <= 65536)
    buffers.add(buffer.buffer)
    reads += 1
    return original.call(this, buffer, ...args)
  })
  const concatHook = t.mock.method(Buffer, 'concat', () => { throw new Error('unbounded source concatenation') })
  try {
    assert.equal((await reader.checkSource('large', { sizeBytes: size, blake3: hash.digestHex() })).status, 'match')
  } finally {
    // fs.rm's internal directory walker also uses Buffer.concat; its after hook
    // must run with real fs internals, not our source-read-only instrumentation.
    concatHook.mock.restore()
    readHook.mock.restore()
  }
  assert.ok(reads >= 512)
  assert.equal(buffers.size, 1)
})
