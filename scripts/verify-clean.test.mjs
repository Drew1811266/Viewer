import assert from 'node:assert/strict'
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
