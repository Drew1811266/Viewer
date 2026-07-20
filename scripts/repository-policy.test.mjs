import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, stat } from 'node:fs/promises'
import test from 'node:test'

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), 'utf8')
const normalizeNewlines = (text) => text.replace(/\r\n?/g, '\n')

const expectedWorkflow = `name: CI

on:
  push:
  pull_request:

permissions:
  contents: read

jobs:
  verify:
    name: Foundation verification
    runs-on: macos-15
    timeout-minutes: 30

    steps:
      - name: Check out repository
        uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7

      - name: Assert Apple Silicon runner
        shell: bash
        run: test "$(uname -m)" = "arm64"

      - name: Set up Node.js 24.18.0
        uses: actions/setup-node@820762786026740c76f36085b0efc47a31fe5020 # v7
        with:
          node-version: "24.18.0"

      - name: Activate pnpm 10.0.0 through Corepack
        shell: bash
        run: |
          corepack enable
          corepack prepare pnpm@10.0.0 --activate
          pnpm --version

      - name: Install Rust 1.97.0 for Apple Silicon
        shell: bash
        run: |
          rustup toolchain install 1.97.0 --profile minimal --component clippy,rustfmt --target aarch64-apple-darwin
          rustc --version --verbose

      - name: Install JavaScript dependencies
        run: pnpm install --frozen-lockfile

      - name: Run foundation verification
        run: pnpm verify

      - name: Install cargo-deny 0.20.2
        run: cargo install cargo-deny --version 0.20.2 --locked

      - name: Enforce dependency policy
        run: cargo deny --locked check
`

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
const expectedVerify =
  'node --test scripts/repository-policy.test.mjs && pnpm --dir ui test && pnpm --dir ui build && cargo fmt --check && cargo clippy --locked --workspace --all-targets -- -D warnings && cargo test --locked --workspace'

test('repository verification inputs are exact and locked', async () => {
  const [workflow, toolchain, packageText] = await Promise.all([
    read('.github/workflows/ci.yml'),
    read('rust-toolchain.toml'),
    read('package.json'),
  ])
  const packageJson = JSON.parse(packageText)

  assert.equal(normalizeNewlines(workflow), expectedWorkflow)
  assert.equal(normalizeNewlines(toolchain), expectedToolchain)
  assert.equal(packageJson.packageManager, expectedPackageManager)
  assert.equal(packageJson.scripts.verify, expectedVerify)
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

  assert.match(adapter, /mouseLocationOutsideOfEventStream/)
  assert.match(
    adapter,
    /mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure/,
  )
  assert.match(adapter, /contentView\(\)/)
  assert.doesNotMatch(adapter, /filter\(\|event\| event\.r#type\(\) == NSEventType::LeftMouseDragged\)/)
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
