import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, stat } from 'node:fs/promises'
import test from 'node:test'

import { validateFinderDragAppKitWiring } from './validate-finder-drag-appkit-wiring.mjs'

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), 'utf8')
const normalizeNewlines = (text) => text.replace(/\r\n?/g, '\n')

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
  'src-tauri/Cargo.toml',
]
const directDependencies = [
  '@biomejs/biome',
  'ammonia',
  'async-trait',
  'blake3',
  'block2',
  'encoding_rs',
  'getrandom',
  'libc',
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
  'pulldown-cmark',
  'rusqlite',
  'serde',
  'serde_json',
  'tempfile',
  'thiserror',
  'tokio',
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

  const jobs = extractCIJobBlocks(workflow)
  assert.deepEqual([...jobs.keys()].sort(), ['quality', 'security'], 'CI jobs must be exactly quality and security')

  const validateJob = (name, rootCommand) => {
    const job = jobs.get(name)
    assert.ok(job, `${name} job must exist`)
    const steps = extractCIJobStepBlocks(job)
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
      /^        uses: actions\/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0(?:\s+#.*)?$/m
    const checkoutReferences = [
      ...job.matchAll(/^        uses: actions\/checkout@([^\s#]+)(?:\s+#.*)?$/gm),
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
      /^        uses: actions\/setup-node@820762786026740c76f36085b0efc47a31fe5020(?:\s+#.*)?$/m
    const setupNodeReferences = [
      ...job.matchAll(/^        uses: actions\/setup-node@([^\s#]+)(?:\s+#.*)?$/gm),
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
      /^        uses: actions\/setup-node@820762786026740c76f36085b0efc47a31fe5020(?:\s+#.*)?\n        with:\n          node-version: "24\.18\.0"$/m,
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

  const actions = [...workflow.matchAll(/uses:\s+([^@\s]+)@([^\s#]+)/g)]
  assert.ok(actions.length > 0, 'workflow must use pinned actions')
  for (const action of actions) {
    assert.match(action[2], /^[0-9a-f]{40}$/, `${action[1]} must be pinned to a commit`)
  }

  assert.equal(normalizeNewlines(toolchain), expectedToolchain)
  assert.equal(packageJson.packageManager, expectedPackageManager)
  assert.match(packageJson.scripts.quality, /scripts\/repository-policy\.test\.mjs/)
  assert.match(packageJson.scripts.quality, /scripts\/scope-coverage\.test\.mjs/)
  assert.match(packageJson.scripts.quality, /scripts\/verify-clean\.test\.mjs/)
  assert.match(packageJson.scripts.quality, /pnpm --dir ui check/)
  assert.match(packageJson.scripts.quality, /pnpm --dir ui test/)
  assert.match(packageJson.scripts.quality, /pnpm --dir ui build/)
  assert.match(packageJson.scripts.quality, /cargo fmt --check/)
  assert.match(packageJson.scripts.quality, /cargo clippy --locked --workspace --all-targets/)
  assert.match(packageJson.scripts.quality, /cargo test --locked --workspace/)
  assert.match(packageJson.scripts.security, /check-tauri-security\.sh/)
  assert.match(packageJson.scripts.security, /cargo deny --offline --locked check/)
  assert.match(packageJson.scripts.security, /node scripts\/check-npm-licenses\.mjs/)
  assert.equal(packageJson.scripts.verify, 'pnpm quality && pnpm security')
  assert.equal(packageJson.scripts['verify:clean'], 'node scripts/verify-clean.mjs')
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
  const [ports, executor, localMutation] = await Promise.all([
    read('crates/viewer-application/src/ports.rs'),
    read('crates/viewer-infrastructure/src/operation/executor.rs'),
    read('crates/viewer-infrastructure/src/operation/copy.rs'),
  ])
  const localMutationProduction = localMutation.split(
    /\n#\[cfg\([^\n]*\btest\b[^\n]*\)\]\nmod tests\b/,
  )[0]
  const pathnameDelete = /\b(?:(?:std|tokio)::)?fs::remove_file\s*\(|\blibc::unlink(?:at)?\s*\(/

  assert.doesNotMatch(ports, /remove_registered_temporary/)
  assert.doesNotMatch(localMutationProduction, /remove_registered_temporary/)
  assert.doesNotMatch(executor, /remove_registered_temporary/)
  assert.doesNotMatch(localMutationProduction, pathnameDelete)
  assert.doesNotMatch(executor, pathnameDelete)
  assert.match(ports, /pub struct StagedCopy/)
  assert.match(ports, /trait StagedCopyLeasePort/)
  assert.match(executor, /create_staged_copy_cancellable_verified\(/)
  assert.doesNotMatch(executor, /\.create_and_copy_cancellable_verified\(/)
  assert.match(localMutationProduction, /impl StagedCopyLeasePort for MacStagedCopyLease/)
  assert.match(localMutationProduction, /FSUnlinkObject/)
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
    preview: 'vite preview',
  })
  assert.equal(uiPackage.devDependencies['@biomejs/biome'], '2.5.5')
  assert.deepEqual(config.files.includes, ['src/**/*.ts', 'src/**/*.tsx', 'vite.config.ts'])
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

test('repository exposes one onboarding path and recursive macOS hygiene', async () => {
  const [readme, contributing, editorConfig, ignore] = await Promise.all([
    read('README.md'),
    read('CONTRIBUTING.md'),
    read('.editorconfig'),
    read('.gitignore'),
  ])
  assert.match(readme, /pnpm verify:clean/)
  assert.match(readme, /Viewer 0\.1.*development-stage baseline/i)
  assert.match(contributing, /Do not stage unrelated user changes/i)
  assert.match(editorConfig, /root = true/)
  assert.match(editorConfig, /charset = utf-8/)
  assert.match(ignore, /(?:^|\n)\.DS_Store(?:\n|$)/)
  assert.doesNotMatch(ignore, /(?:^|\n)(?:\*\*\/)?\.viewer\/?(?:\n|$)/)
})
