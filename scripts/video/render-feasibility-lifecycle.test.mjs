import assert from 'node:assert/strict'
import test from 'node:test'

import {
  appendCleanupFailure,
  cleanupFeasibilityLaunch,
  renderFeasibilityTestPaths,
} from './render-feasibility-lifecycle.mjs'

const executablePath =
  '/worktree/target/debug/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop'

function processInfo(pid, command = executablePath) {
  return { pid, command }
}

test('runs both assertion and lifecycle behavior gates before native work', () => {
  assert.deepEqual(renderFeasibilityTestPaths, [
    'scripts/video/render-feasibility-assertions.test.mjs',
    'scripts/video/render-feasibility-lifecycle.test.mjs',
  ])
})

test('cleans every attributable exact-path process when launch selection is ambiguous', async () => {
  const running = new Set([52, 53])
  const events = []

  await cleanupFeasibilityLaunch(
    {
      baselinePids: new Set([41]),
      client: null,
      executablePath,
      launcher: {},
    },
    {
      currentProcessTable: () => [
        processInfo(41),
        processInfo(44, '/other-worktree/target/debug/viewer-desktop'),
        ...[...running].map((pid) => processInfo(pid)),
      ],
      stopLauncher: async () => events.push('launcher'),
      stopProcess: async (pid) => {
        events.push(`process:${pid}`)
        running.delete(pid)
      },
    },
  )

  assert.deepEqual(events, ['process:52', 'process:53', 'launcher'])
  assert.deepEqual([...running], [])
})

test('rescans after launcher cleanup when initial attribution or ps inspection fails', async () => {
  let scans = 0
  let launcherStopped = false
  const stopped = []

  await cleanupFeasibilityLaunch(
    {
      baselinePids: new Set([41]),
      client: null,
      executablePath,
      launcher: {},
    },
    {
      currentProcessTable: () => {
        scans += 1
        if (scans === 1) throw new Error('ps unavailable')
        return launcherStopped ? [processInfo(77)] : []
      },
      stopLauncher: async () => {
        launcherStopped = true
      },
      stopProcess: async (pid) => stopped.push(pid),
    },
  )

  assert.equal(scans, 2)
  assert.deepEqual(stopped, [77])
})

test('stops a previously attributed exact PID even when every cleanup ps scan fails', async () => {
  const stopped = []

  await assert.rejects(
    cleanupFeasibilityLaunch(
      {
        baselinePids: new Set(),
        client: null,
        executablePath,
        launcher: {},
        pid: 81,
      },
      {
        currentProcessTable: () => {
          throw new Error('ps unavailable')
        },
        stopLauncher: async () => {},
        stopProcess: async (pid) => stopped.push(pid),
      },
    ),
    /Unable to rescan attributable Viewer processes/,
  )

  assert.deepEqual(stopped, [81])
})

test('retries an attributable exact-path process after its first stop attempt fails', async () => {
  let attempts = 0
  let running = true

  await cleanupFeasibilityLaunch(
    {
      baselinePids: new Set(),
      client: null,
      executablePath,
      launcher: {},
    },
    {
      currentProcessTable: () => (running ? [processInfo(82)] : []),
      stopLauncher: async () => {},
      stopProcess: async () => {
        attempts += 1
        if (attempts === 1) throw new Error('transient termination failure')
        running = false
      },
    },
  )

  assert.equal(attempts, 2)
  assert.equal(running, false)
})

test('bounds a stuck helper without blocking Viewer and launcher cleanup', async () => {
  const events = []
  let viewerRunning = true
  const never = new Promise(() => {})

  await assert.rejects(
    cleanupFeasibilityLaunch(
      {
        baselinePids: new Set(),
        client: {
          close: () => never,
          terminate: () => never,
        },
        executablePath,
        launcher: {},
      },
      {
        clientTimeoutMs: 5,
        currentProcessTable: () => (viewerRunning ? [processInfo(88)] : []),
        stopLauncher: async () => events.push('launcher'),
        stopProcess: async (pid) => {
          events.push(`process:${pid}`)
          viewerRunning = false
        },
      },
    ),
    /helper.*within/i,
  )

  assert.deepEqual(events, ['process:88', 'launcher'])
})

test('reports cleanup failure alongside the primary matrix failure', () => {
  const combined = appendCleanupFailure('Error: native assertion failed', new Error('ps failed'))

  assert.match(combined, /native assertion failed/)
  assert.match(combined, /Cleanup failure: Error: ps failed/)
})
