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
  'getrandom',
  'libc',
  'nucleo-matcher',
  'notify',
  'notify-debouncer-full',
  'objc2',
  'objc2-core-foundation',
  'objc2-core-graphics',
  'objc2-foundation',
  'objc2-image-io',
  'objc2-quick-look-thumbnailing',
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
  '@tauri-apps/api',
  '@tauri-apps/cli',
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

  const lockCheck = await stat(new URL('../scripts/check-locked-dependencies.sh', import.meta.url))
  assert.ok((lockCheck.mode & 0o111) !== 0, 'locked dependency check must be executable')
})
