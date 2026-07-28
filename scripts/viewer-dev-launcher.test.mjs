import assert from 'node:assert/strict'
import { describe, it } from 'node:test'

import {
  buildLauncherPaths,
  isExactDevelopmentViewerRunning,
  isTauriDevProcess,
  isViewerExecutable,
  parseProcessTable,
  selectStopTargets,
} from './viewer-dev-launcher.mjs'

const repoRoot = '/Users/example/Project/Viewer'

describe('buildLauncherPaths', () => {
  it('derives repository and runtime paths from the module URL', () => {
    const paths = buildLauncherPaths(
      'file:///Users/example/Project/Viewer/scripts/start-viewer-dev.mjs',
    )

    assert.equal(paths.repoRoot, repoRoot)
    assert.equal(paths.stateDir, `${repoRoot}/target/dev-launcher`)
    assert.equal(paths.statePath, `${repoRoot}/target/dev-launcher/session.json`)
    assert.equal(paths.logPath, `${repoRoot}/target/dev-launcher/tauri-dev.log`)
    assert.equal(paths.executablePath, `${repoRoot}/target/debug/viewer-desktop`)
  })
})

describe('process selection', () => {
  const table = `
  120 1 120 node /Users/example/.local/bin/pnpm tauri dev
  121 120 120 node /Users/example/Project/Viewer/node_modules/@tauri-apps/cli/tauri.js dev
  122 121 120 /Users/example/Project/Viewer/target/debug/viewer-desktop
  210 1 210 node /Users/example/.local/bin/pnpm tauri dev
  211 210 210 node /Users/example/Project/Viewer/.worktrees/theme/node_modules/@tauri-apps/cli/tauri.js dev
  212 211 210 /Users/example/Project/Viewer/.worktrees/theme/target/debug/viewer-desktop
  220 1 220 /Users/example/Project/Viewer/.worktrees/orphan/target/debug/viewer-desktop
  320 1 320 /Applications/Viewer.app/Contents/MacOS/viewer-desktop
  420 1 420 /tmp/another-product/viewer-desktop
  520 1 520 vite --port 5173
  `

  it('parses pid, parent, group, and complete command', () => {
    assert.deepEqual(parseProcessTable(table)[0], {
      pid: 120,
      ppid: 1,
      pgid: 120,
      command: 'node /Users/example/.local/bin/pnpm tauri dev',
    })
  })

  it('recognizes only eligible Viewer executables', () => {
    assert.equal(
      isViewerExecutable(`${repoRoot}/target/debug/viewer-desktop`, repoRoot),
      true,
    )
    assert.equal(
      isViewerExecutable(
        `${repoRoot}/.worktrees/theme/target/debug/viewer-desktop`,
        repoRoot,
      ),
      true,
    )
    assert.equal(
      isViewerExecutable('/Applications/Viewer.app/Contents/MacOS/viewer-desktop', repoRoot),
      true,
    )
    assert.equal(isViewerExecutable('/tmp/another-product/viewer-desktop', repoRoot), false)
  })

  it('recognizes main and worktree Tauri development commands', () => {
    assert.equal(
      isTauriDevProcess(
        `node ${repoRoot}/node_modules/@tauri-apps/cli/tauri.js dev`,
        repoRoot,
      ),
      true,
    )
    assert.equal(
      isTauriDevProcess(
        `node ${repoRoot}/.worktrees/theme/node_modules/@tauri-apps/cli/tauri.js dev`,
        repoRoot,
      ),
      true,
    )
    assert.equal(isTauriDevProcess('vite --port 5173', repoRoot), false)
  })

  it('uses development process groups and exact standalone processes', () => {
    assert.deepEqual(selectStopTargets(parseProcessTable(table), repoRoot, 999), [
      { kind: 'group', id: 120, viewerPid: 122 },
      { kind: 'group', id: 210, viewerPid: 212 },
      { kind: 'process', id: 220, viewerPid: 220 },
      { kind: 'process', id: 320, viewerPid: 320 },
    ])
  })

  it("never selects the current launcher's process group", () => {
    const processes = parseProcessTable(table)
    processes.push({
      pid: 999,
      ppid: 1,
      pgid: 120,
      command: `node ${repoRoot}/scripts/start-viewer-dev.mjs`,
    })

    assert.deepEqual(selectStopTargets(processes, repoRoot, 999), [
      { kind: 'process', id: 122, viewerPid: 122 },
      { kind: 'group', id: 210, viewerPid: 212 },
      { kind: 'process', id: 220, viewerPid: 220 },
      { kind: 'process', id: 320, viewerPid: 320 },
    ])
  })

  it('requires the exact current repository executable for readiness', () => {
    const processes = parseProcessTable(table)
    assert.equal(
      isExactDevelopmentViewerRunning(
        processes,
        `${repoRoot}/target/debug/viewer-desktop`,
      ),
      true,
    )
    assert.equal(
      isExactDevelopmentViewerRunning(
        processes,
        `${repoRoot}/other/target/debug/viewer-desktop`,
      ),
      false,
    )
  })
})
