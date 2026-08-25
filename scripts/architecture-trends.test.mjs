import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  collectSourceFiles,
  compareArchitectureTrends,
  measureFunctions,
  measureSourceFiles,
  runArchitectureTrendsCli,
  validateTrendClassifications,
} from './architecture-trends.mjs'

test('measures production and test lines with strict file thresholds and stable sorting', () => {
  const files = new Map([
    ['src/z-large.ts', 'const z = 1\n'.repeat(1001)],
    ['src/z-large.test.ts', 'test("z", () => {})\n'.repeat(1200)],
    ['src/boundary.ts', 'const b = 1\n'.repeat(1000)],
    ['crates/tests/helper.rs', 'fn helper() {}\n'],
    ['src/a-large.rs', 'const A: usize = 1;\n'.repeat(1001)],
  ])

  assert.deepEqual(measureSourceFiles(files), {
    productionLines: 3002,
    testLines: 1201,
    testToProductionRatio: 1201 / 3002,
    filesOver1000Lines: [
      { path: 'src/a-large.rs', lines: 1001 },
      { path: 'src/z-large.ts', lines: 1001 },
    ],
  })
})

test('returns a zero test ratio when there are no production lines', () => {
  const result = measureSourceFiles(new Map([
    ['src/only.test.ts', 'test("only", () => {})\n'],
  ]))

  assert.equal(result.productionLines, 0)
  assert.equal(result.testLines, 1)
  assert.equal(result.testToProductionRatio, 0)
})

test('counts inline Rust cfg(test) modules as tests without matching string content', () => {
  const files = new Map([
    ['crates/viewer-domain/src/lib.rs', [
      'pub fn production() {}',
      '#[cfg(test)]',
      'mod tests {',
      '    #[test]',
      '    fn works() {}',
      '}',
      '',
    ].join('\n')],
    ['crates/viewer-domain/src/text.rs', 'const EXAMPLE: &str = "#[cfg(test)] mod tests {";\n'],
  ])

  const result = measureSourceFiles(files)
  assert.equal(result.productionLines, 2)
  assert.equal(result.testLines, 5)
})

test('classifies valid inline cfg(test) module layouts without consuming production neighbors', () => {
  const layouts = [
    {
      label: 'same-line attribute and module',
      source: [
        'pub fn before() {}',
        '#[cfg(test)] mod tests {',
        '    fn hidden() {}',
        '}',
        'pub fn after() {}',
        '',
      ].join('\n'),
      testLines: 3,
    },
    {
      label: 'intervening module attribute',
      source: [
        'pub fn before() {}',
        '#[cfg(test)]',
        '#[allow(dead_code)]',
        'mod tests {',
        '    fn hidden() {}',
        '}',
        'pub fn after() {}',
        '',
      ].join('\n'),
      testLines: 5,
    },
    {
      label: 'opening brace on the following line',
      source: [
        'pub fn before() {}',
        '#[cfg(test)]',
        'mod tests',
        '{',
        '    fn hidden() {}',
        '}',
        'pub fn after() {}',
        '',
      ].join('\n'),
      testLines: 5,
    },
  ]

  for (const { label, source, testLines } of layouts) {
    const result = measureSourceFiles(new Map([['src/lib.rs', source]]))
    assert.equal(result.productionLines, 2, `${label}: production lines`)
    assert.equal(result.testLines, testLines, `${label}: test lines`)
  }
})

test('does not classify a neighboring non-test Rust module as test content', () => {
  const source = [
    '#[cfg(not(test))]',
    'mod production {',
    '    fn production_neighbor() {}',
    '}',
    '#[cfg(test)] mod tests {',
    '    fn hidden() {}',
    '}',
    'pub fn after() {}',
    '',
  ].join('\n')

  const result = measureSourceFiles(new Map([['src/lib.rs', source]]))
  assert.equal(result.productionLines, 5)
  assert.equal(result.testLines, 3)
})

test('classifies external cfg(test) module files and leaves similarly named neighbors in production', () => {
  const decisions = '    if (value) value += 1\n'.repeat(15)
  const externalSource = `fn hidden(mut value: usize) {\n${decisions}}\n`
  const parentSource = [
    'pub fn before() {}',
    '#[cfg(test)]',
    '#[allow(dead_code)]',
    'mod quality_tests;',
    'pub fn after() {}',
    '',
  ].join('\n')

  for (const externalPath of [
    'crates/viewer-domain/src/quality_tests.rs',
    'crates/viewer-domain/src/quality_tests/mod.rs',
  ]) {
    const files = new Map([
      ['crates/viewer-domain/src/lib.rs', parentSource],
      [externalPath, externalSource],
      ['crates/viewer-domain/src/quality_tests_helper.rs', 'pub fn production_neighbor() {}\n'],
    ])

    const measured = measureSourceFiles(files)
    assert.equal(measured.productionLines, 3, `${externalPath}: production lines`)
    assert.equal(measured.testLines, 20, `${externalPath}: test lines`)
    assert.deepEqual(
      measureFunctions(files).functionsOverDecisionScore15,
      [],
      `${externalPath}: external test function outliers`,
    )
  }
})

test('recognizes Rust, TypeScript, and hook declarations and sorts decision outliers', () => {
  const decisions = '  if (value) value += 1\n'.repeat(15)
  const result = measureFunctions(new Map([
    ['src/c.rs', `pub const fn const_fn(mut value: usize) {\n${decisions}}\n`],
    ['src/z.rs', `pub(crate) async fn rust_fn(mut value: usize) {\n${decisions}}\n`],
    ['src/a.ts', `export function alpha(value: number) {\n${decisions}}\n`],
    ['src/hooks.ts', `export const useViewer = (value: number) => {\n${decisions}}\n`],
  ]))

  assert.deepEqual(
    result.functionsOverDecisionScore15.map(({ path, name, decisionScore }) => ({
      path,
      name,
      decisionScore,
    })),
    [
      { path: 'src/a.ts', name: 'alpha', decisionScore: 16 },
      { path: 'src/c.rs', name: 'const_fn', decisionScore: 16 },
      { path: 'src/hooks.ts', name: 'useViewer', decisionScore: 16 },
      { path: 'src/z.rs', name: 'rust_fn', decisionScore: 16 },
    ],
  )
})

test('uses strict function thresholds and ignores decision tokens in comments and strings', () => {
  const exactLength = `function exactLength() {\n${'  value += 1\n'.repeat(198)}}\n`
  const overLength = `function overLength() {\n${'  value += 1\n'.repeat(199)}}\n`
  const exactDecisions = [
    'function exactDecisions(value) {',
    '  const text = "if for while match case && || ?"',
    '  // if for while match case && || ?',
    '  /* if for while match case && || ? */',
    ...Array.from({ length: 14 }, () => '  if (value) value += 1'),
    '}',
    '',
  ].join('\n')
  const overDecisions = [
    'function overDecisions(value) {',
    ...Array.from({ length: 15 }, () => '  if (value) value += 1'),
    '}',
    '',
  ].join('\n')

  const result = measureFunctions(new Map([
    ['src/thresholds.ts', `${exactLength}${overLength}${exactDecisions}${overDecisions}`],
  ]))

  assert.deepEqual(
    result.functionsOver200Lines.map(({ name, lines }) => ({ name, lines })),
    [{ name: 'overLength', lines: 201 }],
  )
  assert.deepEqual(
    result.functionsOverDecisionScore15.map(({ name, decisionScore }) => ({
      name,
      decisionScore,
    })),
    [{ name: 'overDecisions', decisionScore: 16 }],
  )
})

test('measures parent functions through their matching brace and does not absorb later siblings', () => {
  const insideParent = Array.from(
    { length: 199 },
    (_, index) => `  const inside${index} = ${index}`,
  )
  const afterSibling = Array.from(
    { length: 200 },
    (_, index) => `const outside${index} = ${index}`,
  )
  const source = [
    'function parent() {',
    '  function nested() {}',
    ...insideParent,
    '}',
    'function laterSibling() {}',
    ...afterSibling,
    '',
  ].join('\n')

  assert.deepEqual(
    measureFunctions(new Map([['src/nested.ts', source]])).functionsOver200Lines,
    [{
      path: 'src/nested.ts',
      name: 'parent',
      startLine: 1,
      lines: 202,
      decisionScore: 1,
    }],
  )
})

test('attributes nested decision points only to the function that contains them', () => {
  const decisions = Array.from({ length: 15 }, () => '    if (value) value += 1')
  const source = [
    'fn parent(mut value: usize) {',
    '    if value > 0 { value += 1; }',
    '    fn nested(mut value: usize) {',
    ...decisions,
    '    }',
    '    if value > 1 { value += 1; }',
    '}',
    'fn later_sibling(mut value: usize) {',
    ...decisions,
    '}',
    '',
  ].join('\n')

  assert.deepEqual(
    measureFunctions(new Map([['src/nested.rs', source]]))
      .functionsOverDecisionScore15,
    [
      {
        path: 'src/nested.rs',
        name: 'nested',
        startLine: 3,
        lines: 17,
        decisionScore: 16,
      },
      {
        path: 'src/nested.rs',
        name: 'later_sibling',
        startLine: 22,
        lines: 17,
        decisionScore: 16,
      },
    ],
  )
})

test('keeps CRLF line offsets aligned when excluding nested function decisions', () => {
  const source = [
    'function parent(value) {',
    ...Array.from({ length: 15 }, () => '  if(value){}'),
    '  function nested() {}',
    '}',
    '',
  ].join('\r\n')

  assert.deepEqual(
    measureFunctions(new Map([['src/crlf.ts', source]])).functionsOverDecisionScore15,
    [{
      path: 'src/crlf.ts',
      name: 'parent',
      startLine: 1,
      lines: 18,
      decisionScore: 16,
    }],
  )
})

test('terminates same-line and expression-bodied declarations at their lexical boundary', () => {
  const afterDeclarations = Array.from(
    { length: 201 },
    (_, index) => `const outside${index} = ${index}`,
  )
  const ternaries = Array.from({ length: 15 }, () => 'value ? 1 : 0').join(' + ')
  const sourceLines = [
    'function sameLine() {}',
    `export const useExpression = (value: boolean) => ${ternaries};`,
    ...afterDeclarations,
    '',
  ]

  for (const lineEnding of ['\n', '\r\n']) {
    const measured = measureFunctions(new Map([
      ['src/boundaries.ts', sourceLines.join(lineEnding)],
    ]))
    assert.deepEqual(measured.functionsOver200Lines, [])
    assert.deepEqual(measured.functionsOverDecisionScore15, [{
      path: 'src/boundaries.ts',
      name: 'useExpression',
      startLine: 2,
      lines: 1,
      decisionScore: 16,
    }])
  }
})

test('skips callable return type objects when finding a TypeScript function body', () => {
  const source = [
    'function factory(): () => { value: number }',
    '{',
    ...Array.from({ length: 15 }, () => '  if (ready) return build()'),
    '}',
    '',
  ].join('\n')

  assert.deepEqual(
    measureFunctions(new Map([['src/factory.ts', source]]))
      .functionsOverDecisionScore15,
    [{
      path: 'src/factory.ts',
      name: 'factory',
      startLine: 1,
      lines: 18,
      decisionScore: 16,
    }],
  )
})

test('does not report declarations from test files or Rust cfg(test) modules', () => {
  const decisions = '    if (value) value += 1\n'.repeat(15)
  const result = measureFunctions(new Map([
    ['src/useHidden.test.ts', `const useHidden = (value) => {\n${decisions}}\n`],
    ['src/lib.rs', [
      'pub fn production() {}',
      '#[cfg(test)]',
      'mod tests {',
      '    fn hidden(mut value: usize) {',
      decisions.trimEnd(),
      '    }',
      '}',
      '',
    ].join('\n')],
  ]))

  assert.deepEqual(result.functionsOver200Lines, [])
  assert.deepEqual(result.functionsOverDecisionScore15, [])
})

test('collects only relative source paths from the selected roots and excludes target content', () => {
  const root = mkdtempSync(join(tmpdir(), 'viewer-architecture-source-'))
  try {
    for (const directory of [
      'crates/viewer-domain/src',
      'src-tauri/src',
      'ui/src/nested',
      'ui/src/target/generated',
      'target/debug',
    ]) {
      mkdirSync(join(root, directory), { recursive: true })
    }
    writeFileSync(join(root, 'crates/viewer-domain/src/lib.rs'), 'pub fn domain() {}\n')
    writeFileSync(join(root, 'src-tauri/src/main.rs'), 'fn main() {}\n')
    writeFileSync(join(root, 'ui/src/nested/view.tsx'), 'export function View() {}\n')
    writeFileSync(join(root, 'ui/src/target/generated/view.ts'), 'function generated() {}\n')
    writeFileSync(join(root, 'target/debug/generated.rs'), 'fn generated() {}\n')

    assert.deepEqual([...collectSourceFiles(root).keys()], [
      'crates/viewer-domain/src/lib.rs',
      'src-tauri/src/main.rs',
      'ui/src/nested/view.tsx',
    ])
  } finally {
    rmSync(root, { recursive: true })
  }
})

const trends = (overrides = {}) => ({
  schemaVersion: 1,
  productionLines: 100,
  testLines: 50,
  testToProductionRatio: 0.5,
  filesOver1000Lines: [],
  functionsOver200Lines: [],
  functionsOverDecisionScore15: [],
  ...overrides,
})

const classifications = (overrides = {}) => ({
  schemaVersion: 1,
  entries: [],
  ...overrides,
})

const classificationEntry = (overrides = {}) => ({
  metric: 'file-over-1000',
  path: 'ui/src/App.tsx',
  symbol: '',
  classification: 'governance-target',
  owner: 'Viewer maintainers',
  rationale: 'Workspace orchestration is the approved frontend architecture pilot.',
  reviewTrigger: 'Re-evaluate when the workspace orchestration plan completes.',
  ...overrides,
})

test('warns only after trend thresholds are crossed and uses stable warning order', () => {
  const baseline = trends({
    filesOver1000Lines: [{ path: 'src/existing.ts', lines: 1001 }],
    functionsOver200Lines: [{
      path: 'src/existing.ts',
      name: 'long',
      startLine: 1,
      lines: 201,
      decisionScore: 1,
    }],
    functionsOverDecisionScore15: [{
      path: 'src/existing.ts',
      name: 'complex',
      startLine: 250,
      lines: 20,
      decisionScore: 16,
    }],
  })
  const boundary = trends({ testToProductionRatio: 0.48 })
  assert.deepEqual(compareArchitectureTrends(boundary, baseline), [])

  const current = trends({
    testToProductionRatio: 0.479,
    filesOver1000Lines: [
      ...baseline.filesOver1000Lines,
      { path: 'src/new.ts', lines: 1001 },
    ],
    functionsOver200Lines: [
      ...baseline.functionsOver200Lines,
      {
        path: 'src/new.ts',
        name: 'longer',
        startLine: 1,
        lines: 201,
        decisionScore: 1,
      },
    ],
    functionsOverDecisionScore15: [
      ...baseline.functionsOverDecisionScore15,
      {
        path: 'src/new.ts',
        name: 'branching',
        startLine: 250,
        lines: 20,
        decisionScore: 16,
      },
    ],
  })

  assert.deepEqual(compareArchitectureTrends(current, baseline), [
    'new production file over 1000 lines: src/new.ts (1001)',
    'new function over 200 lines: src/new.ts:longer at line 1 (201)',
    'new function over decision score 15: src/new.ts:branching at line 250 (16)',
    'test-to-production ratio dropped from 0.5 to 0.479',
  ])
})

test('trend checks report regressions without blocking and never inspect dependencies', () => {
  const output = []
  const errors = []
  const result = runArchitectureTrendsCli(
    ['--check', '/tmp/baseline.json', '/tmp/classifications.json'],
    {
      collect: () => trends({
        filesOver1000Lines: [{ path: 'ui/src/new.ts', lines: 1001 }],
      }),
      readJson: (path) => path.endsWith('baseline.json') ? trends() : classifications(),
      stdout: { write: (value) => output.push(value) },
      stderr: { write: (value) => errors.push(value) },
    },
  )

  assert.equal(result, 0)
  assert.match(errors.join(''), /WARNING: unclassified trend finding/)
  assert.match(output.join(''), /Architecture trend check completed with 1 warning/)
})

test('trend reports contain source metrics only', () => {
  const output = []
  const result = runArchitectureTrendsCli(['--report'], {
    collect: () => trends(),
    stdout: { write: (value) => output.push(value) },
    stderr: { write() {} },
  })

  assert.equal(result, 0)
  assert.deepEqual(JSON.parse(output.join('')), trends())
  assert.equal(Object.hasOwn(JSON.parse(output.join('')), 'workspaceDependencyEdges'), false)
})

test('trend checks fail only for invalid governance data', () => {
  const classification = {
    metric: 'file-over-1000',
    path: 'ui/src/App.tsx',
    symbol: '',
    classification: 'governance-target',
    owner: 'Viewer maintainers',
    rationale: 'Approved architecture pilot.',
    reviewTrigger: 'Review when the pilot completes.',
  }
  const cases = [
    {
      label: 'schema mismatch',
      baseline: trends({ schemaVersion: 2 }),
      registry: classifications(),
      message: /trend baseline schema must be version 1/,
    },
    {
      label: 'missing metric arrays',
      baseline: { schemaVersion: 1 },
      registry: classifications(),
      message: /trend baseline must contain source metric arrays/,
    },
    {
      label: 'duplicate classifications',
      baseline: trends({
        filesOver1000Lines: [{ path: 'ui/src/App.tsx', lines: 1001 }],
      }),
      registry: classifications({ entries: [classification, classification] }),
      message: /duplicate classification: file-over-1000/,
    },
    {
      label: 'unclassified baseline entry',
      baseline: trends({
        filesOver1000Lines: [{ path: 'ui/src/App.tsx', lines: 1001 }],
      }),
      registry: classifications(),
      message: /missing classification: file-over-1000/,
    },
  ]

  for (const entry of cases) {
    const errors = []
    const result = runArchitectureTrendsCli(
      ['--check', '/tmp/baseline.json', '/tmp/classifications.json'],
      {
        collect: () => trends(),
        readJson: (path) => path.endsWith('baseline.json')
          ? entry.baseline
          : entry.registry,
        stdout: { write() {} },
        stderr: { write: (value) => errors.push(value) },
      },
    )

    assert.equal(result, 1, entry.label)
    assert.match(errors.join(''), entry.message, entry.label)
  }
})

test('trend checks reject unreadable JSON governance data', () => {
  const errors = []
  const result = runArchitectureTrendsCli(
    ['--check', '/tmp/baseline.json', '/tmp/classifications.json'],
    {
      collect: () => trends(),
      readJson: () => {
        throw new SyntaxError('Unexpected token')
      },
      stdout: { write() {} },
      stderr: { write: (value) => errors.push(value) },
    },
  )

  assert.equal(result, 1)
  assert.match(errors.join(''), /ERROR: Unexpected token/)
})

test('validates one exact classification for every baseline outlier', () => {
  const baseline = trends({
    filesOver1000Lines: [{ path: 'ui/src/App.tsx', lines: 1001 }],
  })
  const registry = classifications({ entries: [classificationEntry()] })

  assert.deepEqual(validateTrendClassifications(baseline, registry), [])
  assert.deepEqual(
    validateTrendClassifications(baseline, { ...registry, entries: [] }),
    ['missing classification: file-over-1000\0ui/src/App.tsx\0'],
  )
})

test('rejects malformed, duplicate, and unmatched trend classifications', () => {
  const baseline = trends({
    filesOver1000Lines: [{ path: 'ui/src/App.tsx', lines: 1001 }],
  })
  const cases = [
    {
      label: 'blank owner',
      entries: [classificationEntry({ owner: '  ' })],
      message: 'blank owner: file-over-1000\0ui/src/App.tsx\0',
    },
    {
      label: 'blank rationale',
      entries: [classificationEntry({ rationale: '' })],
      message: 'blank rationale: file-over-1000\0ui/src/App.tsx\0',
    },
    {
      label: 'blank review trigger',
      entries: [classificationEntry({ reviewTrigger: '' })],
      message: 'blank reviewTrigger: file-over-1000\0ui/src/App.tsx\0',
    },
    {
      label: 'unknown classification',
      entries: [classificationEntry({ classification: 'deferred' })],
      message: 'unknown classification: file-over-1000\0ui/src/App.tsx\0',
    },
    {
      label: 'duplicate key',
      entries: [classificationEntry(), classificationEntry()],
      message: 'duplicate classification: file-over-1000\0ui/src/App.tsx\0',
    },
    {
      label: 'unmatched key',
      entries: [
        classificationEntry(),
        classificationEntry({ path: 'ui/src/Unexpected.tsx' }),
      ],
      message: 'classification without baseline outlier: file-over-1000\0ui/src/Unexpected.tsx\0',
    },
  ]

  for (const entry of cases) {
    assert.ok(
      validateTrendClassifications(
        baseline,
        classifications({ entries: entry.entries }),
      ).includes(entry.message),
      entry.label,
    )
  }
})

test('supports importing the architecture trends API without running the CLI', () => {
  const moduleUrl = new URL('./architecture-trends.mjs', import.meta.url).href
  assert.doesNotThrow(() => {
    execFileSync(
      process.execPath,
      ['--input-type=module', '--eval', `await import(${JSON.stringify(moduleUrl)})`],
      { stdio: 'pipe' },
    )
  })
})
