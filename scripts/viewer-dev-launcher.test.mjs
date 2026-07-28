import assert from 'node:assert/strict'
import { copyFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { describe, it } from 'node:test'

import { runCli } from './start-viewer-dev.mjs'
import {
  buildLauncherPaths,
  createSystemRuntime,
  isExactDevelopmentViewerRunning,
  isTauriDevProcess,
  isViewerExecutable,
  parseProcessTable,
  restartDevelopmentViewer,
  selectStopTargets,
  sessionMatchesProcess,
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

describe('createSystemRuntime', () => {
  it('launches, records, observes, logs, and stops a detached process group', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-launcher-'))
    const fakeBin = path.join(temporaryRoot, 'fake-bin')
    const stateDir = path.join(temporaryRoot, 'target', 'dev-launcher')
    const executablePath = path.join(temporaryRoot, 'target', 'debug', 'viewer-desktop')
    const paths = {
      repoRoot: temporaryRoot,
      stateDir,
      statePath: path.join(stateDir, 'session.json'),
      logPath: path.join(stateDir, 'tauri-dev.log'),
      executablePath,
    }
    let child

    try {
      await mkdir(path.join(temporaryRoot, 'src-tauri'), { recursive: true })
      await mkdir(path.dirname(executablePath), { recursive: true })
      await mkdir(fakeBin, { recursive: true })
      await writeFile(path.join(temporaryRoot, 'package.json'), '{}\n')
      await copyFile('/bin/sleep', executablePath)
      await writeFile(
        path.join(fakeBin, 'pnpm'),
        `#!/bin/sh\necho "fake pnpm started"\n"${executablePath}" 30\n`,
        { mode: 0o755 },
      )

      const runtime = createSystemRuntime(paths, {
        env: {
          ...process.env,
          PATH: `${fakeBin}:${process.env.PATH}`,
        },
      })
      child = await runtime.spawn()
      const session = {
        version: 1,
        pid: child.pid,
        pgid: child.pgid,
        repoRoot: temporaryRoot,
        startedAt: '2026-07-28T00:00:00.000Z',
      }
      await runtime.writeSession(session)

      let viewerProcess
      for (let attempt = 0; attempt < 40; attempt += 1) {
        viewerProcess = (await runtime.listProcesses()).find(
          (item) => item.command.split(/\s+/, 1)[0] === executablePath,
        )
        if (viewerProcess) break
        await runtime.sleep(25)
      }

      assert.ok(viewerProcess)
      assert.deepEqual(await runtime.readSession(), session)
      assert.match(await runtime.tailLog(10), /fake pnpm started/)

      await runtime.stop({
        kind: 'group',
        id: child.pgid,
        viewerPid: viewerProcess.pid,
      })
      assert.equal(runtime.isAlive(viewerProcess.pid), false)

      await runtime.clearSession()
      assert.equal(await runtime.readSession(), undefined)
    } finally {
      if (child) {
        try {
          process.kill(-child.pgid, 'SIGKILL')
        } catch (error) {
          if (error?.code !== 'ESRCH') throw error
        }
      }
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('runCli', () => {
  it('reports the verified local development source and executable', async () => {
    const output = []
    const runtime = { name: 'runtime' }
    const result = await runCli({
      moduleUrl: 'file:///Users/example/Project/Viewer/scripts/start-viewer-dev.mjs',
      runtimeFactory(paths) {
        assert.equal(paths.repoRoot, repoRoot)
        return runtime
      },
      async restart(options) {
        assert.equal(options.runtime, runtime)
        return {
          pid: 900,
          executablePath: `${repoRoot}/target/debug/viewer-desktop`,
          logPath: `${repoRoot}/target/dev-launcher/tauri-dev.log`,
        }
      },
      async execGit(args, options) {
        assert.equal(options.cwd, repoRoot)
        const command = args.join(' ')
        if (command === 'rev-parse --abbrev-ref HEAD') return { stdout: 'main\n' }
        if (command === 'rev-parse --short HEAD') return { stdout: 'abc1234\n' }
        if (command === 'status --porcelain') return { stdout: ' M ui/src/App.tsx\n' }
        throw new Error(`Unexpected git command: ${command}`)
      },
      log(line) {
        output.push(line)
      },
    })

    assert.equal(result.pid, 900)
    assert.deepEqual(output, [
      `Viewer development version is running: ${repoRoot}/target/debug/viewer-desktop`,
      'Source: main @ abc1234 (dirty)',
      `Log: ${repoRoot}/target/dev-launcher/tauri-dev.log`,
    ])
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

describe('session safety', () => {
  const session = {
    version: 1,
    pid: 700,
    pgid: 700,
    repoRoot,
    startedAt: '2026-07-28T00:00:00.000Z',
  }

  it('accepts only the recorded repository development process', () => {
    assert.equal(
      sessionMatchesProcess(
        session,
        {
          pid: 700,
          ppid: 1,
          pgid: 700,
          command: 'node /Users/example/.local/bin/pnpm tauri dev',
        },
        repoRoot,
      ),
      true,
    )
    assert.equal(
      sessionMatchesProcess(
        session,
        { pid: 700, ppid: 1, pgid: 700, command: 'sleep 120' },
        repoRoot,
      ),
      false,
    )
    assert.equal(
      sessionMatchesProcess(
        session,
        {
          pid: 700,
          ppid: 1,
          pgid: 700,
          command: 'node /Users/example/.local/bin/pnpm tauri dev',
        },
        '/Users/example/Project/Other',
      ),
      false,
    )
  })
})

describe('restartDevelopmentViewer', () => {
  const paths = {
    repoRoot,
    stateDir: `${repoRoot}/target/dev-launcher`,
    statePath: `${repoRoot}/target/dev-launcher/session.json`,
    logPath: `${repoRoot}/target/dev-launcher/tauri-dev.log`,
    executablePath: `${repoRoot}/target/debug/viewer-desktop`,
  }

  it('stops an existing Viewer before spawning and waits for the exact executable', async () => {
    const events = []
    const snapshots = [
      [
        {
          pid: 121,
          ppid: 120,
          pgid: 120,
          command: `node ${repoRoot}/node_modules/@tauri-apps/cli/tauri.js dev`,
        },
        {
          pid: 122,
          ppid: 121,
          pgid: 120,
          command: `${repoRoot}/target/debug/viewer-desktop`,
        },
      ],
      [],
      [
        {
          pid: 900,
          ppid: 899,
          pgid: 899,
          command: `${repoRoot}/target/debug/viewer-desktop`,
        },
      ],
    ]
    const runtime = {
      async listProcesses() {
        events.push('list')
        return snapshots.shift() ?? []
      },
      async readSession() {
        return undefined
      },
      async writeSession(state) {
        events.push(`write:${state.pid}`)
      },
      async clearSession() {
        events.push('clear')
      },
      async stop(target) {
        events.push(`stop:${target.kind}:${target.id}`)
      },
      async spawn() {
        events.push('spawn')
        return { pid: 899, pgid: 899 }
      },
      isAlive() {
        return true
      },
      async sleep() {},
      async tailLog() {
        return ''
      },
    }

    const result = await restartDevelopmentViewer({
      paths,
      runtime,
      timeoutMs: 100,
      pollMs: 1,
    })

    assert.deepEqual(events, [
      'list',
      'stop:group:120',
      'list',
      'spawn',
      'write:899',
      'list',
    ])
    assert.deepEqual(result, {
      pid: 899,
      executablePath: `${repoRoot}/target/debug/viewer-desktop`,
      logPath: `${repoRoot}/target/dev-launcher/tauri-dev.log`,
    })
  })

  it('clears a stale session without signaling its reused pid', async () => {
    const events = []
    const snapshots = [
      [{ pid: 700, ppid: 1, pgid: 700, command: 'sleep 120' }],
      [],
      [
        {
          pid: 902,
          ppid: 901,
          pgid: 901,
          command: `${repoRoot}/target/debug/viewer-desktop`,
        },
      ],
    ]
    const runtime = {
      async listProcesses() {
        return snapshots.shift() ?? []
      },
      async readSession() {
        return {
          version: 1,
          pid: 700,
          pgid: 700,
          repoRoot,
          startedAt: '2026-07-28T00:00:00.000Z',
        }
      },
      async writeSession() {},
      async clearSession() {
        events.push('clear')
      },
      async stop(target) {
        events.push(`stop:${target.kind}:${target.id}`)
      },
      async spawn() {
        return { pid: 901, pgid: 901 }
      },
      isAlive() {
        return true
      },
      async sleep() {},
      async tailLog() {
        return ''
      },
    }

    await restartDevelopmentViewer({ paths, runtime, timeoutMs: 100, pollMs: 1 })

    assert.deepEqual(events, ['clear'])
  })

  it('cleans state and includes the log tail when startup exits early', async () => {
    const events = []
    let spawned = false
    const runtime = {
      async listProcesses() {
        return []
      },
      async readSession() {
        return undefined
      },
      async writeSession() {},
      async clearSession() {
        events.push('clear')
      },
      async stop(target) {
        events.push(`stop:${target.kind}:${target.id}`)
      },
      async spawn() {
        spawned = true
        return { pid: 901, pgid: 901 }
      },
      isAlive() {
        return !spawned
      },
      async sleep() {},
      async tailLog() {
        return 'compile failed'
      },
    }

    await assert.rejects(
      restartDevelopmentViewer({
        paths,
        runtime,
        timeoutMs: 100,
        pollMs: 1,
      }),
      /compile failed/,
    )
    assert.deepEqual(events, ['clear'])
  })
})
