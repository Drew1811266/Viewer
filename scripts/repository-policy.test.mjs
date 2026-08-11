import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, readdir, stat } from 'node:fs/promises'
import test from 'node:test'

import { validateFinderDragAppKitWiring } from './validate-finder-drag-appkit-wiring.mjs'

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), 'utf8')
const normalizeNewlines = (text) => text.replace(/\r\n?/g, '\n')
const findDocumentationRows = (index, path) =>
  index
    .split('\n')
    .filter((line) => line.match(/^\| \[[^\]]+\]\(([^)]+)\) \|/)?.[1] === path)

test('documentation row parsing ignores replacement-target links', () => {
  const path = 'superpowers/specs/current-design.md'
  const ownRow = `| [current-design.md](${path}) | Active | — |`
  const index = [
    '| Document | Status | Replaced by |',
    '| --- | --- | --- |',
    ownRow,
    `| [superseded-design.md](superpowers/specs/superseded-design.md) | Superseded | [current-design.md](${path}) |`,
  ].join('\n')

  assert.equal(findDocumentationRows(index, path).length, 1)
  assert.deepEqual(findDocumentationRows(index.replace(`${ownRow}\n`, ''), path), [])
})

test('documentation index declares one status for every listed source', async () => {
  const index = await read('docs/README.md')
  assert.match(index, /\| Document \| Status \| Replaced by \|/)
  assert.match(index, /0005-continuous-development-governance\.md.*Active/)
  assert.match(index, /0004-viewer-0\.1-architecture-freeze\.md.*Superseded/)
  assert.doesNotMatch(index, /\bTBD\b|\bTODO\b/)

  const specs = await readdir(new URL('../docs/superpowers/specs/', import.meta.url), {
    withFileTypes: true,
  })
  for (const spec of specs.filter((entry) => entry.isFile() && entry.name.endsWith('.md'))) {
    const path = `superpowers/specs/${spec.name}`
    const rows = findDocumentationRows(index, path)
    assert.equal(rows.length, 1, `${path} must have exactly one index row`)
    assert.match(
      rows[0],
      /^\| .+ \| (?:Active|Superseded|Historical) \| (?:—|\[.+\]\(.+\)) \|$/,
      `${path} must declare an allowed status and replacement field`,
    )
  }
})

test('active governance documents agree with the index and README points to that index', async () => {
  const [index, rootReadme, openSourceResearch, dependencyHealth] = await Promise.all([
    read('docs/README.md'),
    read('README.md'),
    read('docs/OPEN_SOURCE_RESEARCH.md'),
    read('docs/quality/DEPENDENCY_HEALTH.md'),
  ])
  const activeSources = [
    ['OPEN_SOURCE_RESEARCH.md', openSourceResearch],
    ['quality/DEPENDENCY_HEALTH.md', dependencyHealth],
  ]
  for (const [path, source] of activeSources) {
    const rows = findDocumentationRows(index, path)
    assert.equal(rows.length, 1, `${path} must have exactly one index row`)
    assert.match(rows[0], /^\| .+ \| Active \| — \|$/, `${path} must be indexed as Active`)
    assert.match(source, /^> Status: Active$/m, `${path} must declare active metadata`)
  }

  const historicalRoadmap =
    'superpowers/plans/2026-07-23-viewer-engineering-optimization-roadmap.md'
  const roadmapRows = findDocumentationRows(index, historicalRoadmap)
  assert.equal(roadmapRows.length, 1, `${historicalRoadmap} must have exactly one index row`)
  assert.match(
    roadmapRows[0],
    /^\| .+ \| Historical \| — \|$/,
    `${historicalRoadmap} must remain Historical`,
  )

  assert.ok(
    rootReadme.includes('](docs/README.md)'),
    'README must link the documentation index',
  )
})

const expectedToolchain = `[toolchain]
channel = "1.97.0"
components = ["clippy", "rustfmt"]
targets = ["aarch64-apple-darwin"]
profile = "minimal"
`

const expectedPackageManager = 'pnpm@10.0.0'
const expectedLicenseHash = 'a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2'
const cargoManifests = [
  'crates/viewer-application/Cargo.toml',
  'crates/viewer-domain/Cargo.toml',
  'crates/viewer-infrastructure/Cargo.toml',
  'crates/viewer-platform-macos/Cargo.toml',
  'crates/viewer-test-support/Cargo.toml',
  'crates/viewer-video-mpv/Cargo.toml',
  'src-tauri/Cargo.toml',
]
const directDependencies = [
  '@biomejs/biome',
  '@vitest/coverage-v8',
  'ammonia',
  'async-trait',
  'blake3',
  'block2',
  'dispatch2',
  'encoding_rs',
  'getrandom',
  'libc',
  'libloading',
  'nucleo-matcher',
  'notify',
  'notify-debouncer-full',
  'objc2',
  'objc2-app-kit',
  'objc2-core-foundation',
  'objc2-core-graphics',
  'objc2-foundation',
  'objc2-image-io',
  'objc2-quick-look-thumbnailing',
  'playwright',
  'pulldown-cmark',
  'rusqlite',
  'serde',
  'serde_json',
  'sha2',
  'tempfile',
  'thiserror',
  'tokio',
  'tokio-util',
  'trash',
  'uuid',
  'walkdir',
  'tauri',
  'tauri-build',
  'tauri-plugin-dialog',
  '@tauri-apps/api',
  '@tauri-apps/cli',
  '@tauri-apps/plugin-dialog',
  'react',
  'react-dom',
  '@testing-library/jest-dom',
  '@testing-library/react',
  '@types/node',
  '@types/react',
  '@types/react-dom',
  '@vitejs/plugin-react',
  'jsdom',
  'typescript',
  'vite',
  'vitest',
]

const extractCIJobBlocks = (workflow) => {
  const lines = normalizeNewlines(workflow).split('\n')
  const jobsStart = lines.indexOf('jobs:')
  assert.notEqual(jobsStart, -1, 'CI workflow must define jobs')

  const starts = []
  for (let index = jobsStart + 1; index < lines.length; index += 1) {
    if (/^\S/.test(lines[index])) break
    if (!/^  \S/.test(lines[index]) || /^\s*#/.test(lines[index])) continue
    const job = lines[index].match(/^  ([A-Za-z][A-Za-z0-9_-]*):\s*(?:#.*)?$/)
    assert.ok(job, `CI job key must be an unquoted two-space identifier: ${lines[index]}`)
    starts.push({ name: job[1], index })
  }

  return new Map(
    starts.map((job, index) => [
      job.name,
      lines.slice(job.index, starts[index + 1]?.index).join('\n'),
    ]),
  )
}

const extractCIJobStepBlocks = (job) => {
  const lines = job.split('\n')
  const starts = []
  for (let index = 0; index < lines.length; index += 1) {
    const step = lines[index].match(/^      - name: (.+)$/)
    if (step) starts.push({ name: step[1], index })
  }

  return starts.map((step, index) => ({
    name: step.name,
    block: lines.slice(step.index, starts[index + 1]?.index).join('\n'),
  }))
}

const validateDeterministicCIWorkflow = (workflow) => {
  const topLevelPermissions = normalizeNewlines(workflow).match(/^permissions:\n((?:  [^\n]*\n?)*)/m)
  assert.ok(topLevelPermissions, 'CI workflow must define top-level permissions')
  assert.equal(topLevelPermissions[1], '  contents: read\n', 'CI permissions must be contents: read')
  assert.doesNotMatch(workflow, /\bpnpm[ \t]+audit\b/, 'CI workflow must not run pnpm audit')
  assert.doesNotMatch(workflow, /\brelease\b/i, 'CI workflow must not include release-oriented content')

  const jobs = extractCIJobBlocks(workflow)
  assert.deepEqual([...jobs.keys()].sort(), ['quality', 'security'], 'CI jobs must be exactly quality and security')
  const expectedStepNames = {
    quality: [
      'Check out repository',
      'Assert Apple Silicon runner',
      'Set up Node.js 24.18.0',
      'Activate pnpm 10.0.0 through Corepack',
      'Install Rust 1.97.0 for Apple Silicon',
      'Install JavaScript dependencies',
      'Run quality gate',
      'Assert verification left no artifacts',
    ],
    security: [
      'Check out repository',
      'Assert Apple Silicon runner',
      'Set up Node.js 24.18.0',
      'Activate pnpm 10.0.0 through Corepack',
      'Install Rust 1.97.0 for Apple Silicon',
      'Install JavaScript dependencies',
      'Install cargo-deny 0.20.2',
      'Fetch locked Rust dependency graph',
      'Run security gate',
      'Assert verification left no artifacts',
    ],
  }

  const validateJob = (name, rootCommand) => {
    const job = jobs.get(name)
    assert.ok(job, `${name} job must exist`)
    const stepEntries = job.match(/^      -.*$/gm) ?? []
    assert.ok(stepEntries.length > 0, `${name} job must define block-style steps`)
    for (const stepEntry of stepEntries) {
      assert.match(stepEntry, /^      - name: .+$/, `${name} step must start with - name:`)
    }
    const steps = extractCIJobStepBlocks(job)
    assert.deepEqual(
      steps.map(({ name: stepName }) => stepName),
      expectedStepNames[name],
      `${name} job must use the planned step sequence`,
    )
    assert.doesNotMatch(
      job,
      /^        ["'?!&*]/m,
      `${name} job must use ordinary unquoted step keys`,
    )
    const usesLines = job.match(/^        uses[ \t]*:[ \t]*.*$/gm) ?? []
    const actionReferences = [
      ...job.matchAll(/^        uses[ \t]*:[ \t]+([^@\s]+)@([^\s#]+)(?:[ \t]+#.*)?$/gm),
    ]
    assert.ok(actionReferences.length > 0, `${name} job must use pinned actions`)
    assert.equal(actionReferences.length, usesLines.length, `${name} uses entries must name an action SHA`)
    for (const action of actionReferences) {
      assert.match(action[2], /^[0-9a-f]{40}$/, `${action[1]} must be pinned to a commit`)
    }
    for (const key of ['needs', 'permissions']) {
      assert.doesNotMatch(
        job,
        new RegExp(`^    (?:${key}|"${key}"|'${key}')\\s*:(?:\\s|$)`, 'm'),
        `${name} job must not override ${key}`,
      )
      assert.doesNotMatch(
        job,
        new RegExp(`^    \\?\\s+(?:${key}|"${key}"|'${key}')\\s*$`, 'm'),
        `${name} job must not use an explicit ${key} key`,
      )
    }
    assert.match(job, /^    runs-on: macos-15$/m, `${name} job must run on macos-15`)
    const checkoutAction =
      /^        uses[ \t]*:[ \t]+actions\/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0(?:[ \t]+#.*)?$/m
    const checkoutReferences = [
      ...job.matchAll(/^        uses[ \t]*:[ \t]+actions\/checkout@([^\s#]+)(?:[ \t]+#.*)?$/gm),
    ]
    assert.equal(checkoutReferences.length, 1, `${name} job must have exactly one checkout action`)
    assert.equal(
      checkoutReferences[0][1],
      '9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0',
      `${name} checkout action must use the locked SHA`,
    )
    const checkoutSteps = steps.filter(({ block }) => checkoutAction.test(block))
    assert.equal(checkoutSteps.length, 1, `${name} job must have exactly one pinned checkout action`)
    assert.equal(checkoutSteps[0].name, 'Check out repository', `${name} checkout step name`)
    const setupNodeAction =
      /^        uses[ \t]*:[ \t]+actions\/setup-node@820762786026740c76f36085b0efc47a31fe5020(?:[ \t]+#.*)?$/m
    const setupNodeReferences = [
      ...job.matchAll(/^        uses[ \t]*:[ \t]+actions\/setup-node@([^\s#]+)(?:[ \t]+#.*)?$/gm),
    ]
    assert.equal(setupNodeReferences.length, 1, `${name} job must have exactly one setup-node action`)
    assert.equal(
      setupNodeReferences[0][1],
      '820762786026740c76f36085b0efc47a31fe5020',
      `${name} setup-node action must use the locked SHA`,
    )
    const setupNodeSteps = steps.filter(({ block }) => setupNodeAction.test(block))
    assert.equal(setupNodeSteps.length, 1, `${name} job must have exactly one pinned setup-node action`)
    assert.equal(setupNodeSteps[0].name, 'Set up Node.js 24.18.0', `${name} setup-node step name`)
    assert.match(
      setupNodeSteps[0].block,
      /^        uses[ \t]*:[ \t]+actions\/setup-node@820762786026740c76f36085b0efc47a31fe5020(?:[ \t]+#.*)?\n        with:\n          node-version: "24\.18\.0"$/m,
      `${name} setup-node action must have the locked node-version in its with block`,
    )
    assert.match(
      job,
      /^        run: test "\$\(uname -m\)" = "arm64"$/m,
      `${name} job must assert Apple Silicon`,
    )
    assert.match(
      job,
      /^          corepack prepare pnpm@10\.0\.0 --activate$/m,
      `${name} job must activate pnpm 10.0.0`,
    )
    assert.match(
      job,
      /^          rustup toolchain install 1\.97\.0 --profile minimal --component clippy,rustfmt --target aarch64-apple-darwin$/m,
      `${name} job must install the locked Rust toolchain`,
    )
    assert.match(
      job,
      /^        run: pnpm install --frozen-lockfile$/m,
      `${name} job must install frozen JavaScript dependencies`,
    )
    assert.match(
      job,
      /^        run: test -z "\$\(git status --porcelain\)"$/m,
      `${name} job must assert a clean worktree`,
    )
    assert.match(job, new RegExp(`^        run: pnpm ${rootCommand}$`, 'm'), `${name} job command`)
    assert.doesNotMatch(
      job,
      new RegExp(`^        run: pnpm ${(rootCommand === 'quality' ? 'security' : 'quality')}$`, 'm'),
      `${name} job must not run the other root gate`,
    )
  }

  validateJob('quality', 'quality')
  validateJob('security', 'security')
  assert.match(
    jobs.get('security'),
    /^        run: cargo install cargo-deny --version 0\.20\.2 --locked$/m,
    'security job must install locked cargo-deny 0.20.2',
  )
  assert.match(
    jobs.get('security'),
    /^        run: cargo fetch --locked$/m,
    'security job must fetch the locked workspace graph before its offline policy gate',
  )
}

const mutateJob = (workflow, name, from, to) => {
  const jobStart = workflow.indexOf(`  ${name}:\n`)
  assert.notEqual(jobStart, -1, `${name} job must exist before mutation`)
  const remainder = workflow.slice(jobStart)
  const nextJob = remainder.slice(name.length + 4).search(/^  [^\s][^:\n]*:\n/m)
  const jobEnd = nextJob === -1 ? workflow.length : jobStart + name.length + 4 + nextJob
  const job = workflow.slice(jobStart, jobEnd)
  assert.notEqual(job.indexOf(from), -1, `${name} mutation target must exist`)
  return `${workflow.slice(0, jobStart)}${job.replace(from, to)}${workflow.slice(jobEnd)}`
}

test('CI deterministic job invariants reject policy bypass mutations', async () => {
  const workflow = await read('.github/workflows/ci.yml')
  const mutations = [
    {
      label: 'unexpected job',
      workflow: workflow.replace('jobs:\n', 'jobs:\n  release:\n    runs-on: macos-15\n'),
    },
    {
      label: 'quoted unexpected job',
      workflow: workflow.replace('jobs:\n', 'jobs:\n  "release":\n    runs-on: macos-15\n'),
    },
    {
      label: 'explicit unexpected job',
      workflow: workflow.replace('jobs:\n', 'jobs:\n  ? release\n  :\n    runs-on: macos-15\n'),
    },
    {
      label: 'job-level write-all permissions',
      workflow: mutateJob(workflow, 'quality', '    steps:', '    permissions: write-all\n    steps:'),
    },
    {
      label: 'job-level contents write permissions',
      workflow: mutateJob(
        workflow,
        'security',
        '    steps:',
        '    permissions:\n      contents: write\n    steps:',
      ),
    },
    {
      label: 'job-level quoted scalar permissions',
      workflow: mutateJob(workflow, 'quality', '    steps:', '    "permissions": write-all\n    steps:'),
    },
    {
      label: 'job-level quoted mapping permissions',
      workflow: mutateJob(
        workflow,
        'security',
        '    steps:',
        '    "permissions":\n      contents: write\n    steps:',
      ),
    },
    {
      label: 'job-level explicit permissions',
      workflow: mutateJob(
        workflow,
        'quality',
        '    steps:',
        '    ? permissions\n    : write-all\n    steps:',
      ),
    },
    {
      label: 'quality job dependency',
      workflow: mutateJob(workflow, 'quality', '    steps:', '    needs: security\n    steps:'),
    },
    {
      label: 'security job dependency',
      workflow: mutateJob(workflow, 'security', '    steps:', '    needs: quality\n    steps:'),
    },
    {
      label: 'job-level quoted dependency',
      workflow: mutateJob(workflow, 'quality', '    steps:', '    "needs": security\n    steps:'),
    },
    {
      label: 'job-level explicit dependency',
      workflow: mutateJob(
        workflow,
        'security',
        '    steps:',
        '    ? needs\n    : quality\n    steps:',
      ),
    },
    {
      label: 'quality runner',
      workflow: mutateJob(workflow, 'quality', 'runs-on: macos-15', 'runs-on: ubuntu-latest'),
    },
    {
      label: 'quality Apple Silicon assertion',
      workflow: mutateJob(
        workflow,
        'quality',
        'test "$(uname -m)" = "arm64"',
        'test "$(uname -m)" = "x86_64"',
      ),
    },
    {
      label: 'quality Node version',
      workflow: mutateJob(workflow, 'quality', 'node-version: "24.18.0"', 'node-version: "24"'),
    },
    {
      label: 'quality setup-node action identity',
      workflow: mutateJob(
        workflow,
        'quality',
        'actions/setup-node@820762786026740c76f36085b0efc47a31fe5020',
        'actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0',
      ),
    },
    {
      label: 'security setup-node action identity',
      workflow: mutateJob(
        workflow,
        'security',
        'actions/setup-node@820762786026740c76f36085b0efc47a31fe5020',
        'actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0',
      ),
    },
    {
      label: 'quality checkout action identity',
      workflow: mutateJob(
        workflow,
        'quality',
        'actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0',
        'actions/setup-node@820762786026740c76f36085b0efc47a31fe5020',
      ),
    },
    {
      label: 'security checkout action identity',
      workflow: mutateJob(
        workflow,
        'security',
        'actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0',
        'actions/setup-node@820762786026740c76f36085b0efc47a31fe5020',
      ),
    },
    {
      label: 'duplicate checkout action family',
      workflow: mutateJob(
        workflow,
        'quality',
        '      - name: Assert Apple Silicon runner',
        '      - name: Decoy checkout\n        uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\n      - name: Assert Apple Silicon runner',
      ),
    },
    {
      label: 'duplicate setup-node action family',
      workflow: mutateJob(
        workflow,
        'security',
        '      - name: Activate pnpm 10.0.0 through Corepack',
        '      - name: Decoy setup-node\n        uses: actions/setup-node@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n\n      - name: Activate pnpm 10.0.0 through Corepack',
      ),
    },
    {
      label: 'nameless duplicate pinned checkout step',
      workflow: mutateJob(
        workflow,
        'quality',
        '      - name: Assert Apple Silicon runner',
        '      - uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0\n\n      - name: Assert Apple Silicon runner',
      ),
    },
    {
      label: 'quoted unpinned flow-mapping checkout step',
      workflow: mutateJob(
        workflow,
        'security',
        '      - name: Activate pnpm 10.0.0 through Corepack',
        '      - { name: Flow-mapping checkout, "uses": actions/checkout@v4 }\n\n      - name: Activate pnpm 10.0.0 through Corepack',
      ),
    },
    {
      label: 'quoted unpinned checkout action',
      workflow: mutateJob(
        workflow,
        'quality',
        '      - name: Assert Apple Silicon runner',
        '      - name: Quoted unpinned checkout\n        "uses": actions/checkout@v4\n\n      - name: Assert Apple Silicon runner',
      ),
    },
    {
      label: 'explicit unpinned setup-node action',
      workflow: mutateJob(
        workflow,
        'security',
        '      - name: Activate pnpm 10.0.0 through Corepack',
        '      - name: Explicit unpinned setup-node\n        ? uses\n        : actions/setup-node@v4\n\n      - name: Activate pnpm 10.0.0 through Corepack',
      ),
    },
    {
      label: 'space-before-colon unpinned checkout action',
      workflow: mutateJob(
        workflow,
        'quality',
        '      - name: Assert Apple Silicon runner',
        '      - name: Space-key unpinned checkout\n        uses : actions/checkout@v4\n\n      - name: Assert Apple Silicon runner',
      ),
    },
    {
      label: 'tab-before-colon unpinned setup-node action',
      workflow: mutateJob(
        workflow,
        'security',
        '      - name: Activate pnpm 10.0.0 through Corepack',
        '      - name: Tab-key unpinned setup-node\n        uses\t: actions/setup-node@v4\n\n      - name: Activate pnpm 10.0.0 through Corepack',
      ),
    },
    {
      label: 'extra Release artifact step',
      workflow: mutateJob(
        workflow,
        'quality',
        '      - name: Assert verification left no artifacts',
        '      - name: Release artifact\n        run: ./scripts/release\n\n      - name: Assert verification left no artifacts',
      ),
    },
    {
      label: 'extra named block step',
      workflow: mutateJob(
        workflow,
        'security',
        '      - name: Assert verification left no artifacts',
        '      - name: Collect diagnostics\n        run: git status --short\n\n      - name: Assert verification left no artifacts',
      ),
    },
    {
      label: 'direct scalar pnpm audit command with extra whitespace',
      workflow: mutateJob(workflow, 'security', 'run: pnpm security', 'run: pnpm   audit'),
    },
    {
      label: 'block-scalar pnpm audit command with extra whitespace',
      workflow: mutateJob(
        workflow,
        'quality',
        '          pnpm --version',
        '          pnpm --version\n          pnpm \t audit',
      ),
    },
    {
      label: 'quality pnpm bootstrap',
      workflow: mutateJob(
        workflow,
        'quality',
        'corepack prepare pnpm@10.0.0 --activate',
        'corepack prepare pnpm@latest --activate',
      ),
    },
    {
      label: 'quality Rust bootstrap',
      workflow: mutateJob(
        workflow,
        'quality',
        'rustup toolchain install 1.97.0 --profile minimal --component clippy,rustfmt --target aarch64-apple-darwin',
        'rustup toolchain install stable',
      ),
    },
    {
      label: 'quality frozen install',
      workflow: mutateJob(
        workflow,
        'quality',
        'pnpm install --frozen-lockfile',
        'pnpm install',
      ),
    },
    {
      label: 'quality clean status assertion',
      workflow: mutateJob(
        workflow,
        'quality',
        'test -z "$(git status --porcelain)"',
        'git status --porcelain',
      ),
    },
    {
      label: 'quality root command',
      workflow: mutateJob(workflow, 'quality', 'run: pnpm quality', 'run: pnpm verify'),
    },
    {
      label: 'security root command',
      workflow: mutateJob(workflow, 'security', 'run: pnpm security', 'run: pnpm verify'),
    },
    {
      label: 'security locked cargo-deny install',
      workflow: mutateJob(
        workflow,
        'security',
        'cargo install cargo-deny --version 0.20.2 --locked',
        'cargo install cargo-deny --version 0.20.2',
      ),
    },
  ]

  assert.doesNotThrow(() => validateDeterministicCIWorkflow(workflow))
  for (const mutation of mutations) {
    assert.throws(
      () => validateDeterministicCIWorkflow(mutation.workflow),
      undefined,
      mutation.label,
    )
  }
})

test('CI defines independent deterministic quality and security gates', async () => {
  const [workflow, toolchain, packageText] = await Promise.all([
    read('.github/workflows/ci.yml'),
    read('rust-toolchain.toml'),
    read('package.json'),
  ])
  const packageJson = JSON.parse(packageText)

  assert.match(workflow, /^name: CI/m)
  assert.match(workflow, /^\s*push:\s*$/m)
  assert.match(workflow, /^\s*pull_request:\s*$/m)
  assert.match(workflow, /permissions:\n\s+contents: read/)
  assert.match(workflow, /runs-on: macos-15/g)
  assert.match(workflow, /pnpm install --frozen-lockfile/g)
  assert.match(workflow, /run: pnpm quality/)
  assert.match(workflow, /run: pnpm security/)
  assert.doesNotMatch(workflow, /run: pnpm audit/)
  validateDeterministicCIWorkflow(workflow)
  assert.deepEqual([...extractCIJobBlocks(workflow).keys()].sort(), ['quality', 'security'])

  const actions = [...workflow.matchAll(/uses[ \t]*:[ \t]+([^@\s]+)@([^\s#]+)/g)]
  assert.ok(actions.length > 0, 'workflow must use pinned actions')
  for (const action of actions) {
    assert.match(action[2], /^[0-9a-f]{40}$/, `${action[1]} must be pinned to a commit`)
  }

  assert.equal(normalizeNewlines(toolchain), expectedToolchain)
  assert.equal(packageJson.packageManager, expectedPackageManager)
  assert.equal(
    packageJson.scripts['test:policy'],
    'node --test scripts/repository-policy.test.mjs scripts/scope-coverage.test.mjs && node scripts/check-scope-coverage.mjs',
  )
  assert.match(packageJson.scripts.quality, /^pnpm test:policy &&/)
  assert.match(packageJson.scripts.quality, /scripts\/verify-clean\.test\.mjs/)
  assert.match(packageJson.scripts.quality, /pnpm --dir ui check/)
  assert.match(packageJson.scripts.quality, /pnpm --dir ui test/)
  assert.match(packageJson.scripts.quality, /pnpm --dir ui build/)
  assert.match(packageJson.scripts.quality, /cargo fmt --check/)
  assert.match(packageJson.scripts.quality, /cargo clippy --locked --workspace --all-targets/)
  assert.match(packageJson.scripts.quality, /cargo test --locked --workspace/)
  assert.match(packageJson.scripts.security, /check-tauri-security\.sh/)
  assert.match(
    packageJson.scripts.security,
    /cargo deny --offline --locked check bans licenses sources/,
  )
  assert.doesNotMatch(packageJson.scripts.security, /check(?:\s+advisories|\s*&&)/)
  assert.match(packageJson.scripts.security, /node scripts\/check-npm-licenses\.mjs/)
  assert.equal(packageJson.scripts.verify, 'pnpm quality && pnpm security')
  assert.equal(packageJson.scripts['verify:clean'], 'node scripts/verify-clean.mjs')
})

test('ordinary package policy evaluates active scope without a network advisory gate', async () => {
  const packageJson = JSON.parse(await read('package.json'))

  assert.equal(
    packageJson.scripts['test:policy'],
    'node --test scripts/repository-policy.test.mjs scripts/scope-coverage.test.mjs && node scripts/check-scope-coverage.mjs',
  )
  assert.match(packageJson.scripts.quality, /^pnpm test:policy &&/)
  assert.equal(
    packageJson.scripts.security,
    './scripts/check-tauri-security.sh && cargo deny --offline --locked check bans licenses sources && node scripts/check-npm-licenses.mjs',
  )
})

test('Apache-2.0 and the Viewer 0.1 direct dependency inventory are frozen', async () => {
  const [license, workspace, packageText, uiPackageText, notices, acknowledgements, ...manifests] =
    await Promise.all([
      read('LICENSE'),
      read('Cargo.toml'),
      read('package.json'),
      read('ui/package.json'),
      read('THIRD_PARTY_NOTICES.md'),
      read('ACKNOWLEDGEMENTS.md'),
      ...cargoManifests.map(read),
    ])

  assert.equal(createHash('sha256').update(license).digest('hex'), expectedLicenseHash)
  assert.match(workspace, /\[workspace\.package\][\s\S]*?license = "Apache-2\.0"/)
  for (const [index, manifest] of manifests.entries()) {
    assert.match(manifest, /\[package\][\s\S]*?license\.workspace = true/, cargoManifests[index])
  }
  assert.equal(JSON.parse(packageText).license, 'Apache-2.0')
  assert.equal(JSON.parse(uiPackageText).license, 'Apache-2.0')
  assert.doesNotMatch(acknowledgements, /license has not yet been selected|No project license is implied/i)

  for (const dependency of directDependencies) {
    assert.ok(notices.includes(`| \`${dependency}\` |`), dependency)
  }

  const observedDependencies = collectDirectDependencies({
    workspace,
    manifests,
    packages: [JSON.parse(packageText), JSON.parse(uiPackageText)],
  })
  assert.deepEqual(observedDependencies, [...directDependencies].sort())

  const lockCheck = await stat(new URL('../scripts/check-locked-dependencies.sh', import.meta.url))
  assert.ok((lockCheck.mode & 0o111) !== 0, 'locked dependency check must be executable')
})

test('third-party notices record the locked ammonia version', async () => {
  const [lockfile, notices] = await Promise.all([
    read('Cargo.lock'),
    read('THIRD_PARTY_NOTICES.md'),
  ])
  const ammoniaPackage = normalizeNewlines(lockfile)
    .split('\n[[package]]\n')
    .find((block) => /^name = "ammonia"$/m.test(block))
  assert.ok(ammoniaPackage, 'Cargo.lock must contain ammonia')
  const lockedVersion = ammoniaPackage.match(/^version = "([^"]+)"$/m)?.[1]
  assert.ok(lockedVersion, 'Cargo.lock ammonia entry must contain a version')
  const noticedVersion = normalizeNewlines(notices)
    .match(/^\| `ammonia` \| ([^|]+?) \|/m)?.[1]
    .trim()
  assert.equal(
    noticedVersion,
    lockedVersion,
    'THIRD_PARTY_NOTICES.md ammonia version must match Cargo.lock',
  )
})

test('third-party notices record the inherited locked dispatch2 dependency', async () => {
  const [lockfile, notices] = await Promise.all([
    read('Cargo.lock'),
    read('THIRD_PARTY_NOTICES.md'),
  ])
  const dispatchPackage = normalizeNewlines(lockfile)
    .split('\n[[package]]\n')
    .find((block) => /^name = "dispatch2"$/m.test(block))
  assert.ok(dispatchPackage, 'Cargo.lock must contain dispatch2')
  const lockedVersion = dispatchPackage.match(/^version = "([^"]+)"$/m)?.[1]
  assert.ok(lockedVersion, 'Cargo.lock dispatch2 entry must contain a version')
  const noticedVersion = normalizeNewlines(notices)
    .match(/^\| `dispatch2` \| ([^|]+?) \|/m)?.[1]
    .trim()
  assert.equal(
    noticedVersion,
    lockedVersion,
    'THIRD_PARTY_NOTICES.md dispatch2 version must match Cargo.lock',
  )
})

test('M3 adds no broad desktop capability or network/update dependency', async () => {
  const [capabilityText, workspace, desktopManifest, packageText, uiPackageText] =
    await Promise.all([
      read('src-tauri/capabilities/main.json'),
      read('Cargo.toml'),
      read('src-tauri/Cargo.toml'),
      read('package.json'),
      read('ui/package.json'),
    ])
  const capability = JSON.parse(capabilityText)
  assert.deepEqual(capability.windows, ['main'])
  assert.deepEqual(capability.permissions, [
    'dialog:allow-open',
    'core:event:allow-listen',
    'core:event:allow-unlisten',
  ])
  assert.equal(Object.hasOwn(capability, 'remote'), false)

  const dependencyText = `${workspace}\n${desktopManifest}\n${packageText}\n${uiPackageText}`
  assert.doesNotMatch(
    dependencyText,
    /(?:tauri-plugin-(?:fs|http|shell|updater|websocket)|reqwest|axios|electron-updater)/i,
  )
})

test('Finder export starts a synthetic AppKit drag from the owning window content view', async () => {
  const adapter = await read('crates/viewer-platform-macos/src/files/drag.rs')

  assert.doesNotThrow(() => validateFinderDragAppKitWiring(adapter))
  for (const drift of [
    adapter.replace(
      'content_view.beginDraggingSessionWithItems_event_source(',
      'self.view.beginDraggingSessionWithItems_event_source(',
    ),
    adapter.replace('&event,', 'current_event.as_ref().unwrap(),'),
  ]) {
    assert.throws(() => validateFinderDragAppKitWiring(drift), /exact AppKit drag wiring/i)
  }
  assert.match(adapter, /mouseLocationOutsideOfEventStream/)
  assert.match(
    adapter,
    /mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure/,
  )
  assert.match(adapter, /contentView\(\)/)
  assert.match(adapter, /fileReferenceURL\(\)/)
  assert.match(adapter, /verify_bound_drag_reference/)
  assert.match(adapter, /O_NOFOLLOW_ANY/)
  assert.match(adapter, /raw_os_error\(\) == Some\(libc::ENOATTR\)/)
  assert.match(adapter, /NSImageNameMultipleDocuments/)
  assert.doesNotMatch(adapter, /NSURL::fileURLWithPath/)
  assert.doesNotMatch(adapter, /iconForFile/)
  assert.doesNotMatch(adapter, /filter\(\|event\| event\.r#type\(\) == NSEventType::LeftMouseDragged\)/)
})

test('registered copy cleanup stays behind the identity-bound staged lease', async () => {
  const [ports, executor, local, staged, fileReference, placement, evidence] = await Promise.all([
    read('crates/viewer-application/src/ports.rs'),
    read('crates/viewer-infrastructure/src/operation/executor.rs'),
    read('crates/viewer-infrastructure/src/operation/copy/local.rs'),
    read('crates/viewer-infrastructure/src/operation/copy/staged.rs'),
    read('crates/viewer-infrastructure/src/operation/copy/file_reference.rs'),
    read('crates/viewer-infrastructure/src/operation/copy/placement.rs'),
    read('crates/viewer-infrastructure/src/operation/copy/evidence.rs'),
  ])
  const copyProduction = [local, staged, fileReference, placement, evidence].join('\n')
  const pathnameDelete = /\b(?:(?:std|tokio)::)?fs::remove_file\s*\(|\blibc::unlink(?:at)?\s*\(/

  const validateCleanupPolicy = (sources) => {
    assert.doesNotMatch(sources.ports, /remove_registered_temporary/)
    assert.doesNotMatch(sources.copyProduction, /remove_registered_temporary/)
    assert.doesNotMatch(sources.executor, /remove_registered_temporary/)
    assert.doesNotMatch(
      sources.copyProduction,
      pathnameDelete,
      'copy production must not use pathname deletion',
    )
    assert.doesNotMatch(sources.executor, pathnameDelete)
    assert.match(sources.ports, /pub struct StagedCopy/)
    assert.match(sources.ports, /trait StagedCopyLeasePort/)
    assert.match(sources.executor, /create_staged_copy_cancellable_verified\(/)
    assert.doesNotMatch(sources.executor, /\.create_and_copy_cancellable_verified\(/)
    assert.match(
      sources.staged,
      /impl StagedCopyLeasePort for MacStagedCopyLease/,
      'staged.rs must own the staged-copy lease implementation',
    )
    assert.match(
      sources.fileReference,
      /\bfn FSUnlinkObject\(reference: \*const BoundFileReference\) -> i32;/,
      'file_reference.rs must own the identity unlink primitive',
    )
    assert.match(
      sources.fileReference,
      /pub\(super\) fn unlink_file_reference\(\s*reference: &BoundFileReference,\s*error_path: &Path,\s*\) -> Result<\(\), FileOperationError> \{[\s\S]*?let status = unsafe \{ FSUnlinkObject\(reference\) \};/,
      'file_reference.rs must implement identity-bound unlink through FSUnlinkObject',
    )
    assert.match(
      sources.staged,
      /impl Drop for MacStagedCopyLease \{\s*fn drop\(&mut self\) \{\s*if self\.armed \{\s*let _ = unlink_file_reference\(\s*&self\.temporary_reference,\s*&self\.temporary_path\s*\);\s*let _ = self\.temporary_parent\.sync_all\(\);\s*self\.armed = false;\s*\}\s*\}\s*\}/,
      'staged-copy Drop must unlink the bound identity and sync its parent',
    )
  }

  const sources = { ports, executor, copyProduction, staged, fileReference }
  validateCleanupPolicy(sources)

  const mutations = [
    {
      name: 'pathname deletion',
      sources: {
        ...sources,
        copyProduction: `${copyProduction}\nstd::fs::remove_file(path)?;`,
      },
      message: /copy production must not use pathname deletion/,
    },
    {
      name: 'missing staged lease implementation',
      sources: {
        ...sources,
        staged: staged.replace(
          'impl StagedCopyLeasePort for MacStagedCopyLease',
          'impl MissingStagedCopyLeasePort for MacStagedCopyLease',
        ),
      },
      message: /staged\.rs must own the staged-copy lease implementation/,
    },
    {
      name: 'missing identity unlink primitive',
      sources: {
        ...sources,
        fileReference: fileReference.replace(
          'pub(super) fn unlink_file_reference(',
          'pub(super) fn missing_unlink_file_reference(',
        ),
      },
      message: /file_reference\.rs must implement identity-bound unlink through FSUnlinkObject/,
    },
    {
      name: 'missing Drop identity cleanup',
      sources: {
        ...sources,
        staged: staged.replace(
          'let _ = unlink_file_reference(',
          'let _ = missing_unlink_file_reference(',
        ),
      },
      message: /staged-copy Drop must unlink the bound identity and sync its parent/,
    },
    {
      name: 'missing Drop parent sync',
      sources: {
        ...sources,
        staged: staged.replace(
          'let _ = self.temporary_parent.sync_all();',
          'let _ = Ok::<(), ()>(());',
        ),
      },
      message: /staged-copy Drop must unlink the bound identity and sync its parent/,
    },
  ]
  for (const mutation of mutations) {
    assert.throws(
      () => validateCleanupPolicy(mutation.sources),
      mutation.message,
      `${mutation.name} must fail the cleanup policy`,
    )
  }
})

test('the macOS release command is non-interactive and uses a valid bundle identifier', async () => {
  const [packageText, tauriText] = await Promise.all([
    read('package.json'),
    read('src-tauri/tauri.conf.json'),
  ])
  const packageJson = JSON.parse(packageText)
  const tauri = JSON.parse(tauriText)

  assert.equal(
    packageJson.scripts['build:macos'],
    'CI=true tauri build --target aarch64-apple-darwin',
  )
  assert.equal(tauri.identifier, 'com.viewer.desktop')
  assert.doesNotMatch(tauri.identifier, /\.app$/)
  assert.deepEqual(tauri.bundle.targets, ['app', 'dmg'])
  assert.equal(tauri.bundle.macOS.minimumSystemVersion, '13.0')
  assert.equal(tauri.bundle.macOS.signingIdentity, '-')
})

test('the Tauri host enforces the Viewer compact-layout minimum width', async () => {
  const tauri = JSON.parse(await read('src-tauri/tauri.conf.json'))
  const [mainWindow] = tauri.app.windows

  assert.equal(mainWindow.minWidth, 500)
  assert.ok(mainWindow.width >= mainWindow.minWidth)
})

test('the Viewer product version has one Cargo source and matches the Tauri bundle', async () => {
  const memberManifests = [
    'src-tauri/Cargo.toml',
    'crates/viewer-domain/Cargo.toml',
    'crates/viewer-application/Cargo.toml',
    'crates/viewer-infrastructure/Cargo.toml',
    'crates/viewer-platform-macos/Cargo.toml',
    'crates/viewer-test-support/Cargo.toml',
  ]
  const [workspaceManifest, tauriText, ...members] = await Promise.all([
    read('Cargo.toml'),
    read('src-tauri/tauri.conf.json'),
    ...memberManifests.map((manifest) => read(manifest)),
  ])
  const workspacePackage = workspaceManifest.match(
    /\[workspace\.package\]([\s\S]*?)(?=\n\[|$)/,
  )?.[1]
  const workspaceVersion = workspacePackage?.match(
    /^version\s*=\s*"([^"]+)"$/m,
  )?.[1]

  assert.match(workspaceVersion ?? '', /^\d+\.\d+\.\d+$/)
  assert.equal(JSON.parse(tauriText).version, workspaceVersion)
  for (const [index, member] of members.entries()) {
    assert.match(
      member,
      /^version\.workspace\s*=\s*true$/m,
      `${memberManifests[index]} must inherit the Viewer workspace version`,
    )
  }
})

test('the npm dependency graph rejects unreviewed license expressions', async () => {
  const { validateLicenseInventory } = await import('./check-npm-licenses.mjs')
  const reviewed = {
    MIT: [{ name: 'react', versions: ['19.2.7'] }],
    'Apache-2.0': [{ name: 'typescript', versions: ['6.0.3'] }],
    'MPL-2.0': [{ name: 'lightningcss', versions: ['1.32.0'] }],
    'BlueOak-1.0.0': [{ name: 'lru-cache', versions: ['11.5.2'] }],
  }

  assert.equal(validateLicenseInventory(reviewed), 4)
  assert.throws(
    () => validateLicenseInventory({ 'GPL-3.0-only': [{ name: 'forbidden', versions: ['1.0.0'] }] }),
    /unreviewed npm license expression GPL-3\.0-only: forbidden@1\.0\.0/,
  )
  assert.throws(
    () => validateLicenseInventory({ UNKNOWN: [{ name: 'unlicensed', versions: ['1.0.0'] }] }),
    /unreviewed npm license expression UNKNOWN/,
  )

  const lockCheck = await read('scripts/check-locked-dependencies.sh')
  assert.match(lockCheck, /node scripts\/check-npm-licenses\.mjs/)
})

test('M1 browsing gate is exact, executable and mandatory', async () => {
  const [packageText, gate, gateStat] = await Promise.all([
    read('package.json'),
    read('scripts/run-m1-browsing-gate.sh'),
    stat(new URL('../scripts/run-m1-browsing-gate.sh', import.meta.url)),
  ])
  const packageJson = JSON.parse(packageText)

  assert.equal(packageJson.scripts['gate:m1'], './scripts/run-m1-browsing-gate.sh')
  assert.ok((gateStat.mode & 0o111) !== 0, 'M1 gate must be executable')
  for (const required of [
    'pnpm verify',
    './scripts/check-locked-dependencies.sh',
    './scripts/check-tauri-security.sh',
    'cargo test --locked --test m1_browse_queries',
    'cargo test --locked --test m1_text_preview',
    'cargo test --locked -p viewer-desktop --test m1_desktop_runtime',
    'node scripts/check-scope-coverage.mjs',
    'node scripts/check-m1-ipc-fixtures.mjs',
  ]) {
    assert.ok(gate.includes(required), `M1 gate is missing: ${required}`)
  }
})

test('M1 IPC fixture validation rejects path disclosure and unsupported kinds', async () => {
  const { validateIpcPayload } = await import('./check-m1-ipc-fixtures.mjs')
  assert.doesNotThrow(() =>
    validateIpcPayload({ kind: 'jpeg', relativePath: 'catalog/id-1/front.jpg' }, 'safe'),
  )
  for (const unsafe of [
    { kind: 'jpeg', relativePath: '/Users/private/front.jpg' },
    { kind: 'jpeg', relativePath: 'catalog/.viewer/front.jpg' },
    { kind: 'jpeg', cachePath: '/Users/a/Library/Caches/Viewer/sessions/1' },
    { kind: 'webp', relativePath: 'catalog/front.webp' },
  ]) {
    assert.throws(() => validateIpcPayload(unsafe, 'unsafe'))
  }
})

test('active governance has no M4 owner or Viewer 0.1 delivery gate', async () => {
  const active = await Promise.all([
    read('docs/PRODUCT_SPEC.md'),
    read('docs/TECHNICAL_FOUNDATIONS.md'),
    read('docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md'),
    read('docs/milestones/viewer-0.1-scope-matrix.md'),
    read('docs/architecture/viewer-0.1-api-baseline.md'),
  ])
  const text = active.join('\n')
  for (const forbidden of [
    /\bM4 Internal Release\b/i,
    /M4 内部发布/,
    /next executable action[^.]*\bM4\b/i,
    /M4 final acceptance/i,
    /Viewer 0\.1 for Mac/i,
  ]) {
    assert.doesNotMatch(text, forbidden)
  }
})

test('the UI has one strict lint and format tool', async () => {
  const [rootPackageText, uiPackageText, configText, baselineExceptions] = await Promise.all([
    read('package.json'),
    read('ui/package.json'),
    read('ui/biome.json'),
    read('ui/BIOME_BASELINE_EXCEPTIONS.md'),
  ])
  const rootPackage = JSON.parse(rootPackageText)
  const uiPackage = JSON.parse(uiPackageText)
  const config = JSON.parse(configText)
  const allowedExceptions = {
    a11y: [
      'noNoninteractiveTabindex',
      'noStaticElementInteractions',
      'useAriaPropsSupportedByRole',
      'useKeyWithClickEvents',
      'useSemanticElements',
    ],
    correctness: ['useExhaustiveDependencies'],
    security: ['noDangerouslySetInnerHtml'],
  }
  const allowedExceptionTokens = Object.entries(allowedExceptions)
    .flatMap(([domain, rules]) => rules.map((rule) => `${domain}/${rule}`))
    .sort()
  const documentedExceptionTokens = [...baselineExceptions.matchAll(/`([a-z0-9]+\/[A-Za-z]+)`/g)]
    .map((match) => match[1])
    .sort()
  const configuredExceptionTokens = Object.entries(config.linter.rules)
    .flatMap(([domain, rules]) =>
      typeof rules === 'object' && rules !== null
        ? Object.entries(rules)
            .filter(([, value]) => value === 'off')
            .map(([rule]) => `${domain}/${rule}`)
        : [],
    )
    .sort()
  assert.deepEqual(uiPackage.scripts, {
    dev: 'vite',
    build: 'tsc -b && vite build',
    typecheck: 'tsc -b',
    lint: 'biome lint .',
    format: 'biome format --write .',
    'format:check': 'biome format .',
    check: 'biome check . && tsc -b',
    test: 'vitest run',
    'test:coverage': 'vitest run --coverage',
    preview: 'vite preview',
  })
  assert.equal(uiPackage.devDependencies['@biomejs/biome'], '2.5.5')
  assert.deepEqual(config.files.includes, ['src/**/*.ts', 'src/**/*.tsx', 'vite*.config.ts'])
  assert.deepEqual(config.formatter, {
    enabled: true,
    indentStyle: 'space',
    indentWidth: 2,
    lineWidth: 100,
  })
  assert.deepEqual(config.javascript.formatter, {
    quoteStyle: 'single',
    semicolons: 'asNeeded',
    trailingCommas: 'all',
  })
  assert.equal(config.linter.enabled, true)
  assert.equal(config.linter.rules.recommended, true)
  assert.equal(config.linter.rules.style.noNonNullAssertion, 'error')
  assert.equal(config.linter.rules.suspicious.noFocusedTests, 'error')
  for (const [domain, rules] of Object.entries(allowedExceptions)) {
    assert.notEqual(config.linter.rules[domain], 'off')
    assert.deepEqual(Object.keys(config.linter.rules[domain] ?? {}).sort(), rules)
    for (const rule of rules) {
      assert.equal(config.linter.rules[domain][rule], 'off')
    }
  }
  assert.deepEqual(
    Object.keys(config.linter.rules)
      .filter((name) => name !== 'recommended')
      .sort(),
    [...Object.keys(allowedExceptions), 'style', 'suspicious'].sort(),
  )
  assert.deepEqual(configuredExceptionTokens, allowedExceptionTokens)
  assert.deepEqual(documentedExceptionTokens, allowedExceptionTokens)
  for (const packageJson of [rootPackage, uiPackage]) {
    for (const group of ['dependencies', 'devDependencies']) {
      for (const dependency of Object.keys(packageJson[group] ?? {})) {
        assert.doesNotMatch(dependency, /(?:eslint|prettier)/i)
      }
    }
  }
  for (const path of [
    '.eslintrc',
    '.eslintrc.json',
    '.eslintrc.js',
    '.eslintrc.cjs',
    '.eslintrc.yml',
    '.eslintrc.yaml',
    '.prettierrc',
    '.prettierrc.json',
    '.prettierrc.js',
    '.prettierrc.cjs',
    '.prettierrc.mjs',
    '.prettierrc.yml',
    '.prettierrc.yaml',
    '.prettierrc.toml',
    'eslint.config.js',
    'eslint.config.cjs',
    'eslint.config.mjs',
    'eslint.config.ts',
    'eslint.config.cts',
    'eslint.config.mts',
    'prettier.config.js',
    'prettier.config.cjs',
    'prettier.config.mjs',
    'prettier.config.ts',
    'prettier.config.cts',
    'prettier.config.mts',
    'ui/.eslintrc',
    'ui/.eslintrc.json',
    'ui/.eslintrc.js',
    'ui/.eslintrc.cjs',
    'ui/.eslintrc.yml',
    'ui/.eslintrc.yaml',
    'ui/.prettierrc',
    'ui/.prettierrc.json',
    'ui/.prettierrc.js',
    'ui/.prettierrc.cjs',
    'ui/.prettierrc.mjs',
    'ui/.prettierrc.yml',
    'ui/.prettierrc.yaml',
    'ui/.prettierrc.toml',
    'ui/eslint.config.js',
    'ui/eslint.config.cjs',
    'ui/eslint.config.mjs',
    'ui/eslint.config.ts',
    'ui/eslint.config.cts',
    'ui/eslint.config.mts',
    'ui/prettier.config.js',
    'ui/prettier.config.cjs',
    'ui/prettier.config.mjs',
    'ui/prettier.config.ts',
    'ui/prettier.config.cts',
    'ui/prettier.config.mts',
  ]) {
    await assert.rejects(stat(new URL(`../${path}`, import.meta.url)))
  }
})

function collectDirectDependencies({ workspace, manifests, packages }) {
  const dependencies = new Set()
  for (const manifest of [workspace, ...manifests]) {
    let dependencySection = false
    for (const line of manifest.split(/\r?\n/)) {
      const section = /^\s*\[\[?([^\]]+)\]\]?\s*$/.exec(line)
      if (section !== null) {
        dependencySection = /(?:^|\.)(?:build-|dev-)?dependencies$/.test(section[1])
        continue
      }
      if (!dependencySection) continue
      const dependency = /^\s*([A-Za-z0-9_-]+)\s*=/.exec(line)?.[1]
      if (dependency && !dependency.startsWith('viewer-')) dependencies.add(dependency)
    }
  }
  for (const packageJson of packages) {
    for (const group of ['dependencies', 'devDependencies']) {
      for (const dependency of Object.keys(packageJson[group] ?? {})) dependencies.add(dependency)
    }
  }
  return [...dependencies].sort()
}

test('repository exposes one verification path and recursive macOS hygiene', async () => {
  const [readme, contributing, editorConfig, ignore] = await Promise.all([
    read('README.md'),
    read('CONTRIBUTING.md'),
    read('.editorconfig'),
    read('.gitignore'),
  ])
  assert.match(readme, /pnpm verify:clean/)
  assert.match(contributing, /Do not stage unrelated user changes/i)
  assert.match(editorConfig, /root = true/)
  assert.match(editorConfig, /charset = utf-8/)
  assert.match(ignore, /(?:^|\n)\.DS_Store(?:\n|$)/)
  assert.doesNotMatch(ignore, /(?:^|\n)(?:\*\*\/)?\.viewer\/?(?:\n|$)/)
})

test('desktop runtime responsibilities live in focused modules', async () => {
  const required = [
    'src-tauri/src/dto/mod.rs',
    'src-tauri/src/state/mod.rs',
    'src-tauri/src/state/session.rs',
    'src-tauri/src/state/scan_index.rs',
    'src-tauri/src/state/preview.rs',
    'src-tauri/src/state/markers.rs',
    'src-tauri/src/state/organization.rs',
    'src-tauri/src/operation_runtime/mod.rs',
    'src-tauri/src/operation_runtime/runtime.rs',
    'src-tauri/src/operation_runtime/commit.rs',
  ]
  await Promise.all(required.map((path) => stat(new URL(`../${path}`, import.meta.url))))
})

test('infrastructure adapters are backed by focused modules', async () => {
  const required = [
    'crates/viewer-infrastructure/src/operation/copy/mod.rs',
    'crates/viewer-infrastructure/src/operation/copy/file_reference.rs',
    'crates/viewer-infrastructure/src/operation/copy/staged.rs',
    'crates/viewer-infrastructure/src/operation/copy/evidence.rs',
    'crates/viewer-infrastructure/src/operation/service/mod.rs',
    'crates/viewer-infrastructure/src/operation/service/preflight.rs',
    'crates/viewer-infrastructure/src/operation/service/execution.rs',
    'crates/viewer-infrastructure/src/search/index/mod.rs',
    'crates/viewer-infrastructure/src/search/index/schema.rs',
    'crates/viewer-infrastructure/src/search/index/writer.rs',
    'crates/viewer-infrastructure/src/search/index/projection.rs',
  ]
  await Promise.all(required.map((path) => stat(new URL(`../${path}`, import.meta.url))))
})

test('network dependency audit is scheduled and never a pull-request gate', async () => {
  const workflow = await read('.github/workflows/dependency-audit.yml')
  assert.match(workflow, /schedule:/)
  assert.match(workflow, /workflow_dispatch:/)
  assert.doesNotMatch(workflow, /pull_request:/)
  assert.match(workflow, /pnpm audit --audit-level high/)
  assert.match(workflow, /cargo deny --locked check advisories/)
})

test('quality reports are continuous trends and not a release gate', async () => {
  const packageSource = await read('package.json')
  const packageJson = JSON.parse(packageSource)
  assert.equal(
    packageJson.scripts['quality:report'],
    'pnpm coverage:ui && pnpm coverage:rust && pnpm architecture:health',
  )
  assert.doesNotMatch(packageJson.scripts.verify, /coverage|quality:report|release|M4/)
})
