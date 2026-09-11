import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import {
  access,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { describe, it } from 'node:test'

import { runCli } from './start-viewer-dev.mjs'
import {
  buildLauncherPaths,
  createSystemRuntime,
  inspectDevelopmentViewers,
  isExactDevelopmentViewerRunning,
  isRepositoryViewerExecutable,
  isTauriDevProcess,
  isViewerExecutable,
  parseProcessTable,
  restartDevelopmentViewer,
  selectStopTargets,
  selectStaleViteTargets,
  sessionMatchesProcess,
  isScopedViteProcess,
} from './viewer-dev-launcher.mjs'

const repoRoot = '/Users/example/Project/Viewer'

async function createVideoRuntimePrerequisite(temporaryRoot) {
  const sourcePath = path.join(
    temporaryRoot,
    'target',
    'viewer-video-runtime',
    'universal-apple-darwin',
    'ViewerVideoRuntime',
  )
  const verifierPath = path.join(temporaryRoot, 'verify-runtime.sh')

  await mkdir(path.join(sourcePath, 'bin'), { recursive: true })
  await writeFile(path.join(sourcePath, 'bin', 'ffmpeg'), 'reviewed runtime\n', {
    mode: 0o755,
  })
  await writeFile(path.join(sourcePath, 'runtime.inventory.sha256'), 'fixture inventory\n')
  await writeFile(path.join(sourcePath, 'runtime.lock.json'), '{"fixture":true}\n')
  await writeFile(
    verifierPath,
    [
      '#!/bin/sh',
      'set -eu',
      'test -x "$VIEWER_VIDEO_STAGE_DIR/bin/ffmpeg"',
      'test -f "$VIEWER_VIDEO_STAGE_DIR/runtime.inventory.sha256"',
      '',
    ].join('\n'),
    { mode: 0o755 },
  )

  return {
    videoRuntimeSourcePath: sourcePath,
    videoRuntimeDestinationPath: path.join(
      temporaryRoot,
      'target',
      'debug',
      'ViewerVideoRuntime',
    ),
    videoRuntimeVerifierPath: verifierPath,
  }
}

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
    assert.equal(
      paths.legacyWrapperPath,
      `${repoRoot}/target/dev-launcher/current-dev-wrapper`,
    )
    assert.equal(
      paths.videoRuntimeSourcePath,
      `${repoRoot}/target/viewer-video-runtime/universal-apple-darwin/ViewerVideoRuntime`,
    )
    assert.equal(
      paths.videoRuntimeDestinationPath,
      `${repoRoot}/target/debug/ViewerVideoRuntime`,
    )
    assert.equal(
      paths.videoRuntimeVerifierPath,
      `${repoRoot}/scripts/video/verify-runtime.sh`,
    )
  })
})

describe('createSystemRuntime', () => {
  it('materializes a verified video runtime before spawning from a clean target', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-launcher-runtime-'))
    const fakeBin = path.join(temporaryRoot, 'fake-bin')
    const stateDir = path.join(temporaryRoot, 'target', 'dev-launcher')
    const sourcePath = path.join(
      temporaryRoot,
      'target',
      'viewer-video-runtime',
      'universal-apple-darwin',
      'ViewerVideoRuntime',
    )
    const destinationPath = path.join(temporaryRoot, 'target', 'debug', 'ViewerVideoRuntime')
    const verifierPath = path.join(temporaryRoot, 'verify-runtime.sh')
    const verifierLogPath = path.join(temporaryRoot, 'verifier.log')
    const paths = {
      repoRoot: temporaryRoot,
      stateDir,
      statePath: path.join(stateDir, 'session.json'),
      logPath: path.join(stateDir, 'tauri-dev.log'),
      executablePath: path.join(temporaryRoot, 'target', 'debug', 'viewer-desktop'),
      legacyWrapperPath: path.join(stateDir, 'current-dev-wrapper'),
      videoRuntimeSourcePath: sourcePath,
      videoRuntimeDestinationPath: destinationPath,
      videoRuntimeVerifierPath: verifierPath,
    }
    let child

    try {
      await mkdir(path.join(temporaryRoot, 'src-tauri'), { recursive: true })
      await mkdir(path.join(sourcePath, 'bin'), { recursive: true })
      await mkdir(fakeBin, { recursive: true })
      await writeFile(path.join(temporaryRoot, 'package.json'), '{}\n')
      await writeFile(path.join(sourcePath, 'bin', 'ffmpeg'), 'reviewed runtime\n', {
        mode: 0o755,
      })
      await writeFile(path.join(sourcePath, 'runtime.inventory.sha256'), 'fixture inventory\n')
      await writeFile(path.join(sourcePath, 'runtime.lock.json'), '{"fixture":true}\n')
      await writeFile(
        verifierPath,
        [
          '#!/bin/sh',
          'set -eu',
          'test -x "$VIEWER_VIDEO_STAGE_DIR/bin/ffmpeg"',
          'test -f "$VIEWER_VIDEO_STAGE_DIR/runtime.inventory.sha256"',
          'printf \'%s\\n\' "$VIEWER_VIDEO_STAGE_DIR" >> "$VIEWER_TEST_VERIFIER_LOG"',
          '',
        ].join('\n'),
        { mode: 0o755 },
      )
      await writeFile(
        path.join(fakeBin, 'pnpm'),
        '#!/bin/sh\nexec /bin/sleep 30\n',
        { mode: 0o755 },
      )

      const runtime = createSystemRuntime(paths, {
        env: {
          ...process.env,
          PATH: `${fakeBin}:${process.env.PATH}`,
          VIEWER_TEST_VERIFIER_LOG: verifierLogPath,
        },
      })
      child = await runtime.spawn()

      assert.equal(
        await readFile(path.join(destinationPath, 'bin', 'ffmpeg'), 'utf8'),
        'reviewed runtime\n',
      )
      assert.notEqual((await stat(path.join(destinationPath, 'bin', 'ffmpeg'))).mode & 0o111, 0)

      const verifiedPaths = (await readFile(verifierLogPath, 'utf8')).trim().split('\n')
      assert.equal(verifiedPaths.length, 2)
      assert.equal(verifiedPaths[0], sourcePath)
      assert.notEqual(verifiedPaths[1], destinationPath)
    } finally {
      if (child) {
        await createSystemRuntime(paths).stop({
          kind: 'group',
          id: child.pgid,
          viewerPid: child.pid,
        })
      }
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('passes an acceptance viewport config through the canonical dev launcher', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-launcher-config-'))
    const videoRuntimePaths = await createVideoRuntimePrerequisite(temporaryRoot)
    const fakeBin = path.join(temporaryRoot, 'fake-bin')
    const argsPath = path.join(temporaryRoot, 'spawn-args.txt')
    const configPath = path.join(temporaryRoot, 'target', 'acceptance-1440.json')
    const stateDir = path.join(temporaryRoot, 'target', 'dev-launcher')
    const paths = {
      repoRoot: temporaryRoot,
      stateDir,
      statePath: path.join(stateDir, 'session.json'),
      logPath: path.join(stateDir, 'tauri-dev.log'),
      executablePath: path.join(temporaryRoot, 'target', 'debug', 'viewer-desktop'),
      legacyWrapperPath: path.join(stateDir, 'current-dev-wrapper'),
      ...videoRuntimePaths,
    }
    let child

    try {
      await mkdir(path.join(temporaryRoot, 'src-tauri'), { recursive: true })
      await mkdir(path.dirname(configPath), { recursive: true })
      await mkdir(fakeBin, { recursive: true })
      await writeFile(path.join(temporaryRoot, 'package.json'), '{}\n')
      await writeFile(configPath, '{}\n')
      await writeFile(
        path.join(fakeBin, 'pnpm'),
        '#!/bin/sh\nprintf \'%s\\n\' "$@" > "$VIEWER_TEST_ARGS_PATH"\nexec /bin/sleep 30\n',
        { mode: 0o755 },
      )

      const runtime = createSystemRuntime(paths, {
        env: {
          ...process.env,
          PATH: `${fakeBin}:${process.env.PATH}`,
          VIEWER_TAURI_CONFIG: configPath,
          VIEWER_TEST_ARGS_PATH: argsPath,
        },
      })
      child = await runtime.spawn()

      let args
      // The launcher first verifies/materializes the development runtime and
      // then starts a detached process. On a busy CI host the fake child may
      // take longer than one second to reach exec; keep the assertion about
      // the exact argv while allowing that bounded startup jitter.
      for (let attempt = 0; attempt < 160; attempt += 1) {
        try {
          args = await readFile(argsPath, 'utf8')
          break
        } catch (error) {
          if (error?.code !== 'ENOENT') throw error
          await runtime.sleep(25)
        }
      }

      assert.equal(args, `tauri\ndev\n--config\n${configPath}\n`)
    } finally {
      if (child) {
        await createSystemRuntime(paths).stop({
          kind: 'group',
          id: child.pgid,
          viewerPid: child.pid,
        })
      }
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('launches, records, observes, logs, and stops a detached process group', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-launcher-'))
    const videoRuntimePaths = await createVideoRuntimePrerequisite(temporaryRoot)
    const fakeBin = path.join(temporaryRoot, 'fake-bin')
    const stateDir = path.join(temporaryRoot, 'target', 'dev-launcher')
    const executablePath = path.join(temporaryRoot, 'target', 'debug', 'viewer-desktop')
    const legacyWrapperPath = path.join(stateDir, 'current-dev-wrapper')
    const preservedPath = path.join(stateDir, 'preserved.txt')
    const paths = {
      repoRoot: temporaryRoot,
      stateDir,
      statePath: path.join(stateDir, 'session.json'),
      logPath: path.join(stateDir, 'tauri-dev.log'),
      executablePath,
      legacyWrapperPath,
      ...videoRuntimePaths,
    }
    let child

    try {
      await mkdir(path.join(temporaryRoot, 'src-tauri'), { recursive: true })
      await mkdir(path.dirname(executablePath), { recursive: true })
      await mkdir(fakeBin, { recursive: true })
      await mkdir(path.join(legacyWrapperPath, 'Viewer.app'), { recursive: true })
      await writeFile(path.join(temporaryRoot, 'package.json'), '{}\n')
      await writeFile(path.join(legacyWrapperPath, 'Viewer.app', 'sentinel'), 'legacy\n')
      await writeFile(preservedPath, 'keep\n')
      await copyFile('/bin/sleep', executablePath)
      // macOS 26 kills copies of system-vault binaries (SIGKILL, exit 137).
      // Re-sign ad hoc so the fake viewer executable is allowed to run.
      if (process.platform === 'darwin') {
        execFileSync('codesign', ['--force', '--sign', '-', executablePath], { stdio: 'ignore' })
      }
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
      await runtime.removeLegacyWrapper()
      await assert.rejects(access(legacyWrapperPath), { code: 'ENOENT' })
      assert.equal(await readFile(preservedPath, 'utf8'), 'keep\n')
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
          viewerPid: 901,
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
      'Viewer process: 901',
      'Source: main @ abc1234 (dirty)',
      `Log: ${repoRoot}/target/dev-launcher/tauri-dev.log`,
    ])
  })
})

describe('package command', () => {
  it('exposes the canonical Viewer development launcher', async () => {
    const packageJson = JSON.parse(
      await readFile(new URL('../package.json', import.meta.url), 'utf8'),
    )

    assert.equal(
      packageJson.scripts['start:viewer'],
      'node scripts/start-viewer-dev.mjs',
    )
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
    assert.equal(
      isViewerExecutable(
        `${repoRoot}/target/debug/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop`,
        repoRoot,
      ),
      true,
    )
    assert.equal(isViewerExecutable('/tmp/another-product/viewer-desktop', repoRoot), false)
  })

  it('allows only repository development Viewers into the stop list', () => {
    assert.equal(
      isRepositoryViewerExecutable(`${repoRoot}/target/debug/viewer-desktop`, repoRoot),
      true,
    )
    assert.equal(
      isRepositoryViewerExecutable(
        '/Applications/Viewer.app/Contents/MacOS/viewer-desktop',
        repoRoot,
      ),
      false,
    )
    assert.equal(
      isRepositoryViewerExecutable(
        `${repoRoot}/target/debug/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop`,
        repoRoot,
      ),
      false,
    )
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

  it('recognizes only repository-scoped Vite executables', () => {
    assert.equal(
      isScopedViteProcess(
        `${repoRoot}/ui/node_modules/.bin/vite`,
        repoRoot,
      ),
      true,
    )
    assert.equal(
      isScopedViteProcess(
        `${repoRoot}/.worktrees/theme/ui/node_modules/vite/bin/vite.js`,
        repoRoot,
      ),
      true,
    )
    assert.equal(
      isScopedViteProcess(
        `node ${repoRoot}/ui/node_modules/vite/bin/vite.js --host localhost`,
        repoRoot,
      ),
      true,
    )
    assert.equal(isScopedViteProcess('vite --port 5173', repoRoot), false)
    assert.equal(isScopedViteProcess('/tmp/vite/bin/vite.js', repoRoot), false)
  })

  it('selects orphaned scoped Vite processes without touching unrelated servers', () => {
    const processes = parseProcessTable(`
      520 1 520 ${repoRoot}/ui/node_modules/.bin/vite --host localhost --port 5173
      521 520 520 ${repoRoot}/node_modules/@tauri-apps/cli/tauri.js dev
      522 521 520 ${repoRoot}/target/debug/viewer-desktop
      620 1 620 /tmp/vite/bin/vite.js --port 5173
    `)
    assert.deepEqual(selectStaleViteTargets(processes, repoRoot), [
      { kind: 'process', id: 520, viewerPid: 520 },
    ])
  })

  it('keeps the main repository and sibling worktrees in scope from a worktree', () => {
    const worktreeRoot = `${repoRoot}/.worktrees/theme`

    assert.equal(
      isViewerExecutable(`${repoRoot}/target/debug/viewer-desktop`, worktreeRoot),
      true,
    )
    assert.equal(
      isViewerExecutable(
        `${repoRoot}/.worktrees/other/target/debug/viewer-desktop`,
        worktreeRoot,
      ),
      true,
    )
    assert.equal(
      isTauriDevProcess(
        `node ${repoRoot}/node_modules/@tauri-apps/cli/tauri.js dev`,
        worktreeRoot,
      ),
      true,
    )
    assert.deepEqual(selectStopTargets(parseProcessTable(table), worktreeRoot, 999), [
      { kind: 'group', id: 120, viewerPid: 122 },
      { kind: 'group', id: 210, viewerPid: 212 },
      { kind: 'process', id: 220, viewerPid: 220 },
    ])
  })

  it('uses development process groups and exact standalone processes', () => {
    assert.deepEqual(selectStopTargets(parseProcessTable(table), repoRoot, 999), [
      { kind: 'group', id: 120, viewerPid: 122 },
      { kind: 'group', id: 210, viewerPid: 212 },
      { kind: 'process', id: 220, viewerPid: 220 },
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

  it('separates the exact development Viewer from every eligible Viewer', () => {
    const observation = inspectDevelopmentViewers(
      parseProcessTable(table),
      repoRoot,
      `${repoRoot}/target/debug/viewer-desktop`,
    )

    assert.deepEqual(observation.exact.map(({ pid }) => pid), [122])
    assert.deepEqual(observation.eligible.map(({ pid }) => pid), [122, 212, 220, 320])
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
    legacyWrapperPath: `${repoRoot}/target/dev-launcher/current-dev-wrapper`,
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
      async removeLegacyWrapper() {
        events.push('remove-wrapper')
      },
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
      'remove-wrapper',
      'list',
      'stop:group:120',
      'list',
      'spawn',
      'write:899',
      'list',
    ])
    assert.deepEqual(result, {
      pid: 899,
      viewerPid: 900,
      executablePath: `${repoRoot}/target/debug/viewer-desktop`,
      logPath: `${repoRoot}/target/dev-launcher/tauri-dev.log`,
    })
  })

  it('never terminates a packaged Viewer and refuses to start beside it', async () => {
    const events = []
    const packaged = {
      pid: 320,
      ppid: 1,
      pgid: 320,
      command: '/Applications/Viewer.app/Contents/MacOS/viewer-desktop',
    }
    const runtime = {
      async removeLegacyWrapper() {},
      async listProcesses() {
        return [packaged]
      },
      async readSession() {
        return undefined
      },
      async writeSession() {
        events.push('write')
      },
      async clearSession() {
        events.push('clear')
      },
      async stop(target) {
        events.push(`stop:${target.kind}:${target.id}`)
      },
      async spawn() {
        events.push('spawn')
        return { pid: 901, pgid: 901 }
      },
    }

    await assert.rejects(
      restartDevelopmentViewer({ paths, runtime, timeoutMs: 100, pollMs: 1 }),
      /Existing Viewer process did not stop/,
    )
    assert.deepEqual(events, [])
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
      async removeLegacyWrapper() {},
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
      async removeLegacyWrapper() {},
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

  it('does not inspect or spawn when legacy wrapper cleanup fails', async () => {
    const events = []
    const runtime = {
      async removeLegacyWrapper() {
        events.push('remove-wrapper')
        throw new Error('wrapper cleanup denied')
      },
      async listProcesses() {
        events.push('list')
        return []
      },
    }

    await assert.rejects(
      restartDevelopmentViewer({ paths, runtime, timeoutMs: 100, pollMs: 1 }),
      /wrapper cleanup denied/,
    )
    assert.deepEqual(events, ['remove-wrapper'])
  })

  it('stops the spawned group and clears state when two Viewers appear', async () => {
    const events = []
    const exactCommand = `${repoRoot}/target/debug/viewer-desktop`
    const snapshots = [
      [],
      [],
      [
        { pid: 910, ppid: 909, pgid: 909, command: exactCommand },
        { pid: 911, ppid: 1, pgid: 911, command: exactCommand },
      ],
    ]
    const runtime = {
      async removeLegacyWrapper() {
        events.push('remove-wrapper')
      },
      async listProcesses() {
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
        return { pid: 909, pgid: 909 }
      },
      isAlive() {
        return true
      },
      async sleep() {},
      async tailLog() {
        return ''
      },
    }

    await assert.rejects(
      restartDevelopmentViewer({ paths, runtime, timeoutMs: 100, pollMs: 1 }),
      /exact=2, eligible=2/,
    )
    assert.deepEqual(events, [
      'remove-wrapper',
      'write:909',
      'stop:group:909',
      'clear',
    ])
  })

  it('stops the spawned group when another eligible Viewer appears', async () => {
    const events = []
    const snapshots = [
      [],
      [],
      [
        {
          pid: 920,
          ppid: 919,
          pgid: 919,
          command: `${repoRoot}/target/debug/viewer-desktop`,
        },
        {
          pid: 921,
          ppid: 1,
          pgid: 921,
          command: '/Applications/Viewer.app/Contents/MacOS/viewer-desktop',
        },
      ],
    ]
    const runtime = {
      async removeLegacyWrapper() {
        events.push('remove-wrapper')
      },
      async listProcesses() {
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
        return { pid: 919, pgid: 919 }
      },
      isAlive() {
        return true
      },
      async sleep() {},
      async tailLog() {
        return ''
      },
    }

    await assert.rejects(
      restartDevelopmentViewer({ paths, runtime, timeoutMs: 100, pollMs: 1 }),
      /exact=1, eligible=2/,
    )
    assert.deepEqual(events, [
      'remove-wrapper',
      'write:919',
      'stop:group:919',
      'clear',
    ])
  })

  it('stops an orphaned current-worktree Vite process before spawning', async () => {
    const events = []
    const snapshots = [
      [
        {
          pid: 950,
          ppid: 1,
          pgid: 950,
          command: `${repoRoot}/ui/node_modules/.bin/vite --host localhost --port 5173`,
        },
      ],
      [],
      [
        {
          pid: 952,
          ppid: 951,
          pgid: 951,
          command: `${repoRoot}/target/debug/viewer-desktop`,
        },
      ],
    ]
    const runtime = {
      async removeLegacyWrapper() {},
      async listProcesses() {
        return snapshots.shift() ?? []
      },
      async readSession() {
        return undefined
      },
      async writeSession() {},
      async clearSession() {},
      async stop(target) {
        events.push(`stop:${target.kind}:${target.id}`)
      },
      async spawn() {
        return { pid: 951, pgid: 951 }
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

    assert.deepEqual(events, ['stop:process:950'])
  })
})
