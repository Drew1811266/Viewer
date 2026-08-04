import assert from 'node:assert/strict'
import { mkdir, mkdtemp, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { describe, it } from 'node:test'

import {
  parseProcessTable,
  selectExactViewer,
  selectExactWindow,
  validateEvidencePath,
  validateFixturePath,
  validateWindow,
  validateWindowPoint,
} from './viewer-native-acceptance.mjs'

const repoRoot = '/Users/example/Project/Viewer/.worktrees/atlas'
const executablePath = `${repoRoot}/target/debug/viewer-desktop`

describe('selectExactViewer', () => {
  it('returns the only exact current-worktree bare development process', () => {
    const processes = parseProcessTable(`
      101 1 101 ${executablePath}
    `)

    assert.deepEqual(processes, [
      {
        pid: 101,
        ppid: 1,
        pgid: 101,
        command: executablePath,
      },
    ])
    assert.equal(
      selectExactViewer(processes, {
        repoRoot,
        executablePath,
        controllerPid: 999,
      }).pid,
      101,
    )
  })

  it('rejects a missing Viewer process', () => {
    assert.throws(
      () =>
        selectExactViewer([], {
          repoRoot,
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_COUNT' },
    )
  })

  it('rejects multiple Viewer processes across repository worktrees', () => {
    const processes = parseProcessTable(`
      101 1 101 ${executablePath}
      202 1 202 /Users/example/Project/Viewer/target/debug/viewer-desktop
    `)

    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_COUNT' },
    )
  })

  it('rejects a packaged Viewer process', () => {
    const processes = parseProcessTable(`
      101 1 101 ${repoRoot}/target/debug/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop
    `)

    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_PATH' },
    )
  })

  it('rejects a lone Viewer process from another worktree', () => {
    const processes = parseProcessTable(`
      101 1 101 /Users/example/Project/Viewer/.worktrees/other/target/debug/viewer-desktop
    `)

    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_PATH' },
    )
  })

  it('rejects the controller or helper process as the Viewer target', () => {
    const processes = parseProcessTable(`
      101 1 101 ${executablePath}
    `)

    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 101,
        }),
      { code: 'PRECONDITION_VIEWER_SELF' },
    )
    assert.throws(
      () =>
        selectExactViewer(processes, {
          repoRoot,
          executablePath,
          controllerPid: 999,
          helperPid: 101,
        }),
      { code: 'PRECONDITION_VIEWER_SELF' },
    )
  })
})

describe('window validation', () => {
  const window = {
    pid: 101,
    windowId: 44,
    x: 20,
    y: 30,
    width: 1024,
    height: 720,
  }

  it('accepts one exact owned acceptance window', () => {
    assert.deepEqual(
      selectExactWindow([window], {
        pid: 101,
        viewport: { width: 1024, height: 720 },
      }),
      window,
    )
  })

  it('rejects missing and duplicate main windows', () => {
    assert.throws(
      () =>
        selectExactWindow([], {
          pid: 101,
          viewport: { width: 1024, height: 720 },
        }),
      { code: 'PRECONDITION_WINDOW_COUNT' },
    )
    assert.throws(
      () =>
        selectExactWindow([window, { ...window, windowId: 45 }], {
          pid: 101,
          viewport: { width: 1024, height: 720 },
        }),
      { code: 'PRECONDITION_WINDOW_COUNT' },
    )
  })

  it('rejects a window owned by another process', () => {
    assert.throws(
      () =>
        validateWindow({ ...window, pid: 202 }, {
          pid: 101,
          viewport: { width: 1024, height: 720 },
        }),
      { code: 'PRECONDITION_WINDOW_OWNER' },
    )
  })

  it('rejects a non-positive or non-integer window ID', () => {
    for (const windowId of [0, -1, 44.5, Number.NaN]) {
      assert.throws(
        () =>
          validateWindow({ ...window, windowId }, {
            pid: 101,
            viewport: { width: 1024, height: 720 },
          }),
        { code: 'PRECONDITION_WINDOW_OWNER' },
      )
    }
  })

  it('rejects dimensions outside the two exact acceptance viewports', () => {
    for (const candidate of [
      { ...window, width: 1023 },
      { ...window, width: 1440, height: 899 },
      { ...window, width: 720, height: 450 },
    ]) {
      assert.throws(
        () =>
          validateWindow(candidate, {
            pid: 101,
            viewport: { width: candidate.width, height: candidate.height },
          }),
        { code: 'PRECONDITION_VIEWPORT' },
      )
    }
  })

  it('accepts exact 1440 by 900 dimensions', () => {
    const largeWindow = { ...window, width: 1440, height: 900 }
    assert.equal(
      validateWindow(largeWindow, {
        pid: 101,
        viewport: { width: 1440, height: 900 },
      }),
      largeWindow,
    )
  })
})

describe('coordinate validation', () => {
  const window = {
    pid: 101,
    windowId: 44,
    x: 20,
    y: 30,
    width: 1024,
    height: 720,
  }

  it('translates an in-window relative point to screen coordinates', () => {
    assert.deepEqual(validateWindowPoint({ x: 0, y: 0 }, window), {
      x: 20,
      y: 30,
    })
    assert.deepEqual(validateWindowPoint({ x: 1023, y: 719 }, window), {
      x: 1043,
      y: 749,
    })
  })

  it('rejects non-finite, negative and right-or-bottom-edge points', () => {
    for (const point of [
      { x: -1, y: 0 },
      { x: 0, y: -1 },
      { x: 1024, y: 0 },
      { x: 0, y: 720 },
      { x: Number.NaN, y: 1 },
      { x: 1, y: Number.POSITIVE_INFINITY },
    ]) {
      assert.throws(() => validateWindowPoint(point, window), {
        code: 'SAFETY_POINT_OUTSIDE_WINDOW',
      })
    }
  })
})

describe('fixture path validation', () => {
  it('accepts only the current disposable run root and descendants', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-'))
    const runId = 'run-123'
    const runRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-fixture',
      'runs',
      runId,
    )
    const child = path.join(runRoot, 'project', '衣服', 'A01')

    try {
      await mkdir(child, { recursive: true })
      assert.equal(
        await validateFixturePath(runRoot, { repoRoot: temporaryRoot, runId }),
        runRoot,
      )
      assert.equal(
        await validateFixturePath(child, { repoRoot: temporaryRoot, runId }),
        child,
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('rejects the baseline, repository root, sibling runs and parent escape', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-'))
    const runId = 'run-123'
    const fixtureRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-fixture',
    )
    const runRoot = path.join(fixtureRoot, 'runs', runId)
    const rejected = [
      '/',
      temporaryRoot,
      path.join(fixtureRoot, 'ViewerAcceptance'),
      path.join(fixtureRoot, 'runs', 'run-456'),
      path.join(runRoot, '..', 'run-456'),
      '$HOME/project',
      '~/project',
      path.join(runRoot, '*.jpg'),
    ]

    try {
      await mkdir(path.join(fixtureRoot, 'ViewerAcceptance'), { recursive: true })
      await mkdir(runRoot, { recursive: true })
      await mkdir(path.join(fixtureRoot, 'runs', 'run-456'), { recursive: true })

      for (const candidate of rejected) {
        await assert.rejects(
          validateFixturePath(candidate, { repoRoot: temporaryRoot, runId }),
          { code: 'SAFETY_FIXTURE_PATH' },
        )
      }
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('rejects a symbolic-link escape from the approved run', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-'))
    const outside = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-outside-'))
    const runId = 'run-123'
    const runRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-fixture',
      'runs',
      runId,
    )
    const link = path.join(runRoot, 'escape')

    try {
      await mkdir(runRoot, { recursive: true })
      await writeFile(path.join(outside, 'secret.txt'), 'outside\n')
      await symlink(outside, link)

      await assert.rejects(
        validateFixturePath(path.join(link, 'secret.txt'), {
          repoRoot: temporaryRoot,
          runId,
        }),
        { code: 'SAFETY_FIXTURE_PATH' },
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
      await rm(outside, { recursive: true, force: true })
    }
  })
})

describe('evidence path validation', () => {
  it('accepts only the exact commit, viewport and ledger-ID directory', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-evidence-'))
    const context = {
      repoRoot: temporaryRoot,
      commit: 'abc123',
      viewport: '1024x720',
      id: 'FIL-01',
    }
    const evidenceRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-acceptance',
      'abc123',
      '1024x720',
      'FIL-01',
    )
    const productPath = path.join(evidenceRoot, 'product.png')

    try {
      await mkdir(evidenceRoot, { recursive: true })
      assert.equal(await validateEvidencePath(productPath, context), productPath)

      for (const candidate of [
        path.join(evidenceRoot, '..', 'FIL-02', 'product.png'),
        path.join(evidenceRoot, '..', '..', '1440x900', 'FIL-01', 'product.png'),
        path.join(evidenceRoot, '..', '..', '..', 'def456', '1024x720', 'FIL-01', 'product.png'),
        path.join(temporaryRoot, 'product.png'),
      ]) {
        await assert.rejects(validateEvidencePath(candidate, context), {
          code: 'SAFETY_EVIDENCE_PATH',
        })
      }
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})
