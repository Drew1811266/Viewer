import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'

const REQUIRED_FIELDS = [
  'id',
  'reason',
  'chain',
  'owner',
  'created',
  'reviewAfter',
  'removeWhen',
]

function parseDate(value, field, id) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    throw new Error(`${id}: ${field} must be a valid date in YYYY-MM-DD format`)
  }

  const parsed = new Date(`${value}T00:00:00Z`)
  if (Number.isNaN(parsed.getTime()) || parsed.toISOString().slice(0, 10) !== value) {
    throw new Error(`${id}: ${field} must be a valid date in YYYY-MM-DD format`)
  }

  return parsed
}

export function validateExceptions(exceptions, today = new Date()) {
  if (!Array.isArray(exceptions)) {
    throw new Error('dependency exceptions must be an array')
  }

  const seen = new Set()
  const todayUtc = new Date(Date.UTC(
    today.getUTCFullYear(),
    today.getUTCMonth(),
    today.getUTCDate(),
  ))

  for (const entry of exceptions) {
    const label = entry?.id || 'dependency exception'
    for (const field of REQUIRED_FIELDS) {
      if (typeof entry?.[field] !== 'string' || entry[field].trim() === '') {
        throw new Error(`${label}: ${field} is required`)
      }
    }

    if (seen.has(entry.id)) {
      throw new Error(`duplicate dependency exception ID: ${entry.id}`)
    }
    seen.add(entry.id)

    parseDate(entry.created, 'created', entry.id)
    const reviewAfter = parseDate(entry.reviewAfter, 'reviewAfter', entry.id)
    if (reviewAfter < todayUtc) {
      throw new Error(`${entry.id}: expired on ${entry.reviewAfter}`)
    }
  }
}

function denyAdvisoryIds(denyToml) {
  const header = denyToml.match(/^\[advisories\]\s*$/m)
  if (!header) {
    return []
  }

  const remainder = denyToml.slice(header.index + header[0].length)
  const nextSection = remainder.search(/^\[[^\]]+\]\s*$/m)
  const advisories = nextSection === -1
    ? remainder
    : remainder.slice(0, nextSection)
  const ignore = advisories.match(/(?:^|\n)\s*ignore\s*=\s*\[([\s\S]*?)\]/)?.[1] ?? ''
  return [...ignore.matchAll(/\bid\s*=\s*"(RUSTSEC-[^"]+)"/g)]
    .map((match) => match[1])
}

export function validateDenyRegistrations(exceptions, denyToml) {
  const registeredIds = new Set(exceptions.map((entry) => entry.id))
  const denyIds = new Set(denyAdvisoryIds(denyToml))

  for (const id of denyIds) {
    if (!registeredIds.has(id)) {
      throw new Error(`${id} is ignored in deny.toml but not registered`)
    }
  }

  for (const id of registeredIds) {
    if (!denyIds.has(id)) {
      throw new Error(`${id} is registered but not present in deny.toml ignores`)
    }
  }
}

export async function validateDependencyFiles({ exceptionsPath, denyPath, today = new Date() }) {
  const [exceptionsJson, denyToml] = await Promise.all([
    readFile(exceptionsPath, 'utf8'),
    readFile(denyPath, 'utf8'),
  ])
  const exceptions = JSON.parse(exceptionsJson)

  validateExceptions(exceptions, today)
  validateDenyRegistrations(exceptions, denyToml)
  return exceptions.length
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  validateDependencyFiles({
    exceptionsPath: 'docs/quality/dependency-exceptions.json',
    denyPath: 'deny.toml',
  }).then(
    (count) => console.log(`Dependency exceptions valid: ${count}`),
    (error) => {
      console.error(error instanceof Error ? error.message : error)
      process.exitCode = 1
    },
  )
}
