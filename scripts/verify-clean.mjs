#!/usr/bin/env node

import { execFileSync, spawnSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'

const NUL = 0
const SPACE = 0x20
const RENAME = 0x52
const COPY = 0x43
const statusBytes = new Set(Buffer.from(' MADRCUT?!'))
const utf8 = new TextDecoder('utf-8', { fatal: true })

function fieldAt(buffer, offset) {
  const end = buffer.indexOf(NUL, offset)
  if (end === -1) return { value: buffer.subarray(offset), terminated: false, offset: buffer.length }
  return { value: buffer.subarray(offset, end), terminated: true, offset: end + 1 }
}

function hasStatusHeader(field) {
  return field.length >= 3 && field[2] === SPACE && statusBytes.has(field[0]) && statusBytes.has(field[1])
}

function parsePorcelain(buffer) {
  const records = []
  let offset = 0

  while (offset < buffer.length) {
    const first = fieldAt(buffer, offset)
    offset = first.offset
    const renamedOrCopied =
      hasStatusHeader(first.value) && (first.value[0] === RENAME || first.value[1] === RENAME || first.value[0] === COPY || first.value[1] === COPY)

    if (!renamedOrCopied) {
      records.push({ fields: [first.value], valid: first.terminated && hasStatusHeader(first.value) })
      continue
    }

    if (!first.terminated || offset >= buffer.length) {
      records.push({ fields: [first.value], valid: false })
      continue
    }

    const second = fieldAt(buffer, offset)
    offset = second.offset
    records.push({ fields: [first.value, second.value], valid: second.terminated })
  }

  return records
}

const keyFor = (record) =>
  `${record.valid ? 'valid' : 'malformed'}:${record.fields
    .map((field) => `${field.length}:${field.toString('hex')}`)
    .join('|')}`

function renderField(field) {
  try {
    return utf8.decode(field).replace(/[\\\u0000-\u001f\u007f-\u009f]/g, (character) => {
      const escapes = { '\\': '\\\\', '\b': '\\b', '\t': '\\t', '\n': '\\n', '\v': '\\v', '\f': '\\f', '\r': '\\r' }
      return escapes[character] ?? `\\x${character.codePointAt(0).toString(16).padStart(2, '0')}`
    })
  } catch {
    return `raw-hex:${field.toString('hex')}`
  }
}

const renderRecord = (record) => {
  const rendered = record.fields.map(renderField).join(' \\0 ')
  return record.valid ? rendered : `malformed porcelain record: ${rendered}`
}

function countsFor(buffer) {
  const counts = new Map()
  for (const record of parsePorcelain(buffer)) {
    const key = keyFor(record)
    const current = counts.get(key)
    if (current) current.count += 1
    else counts.set(key, { count: 1, record })
  }
  return counts
}

export function comparePorcelain(before, after) {
  const baseline = countsFor(before)
  const current = countsFor(after)
  const changes = []
  for (const [key, { count, record }] of baseline) {
    const difference = count - (current.get(key)?.count ?? 0)
    for (let index = 0; index < difference; index += 1) changes.push(`removed: ${renderRecord(record)}`)
  }
  for (const [key, { count, record }] of current) {
    const difference = count - (baseline.get(key)?.count ?? 0)
    for (let index = 0; index < difference; index += 1) changes.push(`added: ${renderRecord(record)}`)
  }
  return changes.sort()
}

function porcelain() {
  return execFileSync('git', ['status', '--porcelain=v1', '-z'], { encoding: 'buffer' })
}

export function main() {
  const before = porcelain()
  const result = spawnSync('pnpm', ['verify'], { stdio: 'inherit' })
  const changes = comparePorcelain(before, porcelain())
  if (changes.length > 0) {
    process.stderr.write(`verification changed worktree entries:\n${changes.join('\n')}\n`)
    return 1
  }
  return result.status ?? 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) process.exitCode = main()
