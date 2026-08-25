import assert from 'node:assert/strict'
import test from 'node:test'

import {
  collectWorkspaceDependencyEdges,
  findForbiddenWorkspaceEdges,
  findWorkspaceDependencyCycles,
  readCargoMetadata,
  runArchitectureBoundariesCli,
} from './architecture-boundaries.mjs'

const metadataWithDependencies = (dependencies) => ({
  workspace_members: ['domain', 'application'],
  packages: [
    {
      id: 'domain',
      name: 'viewer-domain',
      manifest_path: '/workspace/crates/viewer-domain/Cargo.toml',
      dependencies: dependencies.domain ?? [],
    },
    {
      id: 'application',
      name: 'viewer-application',
      manifest_path: '/workspace/crates/viewer-application/Cargo.toml',
      dependencies: dependencies.application ?? [],
    },
  ],
})

test('invokes locked cargo metadata with deterministic arguments', () => {
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

test('collects sorted normal, build, and dev workspace edges only', () => {
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

test('accepts the approved production graph including the media driver', () => {
  const edges = [
    { from: 'viewer-application', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-application', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-video-mpv', kind: 'normal' },
    { from: 'viewer-platform-macos', to: 'viewer-application', kind: 'normal' },
    { from: 'viewer-platform-macos', to: 'viewer-domain', kind: 'build' },
    { from: 'viewer-platform-macos', to: 'viewer-video-mpv', kind: 'normal' },
    { from: 'viewer-desktop', to: 'viewer-platform-macos', kind: 'normal' },
    { from: 'viewer-test-support', to: 'viewer-domain', kind: 'normal' },
  ]

  assert.deepEqual(findForbiddenWorkspaceEdges(edges), [])
  assert.deepEqual(findWorkspaceDependencyCycles(edges), [])
})

test('rejects reversals and production cycles while ignoring dev-only cycles', () => {
  const forbidden = { from: 'viewer-domain', to: 'viewer-application', kind: 'normal' }
  const cycle = [
    { from: 'viewer-application', to: 'viewer-domain', kind: 'normal' },
    { from: 'viewer-domain', to: 'viewer-application', kind: 'normal' },
  ]

  assert.deepEqual(findForbiddenWorkspaceEdges([forbidden]), [forbidden])
  assert.deepEqual(findWorkspaceDependencyCycles(cycle), [
    ['viewer-application', 'viewer-domain', 'viewer-application'],
  ])
  assert.deepEqual(findWorkspaceDependencyCycles([
    { from: 'viewer-domain', to: 'viewer-application', kind: 'dev' },
  ]), [])
})

test('forbids unknown Viewer packages and targets instead of treating them as escape hatches', () => {
  const unknownSource = {
    from: 'viewer-new-adapter',
    to: 'viewer-domain',
    kind: 'normal',
  }
  const unknownTarget = {
    from: 'viewer-desktop',
    to: 'viewer-new-adapter',
    kind: 'build',
  }

  assert.deepEqual(
    findForbiddenWorkspaceEdges([unknownSource, unknownTarget]),
    [unknownTarget, unknownSource],
  )
})

test('canonicalizes and sorts reported production cycles', () => {
  const edges = [
    { from: 'viewer-video-mpv', to: 'viewer-infrastructure', kind: 'normal' },
    { from: 'viewer-infrastructure', to: 'viewer-video-mpv', kind: 'normal' },
    { from: 'viewer-domain', to: 'viewer-application', kind: 'normal' },
    { from: 'viewer-application', to: 'viewer-domain', kind: 'normal' },
  ]

  assert.deepEqual(findWorkspaceDependencyCycles(edges), [
    ['viewer-application', 'viewer-domain', 'viewer-application'],
    ['viewer-infrastructure', 'viewer-video-mpv', 'viewer-infrastructure'],
  ])
})

test('CLI blocks forbidden dependencies and cycles with stable diagnostics', () => {
  const output = []
  const errors = []
  const result = runArchitectureBoundariesCli({
    root: '/workspace/viewer',
    readMetadata: () => metadataWithDependencies({
      domain: [{
        name: 'viewer-application',
        kind: null,
        path: '/workspace/crates/viewer-application',
        source: null,
      }],
      application: [{
        name: 'viewer-domain',
        kind: null,
        path: '/workspace/crates/viewer-domain',
        source: null,
      }],
    }),
    stdout: { write: (value) => output.push(value) },
    stderr: { write: (value) => errors.push(value) },
  })

  assert.equal(result, 1)
  assert.deepEqual(output, [])
  assert.deepEqual(errors, [
    'ERROR: forbidden workspace dependency: viewer-domain -> viewer-application (normal)\n',
    'ERROR: workspace dependency cycle: viewer-application -> viewer-domain -> viewer-application\n',
  ])
})

test('CLI reports success for an allowed acyclic graph', () => {
  const output = []
  const errors = []
  const result = runArchitectureBoundariesCli({
    root: '/workspace/viewer',
    readMetadata: () => metadataWithDependencies({
      application: [{
        name: 'viewer-domain',
        kind: null,
        path: '/workspace/crates/viewer-domain',
        source: null,
      }],
    }),
    stdout: { write: (value) => output.push(value) },
    stderr: { write: (value) => errors.push(value) },
  })

  assert.equal(result, 0)
  assert.deepEqual(errors, [])
  assert.deepEqual(output, ['Architecture boundary check passed.\n'])
})
