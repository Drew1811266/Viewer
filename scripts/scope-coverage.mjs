export const VALID_STAGES = new Set([
  'M1',
  'M2',
  'M3',
  'Continuous',
  'Future',
  'NotApplicable',
])

const idPattern = /\bREQ-[A-Z0-9]+(?:-[A-Z0-9]+)*\b/g

const expectedM3Ids = [
  'REQ-FLOW-BATCH-RENAME',
  'REQ-FLOW-COMPARE',
  'REQ-FLOW-DRAG-DROP',
  'REQ-FLOW-EXTERNAL-CHANGES',
  'REQ-FLOW-LIFECYCLE',
  'REQ-FLOW-ORGANIZE',
  'REQ-FLOW-READONLY-ERRORS',
  'REQ-FLOW-SHORTCUTS',
  'REQ-FLOW-UNDO',
  'REQ-IA-TASK-BAR',
  'REQ-TECH-FILE-CONSISTENCY',
  'REQ-TECH-PATH-SECURITY',
  'REQ-TECH-WATCHER',
]

export function validateScopeCoverage(productSpec, matrix) {
  const specIds = [...productSpec.matchAll(idPattern)].map(([id]) => id)
  const matrixRows = matrix
    .split(/\r?\n/)
    .filter((line) => /^\|\s*REQ-[A-Z0-9-]+\s*\|/.test(line))
    .map((line) => line.split('|').slice(1, -1).map((cell) => cell.trim()))

  assertNoDuplicates('product spec', specIds)
  assertNoDuplicates('scope matrix', matrixRows.map(([id]) => id))
  assertExactCoverage(specIds, matrixRows.map(([id]) => id))

  for (const row of matrixRows) {
    if (row.length !== 7) {
      throw new Error(`matrix row ${row[0]} must contain exactly 7 columns`)
    }
    const [id, summary, owner, stage, tests, acceptance, gate] = row
    for (const [name, value] of Object.entries({
      summary,
      owner,
      stage,
      tests,
      acceptance,
      gate,
    })) {
      if (!value || value === '—') throw new Error(`matrix row ${id} has an empty ${name} column`)
    }
    for (const token of stage.split('/')) {
      if (!VALID_STAGES.has(token)) throw new Error(`invalid stage token ${token} in ${id}`)
    }
  }

  if (/\|\s*M4\s*\|/i.test(matrix)) {
    throw new Error('invalid stage token M4 in active scope matrix')
  }

  const actualM3Ids = matrixRows
    .filter(([, , , stage]) => stage.split('/').includes('M3'))
    .map(([id]) => id)
    .sort()
  assertFrozenM3(actualM3Ids)

  if (specIds.length === 0) throw new Error('product spec contains no stable requirement IDs')
  return { requirementCount: specIds.length, m3Count: actualM3Ids.length }
}

function assertNoDuplicates(source, ids) {
  const duplicates = findDuplicates(ids)
  if (duplicates.length > 0) {
    throw new Error(`duplicate requirement IDs in ${source}: ${duplicates.join(', ')}`)
  }
}

function assertExactCoverage(specIds, matrixIds) {
  const specSet = new Set(specIds)
  const matrixSet = new Set(matrixIds)
  const missing = specIds.filter((id) => !matrixSet.has(id))
  const unknown = matrixIds.filter((id) => !specSet.has(id))
  if (missing.length > 0 || unknown.length > 0) {
    throw new Error(
      `scope coverage mismatch; missing=[${missing.join(', ')}], unknown=[${unknown.join(', ')}]`,
    )
  }
}

function assertFrozenM3(actualM3Ids) {
  if (JSON.stringify(actualM3Ids) !== JSON.stringify(expectedM3Ids)) {
    throw new Error(
      `M3 scope changed without review; expected=[${expectedM3Ids.join(', ')}], actual=[${actualM3Ids.join(', ')}]`,
    )
  }
}

function findDuplicates(values) {
  const seen = new Set()
  const repeated = new Set()
  for (const value of values) {
    if (seen.has(value)) repeated.add(value)
    seen.add(value)
  }
  return [...repeated].sort()
}
