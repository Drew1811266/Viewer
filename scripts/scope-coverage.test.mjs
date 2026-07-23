import assert from 'node:assert/strict'
import test from 'node:test'

import { VALID_STAGES, validateScopeCoverage } from './scope-coverage.mjs'

const frozenM3Ids = [
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
const spec = [
  'REQ-ONE',
  'REQ-TWO',
  ...frozenM3Ids,
].map((id) => `## Requirement [${id}]`).join('\n\n')
const matrix = (oneStage, twoStage) => `| Requirement ID | Summary | Owner | Stage | Tests | Acceptance | Gate |
| --- | --- | --- | --- | --- | --- | --- |
| REQ-ONE | One | Owner | ${oneStage} | Test one | Accept one | G1 |
| REQ-TWO | Two | Owner | ${twoStage} | Test two | Accept two | G2 |
${frozenM3Ids.map((id) => `| ${id} | Summary | Owner | M3 | Test | Accept | G3 |`).join('\n')}
`

test('scope stages include continuous development and exclude M4', () => {
  assert.deepEqual([...VALID_STAGES], [
    'M1',
    'M2',
    'M3',
    'Continuous',
    'Future',
    'NotApplicable',
  ])
})

test('continuous, future and not-applicable rows remain mapped exactly once', () => {
  assert.deepEqual(validateScopeCoverage(spec, matrix('Continuous', 'Future')), {
    requirementCount: 15,
    m3Count: 13,
  })
  assert.equal(
    validateScopeCoverage(spec, matrix('M1/Continuous', 'NotApplicable')).requirementCount,
    15,
  )
})

test('M4 ownership is rejected', () => {
  assert.throws(
    () => validateScopeCoverage(spec, matrix('M4', 'Continuous')),
    /invalid stage token M4 in REQ-ONE/,
  )
})
