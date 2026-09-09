import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import {
  buildAcceptanceWindowConfig,
  runManagedNativeAcceptance,
} from './run-managed-native-acceptance.mjs'

describe('buildAcceptanceWindowConfig', () => {
  it('creates an inline exact viewport override for managed launches', () => {
    assert.deepEqual(
      JSON.parse(buildAcceptanceWindowConfig(['--preflight', '--viewport', '1024x720'])),
      { app: { windows: [{ width: 1024, height: 720 }] } },
    )
    assert.equal(buildAcceptanceWindowConfig(['--viewport', '800x600']), undefined)
  })
})

describe('runManagedNativeAcceptance', () => {
  it('binds acceptance to the launcher result and tears down only that session', async () => {
    const events = []
    const runtime = {
      async stop(target) {
        events.push(['stop', target])
      },
      async clearSession() {
        events.push(['clear'])
      },
    }
    const result = await runManagedNativeAcceptance(['--preflight', '--viewport', '1024x720'], {
      moduleUrl: 'file:///Users/example/Project/Viewer/scripts/run-managed-native-acceptance.mjs',
      runtimeFactory(paths, { env }) {
        assert.equal(paths.repoRoot, '/Users/example/Project/Viewer')
        assert.deepEqual(JSON.parse(env.VIEWER_TAURI_CONFIG), {
          app: { windows: [{ width: 1024, height: 720 }] },
        })
        return runtime
      },
      async restart({ paths, runtime: receivedRuntime }) {
        assert.equal(paths.repoRoot, '/Users/example/Project/Viewer')
        assert.equal(receivedRuntime, runtime)
        events.push(['restart'])
        return { pid: 700, viewerPid: 701 }
      },
      async runAcceptance(argv, { repoRoot }) {
        events.push(['accept', argv, repoRoot])
        return { mode: 'preflight', exitCode: 0 }
      },
    })

    assert.deepEqual(result, { mode: 'preflight', exitCode: 0 })
    assert.deepEqual(events, [
      ['restart'],
      ['accept', ['--preflight', '--viewport', '1024x720'], '/Users/example/Project/Viewer'],
      ['stop', { kind: 'group', id: 700, viewerPid: 701 }],
      ['clear'],
    ])
  })

  it('tears down the managed session when acceptance fails', async () => {
    const events = []
    const runtime = {
      async stop(target) {
        events.push(['stop', target])
      },
      async clearSession() {
        events.push(['clear'])
      },
    }

    await assert.rejects(
      runManagedNativeAcceptance([], {
        moduleUrl: 'file:///Users/example/Project/Viewer/scripts/run-managed-native-acceptance.mjs',
        runtimeFactory() {
          return runtime
        },
        async restart() {
          return { pid: 800, viewerPid: 801 }
        },
        async runAcceptance() {
          throw new Error('acceptance failed')
        },
      }),
      /acceptance failed/,
    )

    assert.deepEqual(events, [
      ['stop', { kind: 'group', id: 800, viewerPid: 801 }],
      ['clear'],
    ])
  })
})
