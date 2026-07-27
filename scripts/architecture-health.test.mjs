import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  collectSourceFiles,
  collectWorkspaceDependencyEdges,
  compareArchitectureHealth,
  findForbiddenWorkspaceEdges,
  measureFunctions,
  measureSourceFiles,
  readCargoMetadata,
  runArchitectureHealthCli,
} from './architecture-health.mjs'

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

test('invokes locked cargo metadata with the exact deterministic arguments', () => {
  let invocation
  const metadata = readCargoMetadata('/workspace/viewer', (command, args, options) => {
    invocation = { command, args, options }
    return JSON.stringify({ packages: [], workspace_members: [] })
  })

  assert.deepEqual(metadata, { packages: [], workspace_members: [] })
  assert.deepEqual(invocation, {
    command: 'cargo',
    args: ['metadata', '--locked', '--no-deps', '--format-version', '1'],
    options: {
      cwd: '/workspace/viewer',
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  })
})

test('retains sorted normal, build, and dev Viewer workspace edges and excludes external edges', () => {
  const metadata = {
    workspace_members: ['domain', 'application', 'infrastructure', 'support'],
    packages: [
      {
        id: 'infrastructure',
        name: 'viewer-infrastructure',
        manifest_path: '/workspace/crates/viewer-infrastructure/Cargo.toml',
        dependencies: [
          {
            name: 'viewer-test-support',
            kind: 'dev',
            path: '/workspace/crates/viewer-test-support',
            source: null,
          },
          {
            name: 'viewer-domain',
            kind: null,
            path: '/workspace/crates/viewer-domain',
            source: null,
          },
          {
            name: 'viewer-domain',
            kind: null,
            source: 'registry+https://github.com/rust-lang/crates.io-index',
          },
          { name: 'serde', kind: null },
        ],
      },
      {
        id: 'domain',
        name: 'viewer-domain',
        manifest_path: '/workspace/crates/viewer-domain/Cargo.toml',
        dependencies: [],
      },
      {
        id: 'application',
        name: 'viewer-application',
        manifest_path: '/workspace/crates/viewer-application/Cargo.toml',
        dependencies: [
          {
            name: 'viewer-domain',
            kind: 'build',
            path: '/workspace/crates/viewer-domain',
            source: null,
          },
        ],
      },
      {
        id: 'support',
        name: 'viewer-test-support',
        manifest_path: '/workspace/crates/viewer-test-support/Cargo.toml',
        dependencies: [],
      },
    ],
  }

  assert.deepEqual(collectWorkspaceDependencyEdges(metadata), [
    { from: 'viewer-application', to: 'viewer-domain', kind: 'build' },
    { from: 'viewer-infrastructure', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-test-support', kind: 'dev' },
  ])
})

test('accepts the production layer directions and rejects their reversals', () => {
  const accepted = [
    { from: 'viewer-application', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-application', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-platform-macos', to: 'viewer-application', kind: 'normal' },
    { from: 'viewer-platform-macos', to: 'viewer-domain', kind: 'build' },
    { from: 'viewer-desktop', to: 'viewer-infrastructure', kind: 'normal' },
    { from: 'viewer-desktop', to: 'viewer-platform-macos', kind: 'normal' },
  ]
  const rejected = [
    { from: 'viewer-domain', to: 'viewer-application', kind: 'normal' },
    { from: 'viewer-application', to: 'viewer-infrastructure', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-platform-macos', kind: 'normal' },
    { from: 'viewer-platform-macos', to: 'viewer-infrastructure', kind: 'build' },
  ]

  assert.deepEqual(findForbiddenWorkspaceEdges(accepted), [])
  assert.deepEqual(findForbiddenWorkspaceEdges([...accepted, ...rejected]), rejected)
})

test('retains a dev-only test-support edge without blocking but blocks the same normal edge', () => {
  const devEdge = {
    from: 'viewer-infrastructure',
    to: 'viewer-test-support',
    kind: 'dev',
  }
  const normalEdge = { ...devEdge, kind: 'normal' }

  assert.deepEqual(findForbiddenWorkspaceEdges([devEdge]), [])
  assert.deepEqual(findForbiddenWorkspaceEdges([normalEdge]), [normalEdge])
})

const health = (overrides = {}) => ({
  schemaVersion: 1,
  productionLines: 100,
  testLines: 50,
  testToProductionRatio: 0.5,
  filesOver1000Lines: [],
  functionsOver200Lines: [],
  functionsOverDecisionScore15: [],
  workspaceDependencyEdges: [],
  ...overrides,
})

test('warns only after trend thresholds are crossed and uses stable warning order', () => {
  const baseline = health({
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
  const boundary = health({ testToProductionRatio: 0.48 })
  assert.deepEqual(compareArchitectureHealth(boundary, baseline), [])

  const current = health({
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

  assert.deepEqual(compareArchitectureHealth(current, baseline), [
    'new production file over 1000 lines: src/new.ts (1001)',
    'new function over 200 lines: src/new.ts:longer at line 1 (201)',
    'new function over decision score 15: src/new.ts:branching at line 250 (16)',
    'test-to-production ratio dropped from 0.5 to 0.479',
  ])
})

test('CLI update and check keep trend regressions non-blocking and dependency violations blocking', () => {
  const root = mkdtempSync(join(tmpdir(), 'viewer-architecture-cli-'))
  const baselinePath = join(root, 'baseline.json')
  const output = []
  const errors = []
  const streams = {
    stdout: { write: (value) => output.push(value) },
    stderr: { write: (value) => errors.push(value) },
  }
  try {
    assert.equal(runArchitectureHealthCli(
      ['--update', baselinePath],
      { root, collect: () => health(), ...streams },
    ), 0)
    assert.deepEqual(JSON.parse(readFileSync(baselinePath, 'utf8')), health())

    assert.equal(runArchitectureHealthCli(
      ['--check', baselinePath],
      {
        root,
        collect: () => health({
          testToProductionRatio: 0.4,
          filesOver1000Lines: [{ path: 'ui/src/new.ts', lines: 1001 }],
        }),
        ...streams,
      },
    ), 0)
    assert.match(errors.join(''), /WARNING: new production file over 1000 lines/)
    assert.match(errors.join(''), /WARNING: test-to-production ratio dropped/)

    assert.equal(runArchitectureHealthCli(
      ['--check', baselinePath],
      {
        root,
        collect: () => health({
          workspaceDependencyEdges: [{
            from: 'viewer-domain',
            to: 'viewer-application',
            kind: 'normal',
          }],
        }),
        ...streams,
      },
    ), 1)
    assert.match(
      errors.join(''),
      /ERROR: forbidden workspace dependency: viewer-domain -> viewer-application \(normal\)/,
    )
  } finally {
    rmSync(root, { recursive: true })
  }
})

test('supports importing the architecture health API without running the CLI', () => {
  const moduleUrl = new URL('./architecture-health.mjs', import.meta.url).href
  assert.doesNotThrow(() => {
    execFileSync(
      process.execPath,
      ['--input-type=module', '--eval', `await import(${JSON.stringify(moduleUrl)})`],
      { stdio: 'pipe' },
    )
  })
})
