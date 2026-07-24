import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import test from 'node:test'

import { comparePorcelain } from './verify-clean.mjs'

test('pre-existing user changes are preserved without failing', () => {
  const before = Buffer.from('?? user-file\0')
  const after = Buffer.from('?? user-file\0')
  assert.deepEqual(comparePorcelain(before, after), [])
})

test('new verification artifacts are reported', () => {
  const before = Buffer.from('?? user-file\0')
  const after = Buffer.from('?? user-file\0?? tests/.DS_Store\0')
  assert.deepEqual(comparePorcelain(before, after), ['added: ?? tests/.DS_Store'])
})

test('removed worktree entries are reported', () => {
  const before = Buffer.from('?? user-file\0?? generated-cache\0')
  const after = Buffer.from('?? user-file\0')
  assert.deepEqual(comparePorcelain(before, after), ['removed: ?? generated-cache'])
})

test('rename records keep the second path as one logical record', () => {
  const before = Buffer.alloc(0)
  const after = Buffer.from('R  destination\0source\0')
  assert.deepEqual(comparePorcelain(before, after), ['added: R  destination \\0 source'])
})

test('rename source fields do not collide with independent status records', () => {
  const before = Buffer.from([
    0x52, 0x20, 0x20, 0x64, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x61, 0x74, 0x69, 0x6f, 0x6e, 0x00,
    0x3f, 0x3f, 0x20, 0x73, 0x6f, 0x75, 0x72, 0x63, 0x65, 0x00,
    0x3f, 0x3f, 0x20, 0x73, 0x6f, 0x75, 0x72, 0x63, 0x65, 0x00,
  ])
  const after = Buffer.from([
    0x52, 0x20, 0x20, 0x64, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x61, 0x74, 0x69, 0x6f, 0x6e, 0x00,
    0x3f, 0x3f, 0x20, 0x73, 0x6f, 0x75, 0x72, 0x63, 0x65, 0x00,
  ])
  assert.deepEqual(comparePorcelain(before, after), ['removed: ?? source'])
})

test('copy records keep the second path as one logical record', () => {
  const before = Buffer.alloc(0)
  const after = Buffer.from('C  destination\0source\0')
  assert.deepEqual(comparePorcelain(before, after), ['added: C  destination \\0 source'])
})

test('control characters in paths are escaped in drift reports', () => {
  const before = Buffer.alloc(0)
  const after = Buffer.from('?? line\nname\0')
  assert.deepEqual(comparePorcelain(before, after), ['added: ?? line\\nname'])
})

test('distinct invalid UTF-8 records remain distinct', () => {
  const before = Buffer.alloc(0)
  const after = Buffer.from([0x3f, 0x3f, 0x20, 0xff, 0x00, 0x3f, 0x3f, 0x20, 0xfe, 0x00])
  assert.deepEqual(comparePorcelain(before, after), [
    'added: raw-hex:3f3f20fe',
    'added: raw-hex:3f3f20ff',
  ])
})

test('duplicate logical records retain multiplicity', () => {
  const before = Buffer.from('?? duplicate\0')
  const after = Buffer.from('?? duplicate\0?? duplicate\0')
  assert.deepEqual(comparePorcelain(before, after), ['added: ?? duplicate'])
})

test('truncated rename records do not merge with valid records', () => {
  const before = Buffer.from('?? safe\0')
  const after = Buffer.from('R  destination\0')
  assert.deepEqual(comparePorcelain(before, after), [
    'added: malformed porcelain record: R  destination',
    'removed: ?? safe',
  ])
})

test('importing from eval without argv[1] does not execute the wrapper', () => {
  assert.doesNotThrow(() =>
    execFileSync(process.execPath, ['--input-type=module', '--eval', "import './scripts/verify-clean.mjs'"], {
      stdio: 'pipe',
    }),
  )
})
