#!/usr/bin/env node

import { readdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const fixtureDirectory = fileURLToPath(new URL('../tests/fixtures/m1-ipc/', import.meta.url))
const fixtureFiles = readdirSync(fixtureDirectory)
  .filter((name) => name.endsWith('.json'))
  .sort()

if (fixtureFiles.length === 0) throw new Error('M1 IPC fixture directory is empty')
for (const name of fixtureFiles) {
  const payload = JSON.parse(readFileSync(`${fixtureDirectory}/${name}`, 'utf8'))
  validateIpcPayload(payload, name)
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  console.log(`M1 IPC fixtures passed: ${fixtureFiles.length} payloads`)
}

export function validateIpcPayload(payload, source) {
  visit(payload, source, [])
}

function visit(value, source, keys) {
  if (typeof value === 'string') {
    validateString(value, source, keys)
    return
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => visit(item, source, [...keys, String(index)]))
    return
  }
  if (typeof value !== 'object' || value === null) return
  for (const [key, child] of Object.entries(value)) {
    if (key === 'kind' && typeof child === 'string') {
      const allowed = ['directory', 'jpeg', 'png', 'markdown', 'text']
      if (!allowed.includes(child)) fail(source, [...keys, key], `unsupported kind ${child}`)
    }
    visit(child, source, [...keys, key])
  }
}

function validateString(value, source, keys) {
  const normalized = value.replaceAll('\\', '/')
  if (
    normalized.startsWith('/') ||
    /^[a-z]:\//i.test(normalized) ||
    normalized.startsWith('file:')
  ) {
    fail(source, keys, 'absolute path disclosure')
  }
  if (normalized.split('/').some((segment) => segment.toLowerCase() === '.viewer')) {
    fail(source, keys, 'reserved .viewer disclosure')
  }
  if (/library\/caches|\/sessions\/|viewer-cache/i.test(normalized)) {
    fail(source, keys, 'session cache disclosure')
  }
}

function fail(source, keys, message) {
  throw new Error(`${source}:${keys.join('.')}: ${message}`)
}
