import assert from 'node:assert/strict'
import { execFile, spawn } from 'node:child_process'
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  stat,
  symlink,
  writeFile,
} from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { describe, it } from 'node:test'
import { promisify } from 'node:util'

import * as nativeAcceptance from './viewer-native-acceptance.mjs'
import {
  AUDIT_IDS,
  ALLOWED_COMMANDS,
  AcceptanceError,
  NativeAcceptanceClient,
  PROTOCOL_VERSION,
  STATE_RECIPES,
  buildEvidenceManifest,
  buildStateEntryPlan,
  buildNativeHelper,
  createFixtureRun,
  combinePngEvidence,
  discoverNativeWindows,
  executeStateEntryPlan,
  parseNativeAcceptanceCli,
  openProjectViaPanel,
  parseProcessTable,
  prepareFixtureForState,
  selectExactViewer,
  selectExactWindow,
  resetFixtureVariant,
  resolveAtlasReferencePath,
  validateCapturePreflight,
  validateCommand,
  validateEvidencePath,
  validateFixturePath,
  validateWindow,
  validateWindowPoint,
  waitFor,
  readRgbaPng,
  writeRgbaPng,
} from './viewer-native-acceptance.mjs'

const repoRoot = '/Users/example/Project/Viewer/.worktrees/atlas'
const executablePath = `${repoRoot}/target/debug/viewer-desktop`
const execFileAsync = promisify(execFile)
const actualRepoRoot = path.resolve(new URL('..', import.meta.url).pathname)

function parseTableIds(markdown) {
  return [
    ...markdown.matchAll(
      /^\| ((?:LAU|SID|STR|THU|OTH|SEA|FIL|MEN|RAD|PRE|COM|DOC|INF|DIA|TAS|RES|A11Y)-\d{2}) \|/gm,
    ),
  ].map((match) => match[1])
}

describe('recipe registry', () => {
  it('is exactly set-equal to both authoritative 89-state documents', async () => {
    const audit = await readFile(
      path.join(
        actualRepoRoot,
        'docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md',
      ),
      'utf8',
    )
    const ledger = await readFile(
      path.join(
        actualRepoRoot,
        'docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md',
      ),
      'utf8',
    )
    const auditIds = parseTableIds(audit)
    const ledgerIds = parseTableIds(ledger)
    const recipeIds = [...STATE_RECIPES.keys()]

    for (const ids of [auditIds, ledgerIds, AUDIT_IDS, recipeIds]) {
      assert.equal(ids.length, 89)
      assert.equal(new Set(ids).size, 89)
    }
    assert.deepEqual(new Set(AUDIT_IDS), new Set(auditIds))
    assert.deepEqual(new Set(AUDIT_IDS), new Set(ledgerIds))
    assert.deepEqual(new Set(AUDIT_IDS), new Set(recipeIds))
  })

  it('materializes a complete, deterministic recipe for every state', () => {
    for (const id of AUDIT_IDS) {
      const recipe = STATE_RECIPES.get(id)
      assert.equal(recipe.id, id)
      assert.match(recipe.fixtureVariant, /^[a-z][a-z0-9-]+$/)
      assert.ok([1, 2, 3, 4].includes(recipe.wave))
      assert.ok(Array.isArray(recipe.steps) && recipe.steps.length > 0)
      assert.deepEqual(
        recipe.steps.map((step) => step.kind),
        ['resetFixture', 'focusWindow', 'executeState', 'waitForVisibleState'],
      )
      assert.equal(recipe.steps[2].id, id)
      assert.ok(
        recipe.steps.every(
          (step) =>
            typeof step.kind === 'string' &&
            step.kind !== 'sleep' &&
            !Object.hasOwn(step, 'viewport'),
        ),
      )
      assert.equal(typeof recipe.visibleAssertion, 'string')
      assert.ok(recipe.visibleAssertion.trim().length > 0)
      assert.ok(Object.isFrozen(recipe))
      assert.ok(Object.isFrozen(recipe.steps))
    }
  })
})

describe('state entry plans', () => {
  it('uses concrete native actions for the first stable workspace states', () => {
    const projectRootPlan = buildStateEntryPlan('STR-01')
    assert.deepEqual(projectRootPlan.filter((step) => step.kind === 'press'), [
      { kind: 'press', target: { role: 'AXCheckBox', name: '测试图' } },
    ])
    assert.deepEqual(projectRootPlan.slice(-4), [
      { kind: 'waitMissing', target: { name: '扫描项目' } },
      { kind: 'waitMissing', target: { name: '正在生成缩略图' } },
      { kind: 'waitMissing', target: { name: '2 个任务已完成' }, stableMs: 1_000 },
      { kind: 'assert', target: { role: 'AXCheckBox', name: '测试图' } },
    ])
    assert.deepEqual(buildStateEntryPlan('SID-01'), [
      { kind: 'openProject' },
      {
        kind: 'normalizeWorkspace',
        density: '标准',
        sidebar: 'expanded',
        sidebarWidth: 220,
      },
      { kind: 'click', target: { role: 'AXGroup', name: '衣服/A01' } },
      { kind: 'waitMissing', target: { name: '扫描项目' } },
      { kind: 'waitMissing', target: { name: '正在生成缩略图' } },
      {
        kind: 'waitMissing',
        target: { name: '2 个任务已完成' },
        stableMs: 1_000,
      },
      { kind: 'assert', target: { role: 'AXGroup', name: '衣服/A01' } },
    ])
    assert.deepEqual(buildStateEntryPlan('STR-03'), [
      { kind: 'openProject' },
      {
        kind: 'normalizeWorkspace',
        density: '标准',
        sidebar: 'expanded',
        sidebarWidth: 220,
      },
      { kind: 'click', target: { role: 'AXGroup', name: '衣服/A01' } },
      { kind: 'waitMissing', target: { name: '扫描项目' } },
      { kind: 'waitMissing', target: { name: '正在生成缩略图' } },
      {
        kind: 'waitMissing',
        target: { name: '2 个任务已完成' },
        stableMs: 1_000,
      },
      { kind: 'assert', target: { role: 'AXGroup', name: '衣服/A01' } },
    ])
    assert.deepEqual(buildStateEntryPlan('FIL-01'), [
      { kind: 'openProject' },
      {
        kind: 'normalizeWorkspace',
        density: '紧凑',
        sidebar: 'expanded',
        sidebarWidth: 220,
      },
      { kind: 'click', target: { role: 'AXGroup', name: '衣服/A01' } },
      { kind: 'waitMissing', target: { name: '扫描项目' } },
      { kind: 'waitMissing', target: { name: '正在生成缩略图' } },
      {
        kind: 'waitMissing',
        target: { name: '2 个任务已完成' },
        stableMs: 1_000,
      },
      { kind: 'click', target: { role: 'AXButton', name: '筛选' } },
      { kind: 'movePointerToTitlebar' },
      { kind: 'assert', target: { role: 'AXHeading', name: '筛选' } },
    ])
    assert.deepEqual(buildStateEntryPlan('MEN-02'), [
      { kind: 'openProject' },
      {
        kind: 'normalizeWorkspace',
        density: '标准',
        sidebar: 'expanded',
        sidebarWidth: 220,
      },
      { kind: 'click', target: { role: 'AXGroup', name: '衣服/A01' } },
      { kind: 'waitMissing', target: { name: '扫描项目' } },
      { kind: 'waitMissing', target: { name: '正在生成缩略图' } },
      {
        kind: 'waitMissing',
        target: { name: '2 个任务已完成' },
        stableMs: 1_000,
      },
      { kind: 'click', target: { role: 'AXButton', name: '更多' } },
      { kind: 'movePointerToTitlebar' },
      { kind: 'assert', target: { name: '软件设置' } },
    ])
  })

  it('asserts thumbnail selection through the stable accessibility label', () => {
    const plan = buildStateEntryPlan('THU-05')

    assert.deepEqual(plan.at(-2), { kind: 'movePointerToTitlebar' })
    assert.deepEqual(plan.at(-1), {
      kind: 'assert',
      target: { role: 'AXGroup', name: '选择摘要' },
    })
  })

  it('matches the atlas three-item multi-selection state', () => {
    const plan = buildStateEntryPlan('THU-06')
    const selectedNames = plan
      .filter((step) => step.kind === 'click' && /^商品-0[1-9]\.jpg$/.test(step.target?.name ?? ''))
      .map((step) => step.target.name)

    assert.deepEqual(selectedNames, ['商品-01.jpg', '商品-02.jpg', '商品-03.jpg'])
  })

  it('keeps the other-file states in the compact A01 content context', () => {
    const collapsed = buildStateEntryPlan('OTH-01')
    const expanded = buildStateEntryPlan('OTH-02')

    for (const plan of [collapsed, expanded]) {
      assert.deepEqual(plan.slice(0, 4), [
        { kind: 'prepareFixture', operation: 'prepareOrganizationDrag' },
        { kind: 'openProject' },
        {
          kind: 'normalizeWorkspace',
          density: '紧凑',
          sidebar: 'expanded',
          sidebarWidth: 220,
        },
        { kind: 'click', target: { role: 'AXGroup', name: '衣服/A01' } },
      ])
      assert.deepEqual(
        plan
          .filter(
            (step) => step.kind === 'click' && /^商品-0[1-9]\.jpg$/.test(step.target?.name ?? ''),
          )
          .map((step) => step.target.name),
        ['商品-01.jpg', '商品-02.jpg', '商品-03.jpg'],
      )
      assert.equal(
        plan.some((step) => step.kind === 'click' && step.target?.name === '其它'),
        false,
      )
    }

    assert.equal(
      collapsed.some(
        (step) => step.kind === 'click' && step.target?.name === '其它文件 · 3',
      ),
      false,
    )
    assert.equal(
      expanded.some(
        (step) => step.kind === 'click' && step.target?.name === '其它文件 · 3',
      ),
      true,
    )
    assert.deepEqual(expanded.at(-1), {
      kind: 'assert',
      target: { name: '交付清单.xlsx' },
    })
  })

  it('builds the advanced-filter visual state from six real conditions', () => {
    const plan = buildStateEntryPlan('FIL-04')
    const advancedIndex = plan.findIndex(
      (step) => step.kind === 'click' && step.target?.name === '高级条件',
    )

    assert.deepEqual(plan.slice(advancedIndex - 4, advancedIndex), [
      { kind: 'click', target: { role: 'AXCheckBox', name: 'JPEG' } },
      { kind: 'click', target: { role: 'AXCheckBox', name: 'Markdown' } },
      { kind: 'click', target: { role: 'AXCheckBox', name: '保留' } },
      { kind: 'click', target: { role: 'AXCheckBox', name: '待定' } },
    ])
    const dateIndex = plan.findIndex((step) => step.target?.name === '最早修改时间')
    assert.deepEqual(plan.slice(dateIndex, dateIndex + 21), [
      { kind: 'focus', target: { name: '最早修改时间', position: 'rightmost' } },
      { kind: 'key', key: '2', modifiers: [] },
      { kind: 'key', key: '0', modifiers: [] },
      { kind: 'key', key: '2', modifiers: [] },
      { kind: 'key', key: '6', modifiers: [] },
      { kind: 'key', key: 'arrowRight', modifiers: [] },
      { kind: 'key', key: '0', modifiers: [] },
      { kind: 'key', key: '1', modifiers: [] },
      { kind: 'key', key: 'arrowRight', modifiers: [] },
      { kind: 'key', key: '0', modifiers: [] },
      { kind: 'key', key: '1', modifiers: [] },
      { kind: 'key', key: 'arrowRight', modifiers: [] },
      { kind: 'key', key: '1', modifiers: [] },
      { kind: 'key', key: '2', modifiers: [] },
      { kind: 'key', key: 'arrowRight', modifiers: [] },
      { kind: 'key', key: '0', modifiers: [] },
      { kind: 'key', key: '0', modifiers: [] },
      { kind: 'key', key: 'arrowRight', modifiers: [] },
      { kind: 'key', key: 'a', modifiers: [] },
      { kind: 'key', key: 'tab', modifiers: [] },
      { kind: 'assert', target: { role: 'AXButton', name: '筛选，6 项已启用' } },
    ])
    const countIndex = plan.findIndex((step) => step.target?.name === '筛选，6 项已启用')
    assert.deepEqual(plan.slice(countIndex, countIndex + 4), [
      { kind: 'assert', target: { role: 'AXButton', name: '筛选，6 项已启用' } },
      { kind: 'click', target: { role: 'AXButton', name: '完成高级条件编辑' } },
      { kind: 'movePointerToTitlebar' },
      { kind: 'assert', target: { namePrefix: '像素宽度 · 至少 1200 px' } },
    ])
  })

  it('waits for the completion task to stay absent before capture', () => {
    const plan = buildStateEntryPlan('SID-01')

    assert.deepEqual(plan.slice(3, 6), [
      { kind: 'waitMissing', target: { name: '扫描项目' } },
      { kind: 'waitMissing', target: { name: '正在生成缩略图' } },
      {
        kind: 'waitMissing',
        target: { name: '2 个任务已完成' },
        stableMs: 1_000,
      },
    ])
  })

  it('holds the native organization pointer over the approved sidebar destination', () => {
    const plan = buildStateEntryPlan('SID-04')
    const holdIndex = plan.findIndex((step) => step.kind === 'holdOrganizationDrag')

    assert.notEqual(holdIndex, -1)
    assert.deepEqual(plan.slice(holdIndex - 2), [
      { kind: 'click', target: { name: '商品-01.jpg' } },
      { kind: 'click', target: { name: '商品-02.jpg' }, modifiers: ['command'] },
      {
        kind: 'holdOrganizationDrag',
        source: { name: '商品-02.jpg' },
        destination: { role: 'AXGroup', name: '目标/Destination' },
        modifiers: [],
      },
    ])
    assert.equal(plan.at(-1).kind, 'holdOrganizationDrag')
    assert.equal(plan.some((step) => step.kind === 'sleep'), false)
  })

  it('matches the compact three-item organization drag reference state', () => {
    const plan = buildStateEntryPlan('OTH-03')
    const holdIndex = plan.findIndex((step) => step.kind === 'holdOrganizationDrag')

    assert.deepEqual(plan.slice(0, 3), [
      { kind: 'prepareFixture', operation: 'prepareOrganizationDrag' },
      { kind: 'openProject' },
      {
      kind: 'normalizeWorkspace',
      density: '紧凑',
      sidebar: 'expanded',
      sidebarWidth: 220,
      },
    ])
    assert.deepEqual(plan.slice(holdIndex - 4), [
      { kind: 'click', target: { role: 'AXButton', name: '其它文件 · 3' } },
      { kind: 'click', target: { name: '商品-01.jpg' } },
      { kind: 'click', target: { name: '商品-02.jpg' }, modifiers: ['command'] },
      { kind: 'click', target: { name: '商品-03.jpg' }, modifiers: ['command'] },
      {
        kind: 'holdOrganizationDrag',
        source: { name: '商品-03.jpg' },
        destination: { role: 'AXGroup', name: '目标/Destination' },
        modifiers: [],
      },
    ])
    assert.equal(plan.at(-1).kind, 'holdOrganizationDrag')
    assert.equal(plan.some((step) => step.kind === 'sleep'), false)
  })

  it('normalizes persistent workspace chrome before entering every stable Wave 1 state', () => {
    const standardDensityIds = [
      'SID-01',
      'SID-02',
      'SID-03',
      'STR-01',
      'STR-02',
      'STR-03',
      'STR-05',
      'THU-02',
      'THU-04',
      'THU-05',
      'THU-06',
      'THU-07',
      'SEA-01',
      'SEA-02',
      'SEA-04',
      'SEA-05',
      'MEN-01',
      'MEN-02',
      'LAU-07',
    ]
    const compactDensityIds = [
      'STR-04',
      'THU-01',
      'OTH-01',
      'OTH-02',
      'OTH-03',
      'FIL-01',
      'FIL-02',
      'FIL-03',
      'FIL-04',
      'MEN-03',
    ]
    const largeDensityIds = ['THU-03']

    for (const [density, ids] of [
      ['标准', standardDensityIds],
      ['紧凑', compactDensityIds],
      ['大图', largeDensityIds],
    ]) {
      for (const id of ids) {
        const plan = buildStateEntryPlan(id)
        const openIndex = plan.findIndex((step) => step.kind === 'openProject')
        assert.notEqual(openIndex, -1, id)
        assert.deepEqual(
          plan[openIndex + 1],
          {
            kind: 'normalizeWorkspace',
            density,
            sidebar: 'expanded',
            sidebarWidth: 220,
          },
          id,
        )
      }
    }
  })

  it('waits for the search control and proves the live indexing state for SEA-03', () => {
    const plan = buildStateEntryPlan('SEA-03')
    const setValueIndex = plan.findIndex((step) => step.kind === 'setValue')

    assert.deepEqual(plan.slice(0, 3), [
      { kind: 'prepareFixture', operation: 'populateSearchIndexing' },
      { kind: 'openProject' },
      {
        kind: 'normalizeWorkspace',
        density: null,
        sidebar: 'expanded',
        sidebarWidth: 220,
      },
    ])
    assert.deepEqual(plan[setValueIndex - 1], {
      kind: 'assert',
      target: { role: 'AXTextField', name: '搜索项目' },
    })
    assert.deepEqual(plan.at(-1), {
      kind: 'assert',
      target: { namePrefix: '结果仍在更新' },
    })
  })

  it('enters every non-drag launch state through a real disposable project transition', () => {
    assert.deepEqual(buildStateEntryPlan('LAU-04'), [
      { kind: 'prepareFixture', operation: 'seedOpeningRecoveryLoad' },
      { kind: 'beginOpenProject' },
      { kind: 'assert', target: { role: 'AXProgressIndicator', name: '正在打开项目' } },
    ])
    assert.deepEqual(buildStateEntryPlan('LAU-05'), [
      { kind: 'prepareFixture', operation: 'populateSearchIndexing' },
      { kind: 'beginOpenProject' },
      {
        kind: 'assert',
        target: { role: 'AXStaticText', name: '测试图', position: 'rightmost' },
      },
    ])
    assert.deepEqual(buildStateEntryPlan('LAU-06'), [
      { kind: 'prepareFixture', operation: 'populateSearchIndexing' },
      { kind: 'beginOpenProject' },
      { kind: 'click', target: { role: 'AXGroup', name: '衣服/A01' } },
      { kind: 'assert', target: { role: 'AXStaticText', name: '正在生成缩略图' } },
      { kind: 'captureCheckpoint' },
    ])
    assert.deepEqual(buildStateEntryPlan('LAU-08'), [
      { kind: 'prepareFixture', operation: 'corruptViewerMetadata' },
      { kind: 'beginOpenProject' },
      { kind: 'assert', target: { role: 'AXHeading', name: '无法打开项目' } },
    ])
    assert.deepEqual(buildStateEntryPlan('LAU-09'), [
      { kind: 'prepareFixture', operation: 'seedRecoveryJournal' },
      { kind: 'openProject' },
      { kind: 'waitMissing', target: { name: '扫描项目' } },
      { kind: 'waitMissing', target: { name: '正在生成缩略图' } },
      {
        kind: 'waitMissing',
        target: { name: '2 个任务已完成' },
        stableMs: 1_000,
      },
      { kind: 'click', target: { role: 'AXGroup', name: '衣服/A01' } },
      { kind: 'assert', target: { role: 'AXHeading', name: '项目状态已恢复' } },
    ])
  })

  it('uses real Finder drag plans for the empty-project drop states', () => {
    assert.deepEqual(buildStateEntryPlan('LAU-02'), [
      { kind: 'ensureNoProject' },
      {
        kind: 'holdFinderDrag',
        source: '.',
        durationMs: 700,
      },
    ])
    assert.deepEqual(buildStateEntryPlan('LAU-03'), [
      { kind: 'ensureNoProject' },
      {
        kind: 'finderDrop',
        source: '衣服/A01/商品-01.jpg',
        durationMs: 700,
      },
      { kind: 'assert', target: { role: 'AXStaticText', name: '请选择一个文件夹' } },
    ])
  })

  it('rejects a state until it has a real executable entry plan', () => {
    assert.throws(() => buildStateEntryPlan('DIA-01'), {
      code: 'STATE_RECIPE_EXECUTOR',
    })
  })

  it('uses real radial pointer and keyboard entry plans for every round-menu state', () => {
    assert.deepEqual(
      buildStateEntryPlan('RAD-01').filter((step) =>
        ['contextClick', 'assert'].includes(step.kind),
      ),
      [
        { kind: 'contextClick', target: { name: '商品-01.jpg' } },
        { kind: 'assert', target: { role: 'AXMenu', name: '文件操作' } },
      ],
    )
    assert.deepEqual(
      buildStateEntryPlan('RAD-02').filter((step) =>
        ['holdRadialGesture', 'assert'].includes(step.kind),
      ),
      [
        {
          kind: 'holdRadialGesture',
          target: { name: '商品-01.jpg' },
          delta: { x: 0, y: -88 },
        },
        { kind: 'assert', target: { role: 'AXMenu', name: '文件操作' } },
      ],
    )
    assert.equal(
      buildStateEntryPlan('RAD-07').some(
        (step) =>
          step.kind === 'key' && step.key === 'f10' && step.modifiers?.includes('shift'),
      ),
      true,
    )
  })

  it('opens image, comparison, document and information states through product actions', () => {
    const previewPlan = buildStateEntryPlan('PRE-01')
    assert.deepEqual(
      previewPlan.filter(
        (step) =>
          (step.kind === 'contextClick' && step.target?.name === '商品-02.jpg') ||
          (step.kind === 'press' && step.target?.name === '预览'),
      ),
      [
        { kind: 'contextClick', target: { name: '商品-02.jpg' } },
        { kind: 'press', target: { role: 'AXMenuItem', name: '预览' } },
      ],
    )
    assert.deepEqual(
      buildStateEntryPlan('PRE-03').filter((step) => step.kind === 'click').slice(-2),
      [
        { kind: 'click', target: { role: 'AXButton', name: '放大' } },
        { kind: 'click', target: { role: 'AXButton', name: '放大' } },
      ],
    )
    assert.equal(
      buildStateEntryPlan('COM-04').filter(
        (step) => step.kind === 'click' && /^商品-\d{2}\.jpg$/.test(step.target?.name ?? ''),
      ).length,
      20,
    )
    assert.equal(
      buildStateEntryPlan('DOC-05').filter(
        (step) => step.kind === 'click' && ['sample.md', 'plain.txt'].includes(step.target?.name),
      ).length,
      2,
    )
    assert.equal(
      buildStateEntryPlan('INF-01').some(
        (step) => step.kind === 'key' && step.key === 'i' && step.modifiers?.includes('command'),
      ),
      true,
    )
  })

  it('covers every Wave 2 product state without recipe sleeps', () => {
    const ids = AUDIT_IDS.filter((id) => STATE_RECIPES.get(id).wave === 2)
    assert.equal(ids.length, 27)
    for (const id of ids) {
      const plan = buildStateEntryPlan(id)
      assert.ok(plan.length > 0, id)
      assert.equal(plan.some((step) => step.kind === 'sleep'), false, id)
      assert.ok(['assert', 'captureCheckpoint'].includes(plan.at(-1).kind), id)
    }
  })

  it('covers every stable Wave 1 product state without recipe sleeps', () => {
    const ids = [
      'LAU-07',
      'SID-02',
      'SID-03',
      'STR-04',
      'STR-05',
      'THU-01',
      'THU-02',
      'THU-03',
      'THU-06',
      'THU-07',
      'OTH-01',
      'OTH-02',
      'SEA-01',
      'SEA-02',
      'SEA-03',
      'SEA-04',
      'SEA-05',
      'FIL-02',
      'FIL-03',
      'FIL-04',
      'MEN-01',
      'MEN-03',
    ]

    for (const id of ids) {
      const plan = buildStateEntryPlan(id)
      assert.ok(plan.length > 0, id)
      assert.equal(plan.some((step) => step.kind === 'sleep'), false, id)
      assert.equal(plan.at(-1).kind, 'assert', id)
    }
  })

  it('executes plan steps through logged native requests', async () => {
    const commands = []
    const client = {
      async request(command, payload) {
        commands.push({ command, payload })
        if (command === 'query') {
          if (
            ['扫描项目', '2 个任务已完成', '正在生成缩略图'].includes(payload.target.name)
          ) {
            throw new AcceptanceError('STATE_TARGET_NOT_FOUND', 'not found')
          }
          return {
            elements: [
              {
                role: 'AXGroup',
                name: payload.target.name,
                frame: { x: 120, y: 90, width: 80, height: 24 },
              },
            ],
          }
        }
        return { performed: true, command }
      },
    }
    const actions = []
    let opened = false

    const result = await executeStateEntryPlan({
      id: 'STR-03',
      client,
      actions,
      projectPath: '/Users/example/ViewerAcceptanceRuns/run/测试图',
      window: { x: 100, y: 70, width: 1024, height: 720 },
      openProject: async () => {
        opened = true
      },
    })

    assert.equal(opened, true)
    assert.equal(result.passed, true)
    assert.deepEqual(commands[0], {
      command: 'query',
      payload: { target: { role: 'AXButton', name: '折叠文件夹栏' } },
    })
    assert.deepEqual(
      commands.find(({ command }) => command === 'drag')?.payload.to,
      { x: 220, y: 32 },
    )
    for (const name of ['更多', '软件设置', '标准', '关闭', '衣服/A01']) {
      assert.ok(
        commands.some(
          ({ command, payload }) => command === 'query' && payload.target.name === name,
        ),
        name,
      )
    }
    assert.ok(commands.filter(({ command }) => command === 'pointer').length >= 5)
    assert.ok(actions.length >= 12)
  })
  it('keeps an organization drag held through capture and releases it exactly once', async () => {
    const commands = []
    const client = {
      async request(command, payload) {
        commands.push({ command, payload })
        if (command === 'query') {
          if (
            ['扫描项目', '2 个任务已完成', '正在生成缩略图'].includes(payload.target.name)
          ) {
            throw new AcceptanceError('STATE_TARGET_NOT_FOUND', 'not found')
          }
          return {
            elements: [
              {
                role: payload.target.role ?? 'AXGroup',
                name: payload.target.name,
                frame: payload.target.name === '目标/Destination'
                  ? { x: 180, y: 450, width: 120, height: 28 }
                  : { x: 520, y: 250, width: 80, height: 24 },
              },
            ],
          }
        }
        return { performed: true, command }
      },
    }

    const result = await executeStateEntryPlan({
      id: 'SID-04',
      client,
      actions: [],
      projectPath: '/Users/example/ViewerAcceptanceRuns/run/测试图',
      window: { x: 100, y: 70, width: 1024, height: 720 },
      openProject: async () => {},
      observeHeldPointer: async () => {
        commands.push({ command: 'observeHeldPointer', payload: {} })
      },
    })

    assert.deepEqual(
      commands
        .filter(({ command, payload }) =>
          command === 'pointer' && ['leftDown', 'leftDrag', 'leftUp'].includes(payload.kind),
        )
        .map(({ payload }) => payload),
      [
        { kind: 'leftDown', point: { x: 480, y: 200 }, modifiers: [] },
        { kind: 'leftDrag', point: { x: 140, y: 394 }, modifiers: [] },
      ],
    )
    assert.ok(
      commands.findIndex(
        ({ command, payload }) => command === 'pointer' && payload.kind === 'leftDrag',
      ) <
        commands.findIndex(({ command }) => command === 'observeHeldPointer'),
    )
    assert.equal(commands.some(({ command }) => command === 'observeHeldPointer'), true)

    await result.releasePointer()
    await result.releasePointer()

    assert.deepEqual(
      commands
        .filter(({ command, payload }) => command === 'pointer' && payload.kind === 'leftUp')
        .map(({ payload }) => payload),
      [{ kind: 'leftUp', point: { x: 140, y: 394 } }],
    )
  })

  it('keeps an external Finder drag held through capture and releases it once', async () => {
    const commands = []
    const client = {
      async request(command, payload) {
        commands.push({ command, payload })
        return { performed: true, command }
      },
    }
    const projectPath = path.join(
      os.homedir(),
      'ViewerAcceptanceRuns',
      'run-123',
      '测试图',
    )

    const result = await executeStateEntryPlan({
      id: 'LAU-02',
      client,
      actions: [],
      projectPath,
      window: { x: 100, y: 70, width: 1024, height: 720 },
      ensureNoProject: async () => ({ visible: true }),
      observeHeldPointer: async () => {
        commands.push({ command: 'observeHeldPointer', payload: {} })
      },
    })

    assert.deepEqual(commands[0], {
      command: 'finderDrag',
      payload: {
        path: projectPath,
        destination: { x: 512, y: 360 },
        release: false,
        durationMs: 700,
      },
    })
    assert.equal(commands[1].command, 'observeHeldPointer')

    await result.releasePointer()
    await result.releasePointer()

    assert.deepEqual(commands.at(-1), {
      command: 'pointer',
      payload: { kind: 'leftUp', point: { x: 512, y: 360 } },
    })
    assert.equal(
      commands.filter(
        ({ command, payload }) => command === 'pointer' && payload.kind === 'leftUp',
      ).length,
      1,
    )
  })

  it('keeps a radial secondary-button gesture held through capture and releases it once', async () => {
    const commands = []
    const client = {
      async request(command, payload) {
        commands.push({ command, payload })
        if (command === 'query') {
          if (['扫描项目', '2 个任务已完成', '正在生成缩略图'].includes(payload.target.name)) {
            throw new AcceptanceError('STATE_TARGET_NOT_FOUND', 'not found')
          }
          return {
            elements: [
              {
                role: payload.target.role ?? 'AXGroup',
                name: payload.target.name,
                frame: { x: 520, y: 250, width: 80, height: 24 },
              },
            ],
          }
        }
        return { performed: true, command }
      },
    }

    const result = await executeStateEntryPlan({
      id: 'RAD-02',
      client,
      actions: [],
      projectPath: '/Users/example/ViewerAcceptanceRuns/run/测试图',
      window: { x: 100, y: 70, width: 1024, height: 720 },
      openProject: async () => {},
      observeHeldPointer: async () => {
        commands.push({ command: 'observeHeldPointer', payload: {} })
      },
    })

    assert.deepEqual(
      commands
        .filter(
          ({ command, payload }) =>
            command === 'pointer' && ['rightDown', 'rightDrag', 'rightUp'].includes(payload.kind),
        )
        .map(({ payload }) => payload),
      [
        { kind: 'rightDown', point: { x: 460, y: 192 } },
        { kind: 'rightDrag', point: { x: 460, y: 104 } },
      ],
    )
    assert.equal(commands.some(({ command }) => command === 'observeHeldPointer'), true)

    await result.releasePointer()
    await result.releasePointer()

    assert.deepEqual(
      commands
        .filter(({ command, payload }) => command === 'pointer' && payload.kind === 'rightUp')
        .map(({ payload }) => payload),
      [{ kind: 'rightUp', point: { x: 460, y: 104 } }],
    )
  })

  it('releases a held organization pointer when held-state observation fails', async () => {
    const commands = []
    const client = {
      async request(command, payload) {
        commands.push({ command, payload })
        if (command === 'query') {
          if (
            ['扫描项目', '2 个任务已完成', '正在生成缩略图'].includes(payload.target.name)
          ) {
            throw new AcceptanceError('STATE_TARGET_NOT_FOUND', 'not found')
          }
          return {
            elements: [
              {
                role: payload.target.role ?? 'AXGroup',
                name: payload.target.name,
                frame: payload.target.name === '目标/Destination'
                  ? { x: 180, y: 450, width: 120, height: 28 }
                  : { x: 520, y: 250, width: 80, height: 24 },
              },
            ],
          }
        }
        return { performed: true, command }
      },
    }

    await assert.rejects(
      executeStateEntryPlan({
        id: 'SID-04',
        client,
        actions: [],
        projectPath: '/Users/example/ViewerAcceptanceRuns/run/测试图',
        window: { x: 100, y: 70, width: 1024, height: 720 },
        openProject: async () => {},
        observeHeldPointer: async () => {
          throw new AcceptanceError('BROKEN_ASSERTION', 'held-state capture failed')
        },
      }),
      { code: 'BROKEN_ASSERTION' },
    )

    assert.equal(
      commands.filter(({ command, payload }) => command === 'pointer' && payload.kind === 'leftUp')
        .length,
      1,
    )
  })
})

async function createFixtureBaseline(repoDirectory) {
  const baseline = path.join(
    repoDirectory,
    'target/atlas-product-migration-fixture/ViewerAcceptance',
  )
  await mkdir(path.join(baseline, '衣服/A01'), { recursive: true })
  await mkdir(path.join(baseline, '文档'), { recursive: true })
  await mkdir(path.join(baseline, '空目录/Empty'), { recursive: true })
  await mkdir(path.join(baseline, '目标/Source'), { recursive: true })
  await mkdir(path.join(baseline, '目标/Destination'), { recursive: true })
  await writeFile(path.join(baseline, '衣服/A01/image.jpg'), 'jpeg-data')
  await writeFile(path.join(baseline, '文档/sample.md'), '# fixture')
  await writeFile(path.join(baseline, 'corrupt.jpg'), 'not-a-jpeg')
  await writeFile(path.join(baseline, '目标/Source/same.txt'), 'source')
  await writeFile(path.join(baseline, '目标/Destination/same.txt'), 'destination')
  return baseline
}

describe('fixture run', () => {
  it('copies a manifest-bound baseline without mutating the source', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-fixture-run-'))
    const runId = `acceptance-001-${process.pid}-${Date.now()}`
    const expectedRunRoot = path.join(
      os.homedir(),
      'ViewerAcceptanceRuns',
      runId,
    )
    try {
      const baseline = await createFixtureBaseline(temporaryRoot)
      const baselineBefore = await readFile(path.join(baseline, '文档/sample.md'))
      const run = await createFixtureRun({
        repoRoot: temporaryRoot,
        runId,
      })
      const manifest = JSON.parse(await readFile(run.manifestPath, 'utf8'))

      assert.equal(run.runRoot, expectedRunRoot)
      assert.equal(run.variantsRoot, expectedRunRoot)
      assert.equal(manifest.schemaVersion, 1)
      assert.equal(manifest.runId, runId)
      assert.ok(manifest.files.some((file) => file.path === '文档/sample.md'))
      assert.ok(
        manifest.files.every(
          (file) =>
            Number.isInteger(file.size) && /^[a-f0-9]{64}$/.test(file.sha256),
        ),
      )
      assert.deepEqual(await readFile(path.join(baseline, '文档/sample.md')), baselineBefore)
      await assert.rejects(
        createFixtureRun({ repoRoot: temporaryRoot, runId }),
        { code: 'FIXTURE_RUN_EXISTS' },
      )
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('restores a named variant deterministically from its private snapshot', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-fixture-reset-'))
    const runId = `acceptance-002-${process.pid}-${Date.now()}`
    const expectedRunRoot = path.join(
      os.homedir(),
      'ViewerAcceptanceRuns',
      runId,
    )
    try {
      await createFixtureBaseline(temporaryRoot)
      const run = await createFixtureRun({
        repoRoot: temporaryRoot,
        runId,
      })
      const first = await resetFixtureVariant(run, 'search-results')
      assert.equal(path.basename(first), '测试图')
      assert.equal(path.basename(path.dirname(first)), 'search-results')
      await rm(path.join(first, '衣服/A01/image.jpg'))
      await writeFile(path.join(first, '文档/sample.md'), 'mutated')

      const restored = await resetFixtureVariant(run, 'search-results')
      assert.equal(restored, first)
      assert.equal(await readFile(path.join(restored, '衣服/A01/image.jpg'), 'utf8'), 'jpeg-data')
      assert.equal(await readFile(path.join(restored, '文档/sample.md'), 'utf8'), '# fixture')
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('prepares a bounded compact organization-drag fixture without mutating the baseline', async () => {
    const baseline = path.join(
      actualRepoRoot,
      'target/atlas-product-migration-fixture/ViewerAcceptance',
    )
    const baselineFiles = await readdir(path.join(baseline, '衣服/A01'))
    const runId = `acceptance-organization-${process.pid}-${Date.now()}`
    const expectedRunRoot = path.join(os.homedir(), 'ViewerAcceptanceRuns', runId)
    try {
      const run = await createFixtureRun({ repoRoot: actualRepoRoot, runId })
      const projectPath = await resetFixtureVariant(run, 'other-files')

      await prepareFixtureForState(projectPath, 'prepareOrganizationDrag')

      const files = await readdir(path.join(projectPath, '衣服/A01'))
      assert.deepEqual(
        files.filter((name) => /\.(?:jpe?g|png)$/i.test(name)).sort(),
        Array.from({ length: 8 }, (_, index) => `商品-${String(index + 1).padStart(2, '0')}.jpg`),
      )
      assert.deepEqual(
        files.filter((name) => !/\.(?:jpe?g|png)$/i.test(name)).sort(),
        ['产品说明.md', '交付清单.xlsx', '色卡.txt'].sort(),
      )
      await assert.rejects(stat(path.join(projectPath, '.viewer')), { code: 'ENOENT' })
      assert.deepEqual(await readdir(path.join(baseline, '衣服/A01')), baselineFiles)
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
    }
  })

  it('builds a disposable long-running search-indexing fixture', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-fixture-indexing-'))
    const runId = `acceptance-indexing-${process.pid}-${Date.now()}`
    const expectedRunRoot = path.join(os.homedir(), 'ViewerAcceptanceRuns', runId)
    try {
      await createFixtureBaseline(temporaryRoot)
      const run = await createFixtureRun({ repoRoot: temporaryRoot, runId })
      const projectPath = await resetFixtureVariant(run, 'search-results')
      await mkdir(path.join(projectPath, '.viewer'))

      await prepareFixtureForState(projectPath, 'populateSearchIndexing')

      const generated = await readdir(path.join(projectPath, '搜索索引中'))
      assert.equal(generated.length, 1_200)
      await assert.rejects(stat(path.join(projectPath, '.viewer')), { code: 'ENOENT' })
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('corrupts only the disposable metadata database for the open-error state', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-fixture-corrupt-'))
    const runId = `acceptance-corrupt-${process.pid}-${Date.now()}`
    const expectedRunRoot = path.join(os.homedir(), 'ViewerAcceptanceRuns', runId)
    try {
      await createFixtureBaseline(temporaryRoot)
      const run = await createFixtureRun({ repoRoot: temporaryRoot, runId })
      const projectPath = await resetFixtureVariant(run, 'launch-error')
      await mkdir(path.join(projectPath, '.viewer'))
      await writeFile(path.join(projectPath, '.viewer/metadata.sqlite'), 'sqlite baseline')

      await prepareFixtureForState(projectPath, 'corruptViewerMetadata')

      assert.equal(
        await readFile(path.join(projectPath, '.viewer/metadata.sqlite'), 'utf8'),
        'not-a-viewer-sqlite-database',
      )
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('seeds one bounded real recovery obligation in the disposable journal', async () => {
    const baseline = path.join(
      actualRepoRoot,
      'target/atlas-product-migration-fixture/ViewerAcceptance',
    )
    const runId = `acceptance-recovery-${process.pid}-${Date.now()}`
    const expectedRunRoot = path.join(os.homedir(), 'ViewerAcceptanceRuns', runId)
    try {
      const baselinePlainText = await readFile(path.join(baseline, '文档/plain.txt'))
      const run = await createFixtureRun({ repoRoot: actualRepoRoot, runId })
      const projectPath = await resetFixtureVariant(run, 'launch-recovery')

      await prepareFixtureForState(projectPath, 'seedRecoveryJournal')

      const database = path.join(projectPath, '.viewer/metadata.sqlite')
      const { stdout } = await execFileAsync('/usr/bin/sqlite3', [
        database,
        "SELECT b.kind || '|' || b.state || '|' || i.kind || '|' || i.state || '|' || i.temporary_path FROM operation_batches b JOIN operation_items i USING(batch_id) WHERE b.created_at_ms = 1;",
      ])
      assert.match(stdout.trim(), /^copy\|running\|copy\|prepared\|目标\/Destination\/\.viewer-copy-[0-9a-f-]+\.part$/)
      assert.deepEqual(await readFile(path.join(baseline, '文档/plain.txt')), baselinePlainText)
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
    }
  })

  it('seeds a bounded recovery workload long enough to capture real project validation', async () => {
    const runId = `acceptance-opening-${process.pid}-${Date.now()}`
    const expectedRunRoot = path.join(os.homedir(), 'ViewerAcceptanceRuns', runId)
    try {
      const run = await createFixtureRun({ repoRoot: actualRepoRoot, runId })
      const projectPath = await resetFixtureVariant(run, 'entry-and-loading')

      await prepareFixtureForState(projectPath, 'seedOpeningRecoveryLoad')

      const database = path.join(projectPath, '.viewer/metadata.sqlite')
      const { stdout } = await execFileAsync('/usr/bin/sqlite3', [
        database,
        "SELECT COUNT(*) FROM operation_items WHERE updated_at_ms = 2 AND state = 'prepared';",
      ])
      assert.equal(stdout.trim(), '400')
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
    }
  })

  it('rejects a symbolic link anywhere in the source baseline', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-fixture-link-'))
    const runId = `acceptance-003-${process.pid}-${Date.now()}`
    const expectedRunRoot = path.join(
      os.homedir(),
      'ViewerAcceptanceRuns',
      runId,
    )
    try {
      const baseline = await createFixtureBaseline(temporaryRoot)
      await symlink('/tmp', path.join(baseline, 'escape'))
      await assert.rejects(
        createFixtureRun({ repoRoot: temporaryRoot, runId }),
        { code: 'FIXTURE_SYMLINK' },
      )
      await assert.rejects(
        stat(expectedRunRoot),
        { code: 'ENOENT' },
      )
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('removes only the exact completed home-scoped run', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-fixture-cleanup-'))
    const runId = `cleanup-${process.pid}-${Date.now()}`
    const siblingId = `cleanup-sibling-${process.pid}-${Date.now()}`
    const runsRoot = path.join(os.homedir(), 'ViewerAcceptanceRuns')
    const expectedRunRoot = path.join(runsRoot, runId)
    const siblingRoot = path.join(runsRoot, siblingId)
    try {
      await createFixtureBaseline(temporaryRoot)
      const run = await createFixtureRun({ repoRoot: temporaryRoot, runId })
      await mkdir(siblingRoot, { recursive: true })
      await writeFile(path.join(siblingRoot, 'keep.txt'), 'keep')
      const acceptance = await import('./viewer-native-acceptance.mjs')

      assert.equal(typeof acceptance.removeFixtureRun, 'function')
      await acceptance.removeFixtureRun(run)

      await assert.rejects(stat(expectedRunRoot), { code: 'ENOENT' })
      assert.equal(await readFile(path.join(siblingRoot, 'keep.txt'), 'utf8'), 'keep')
    } finally {
      await rm(expectedRunRoot, { recursive: true, force: true })
      await rm(siblingRoot, { recursive: true, force: true })
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('evidence manifest', () => {
  it('records every provenance, action and joint-comparison field', () => {
    const manifest = buildEvidenceManifest({
      id: 'FIL-03',
      wave: 1,
      commit: 'a'.repeat(40),
      branch: 'codex/viewer-atlas-product-migration',
      dirty: false,
      pid: 4859,
      executablePath: `${actualRepoRoot}/target/debug/viewer-desktop`,
      window: { windowId: 194, x: 223, y: 69, width: 1024, height: 720 },
      viewport: '1024x720',
      fixture: {
        runId: 'run-001',
        variant: 'filter-controls',
        path: `${actualRepoRoot}/target/atlas-product-migration-fixture/runs/run-001/variants/filter-controls`,
      },
      actions: [
        { sequence: 1, command: 'activate', target: '筛选', ok: true },
        { sequence: 2, command: 'activate', target: 'JPEG', ok: true },
      ],
      assertion: {
        description: 'Four selected filter chips are visible in the formal popover.',
        passed: true,
      },
      evidence: {
        raw: { path: '/evidence/raw.png', sha256: '1'.repeat(64) },
        reference: { path: '/evidence/reference.png', sha256: '2'.repeat(64) },
        combined: { path: '/evidence/combined.png', sha256: '3'.repeat(64) },
      },
      timestamp: '2026-08-03T10:00:00.000Z',
      verdict: 'pass',
    })

    assert.deepEqual(Object.keys(manifest), [
      'schemaVersion',
      'id',
      'wave',
      'commit',
      'branch',
      'dirty',
      'process',
      'window',
      'viewport',
      'fixture',
      'actions',
      'assertion',
      'evidence',
      'timestamp',
      'verdict',
    ])
    assert.deepEqual(manifest.process, {
      pid: 4859,
      executablePath: `${actualRepoRoot}/target/debug/viewer-desktop`,
    })
    assert.deepEqual(manifest.evidence, {
      raw: { path: '/evidence/raw.png', sha256: '1'.repeat(64) },
      reference: { path: '/evidence/reference.png', sha256: '2'.repeat(64) },
      combined: { path: '/evidence/combined.png', sha256: '3'.repeat(64) },
    })
    assert.ok(Object.isFrozen(manifest))
  })

  it('rejects incomplete, dirty or unproved evidence', () => {
    assert.throws(
      () => buildEvidenceManifest({ id: 'FIL-03' }),
      { code: 'EVIDENCE_MANIFEST_INVALID' },
    )
  })
})

describe('joint PNG evidence', () => {
  it('places reference and native state in one lossless comparison input', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-combined-png-'))
    try {
      const reference = path.join(temporaryRoot, 'reference.png')
      const native = path.join(temporaryRoot, 'native.png')
      const combined = path.join(temporaryRoot, 'combined.png')
      await writeRgbaPng(reference, {
        width: 2,
        height: 1,
        data: Buffer.from([255, 0, 0, 255, 255, 0, 0, 255]),
      })
      await writeRgbaPng(native, {
        width: 2,
        height: 1,
        data: Buffer.from([0, 0, 255, 255, 0, 0, 255, 255]),
      })

      await combinePngEvidence({ reference, native, output: combined })
      const image = await readRgbaPng(combined)
      assert.equal(image.width, 4)
      assert.equal(image.height, 1)
      assert.deepEqual(
        image.data,
        Buffer.from([
          255, 0, 0, 255,
          255, 0, 0, 255,
          0, 0, 255, 255,
          0, 0, 255, 255,
        ]),
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('atlas reference evidence', () => {
  it('binds a reference image to the exact atlas hash, viewport and state', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-atlas-reference-'))
    try {
      const atlasPath = path.join(
        temporaryRoot,
        'docs/prototypes/viewer-complete-ui-visual-atlas.html',
      )
      await mkdir(path.dirname(atlasPath), { recursive: true })
      await writeFile(atlasPath, '<!doctype html><title>approved atlas</title>')
      const atlasHash = '93e7ef93dfcf1e508fa5c55836cfd351da463b1f25cc9e153103352364974e67'
      const referencePath = path.join(
        temporaryRoot,
        'target/atlas-product-migration-reference',
        atlasHash,
        '1024x720',
        'FIL-01',
        'reference.png',
      )
      await mkdir(path.dirname(referencePath), { recursive: true })
      await writeRgbaPng(referencePath, {
        width: 1024,
        height: 720,
        data: Buffer.alloc(1024 * 720 * 4, 255),
      })

      assert.equal(
        await resolveAtlasReferencePath({
          repoRoot: temporaryRoot,
          viewport: '1024x720',
          id: 'FIL-01',
        }),
        referencePath,
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('rejects a reference image whose dimensions do not match its viewport', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-atlas-reference-'))
    try {
      const atlasPath = path.join(
        temporaryRoot,
        'docs/prototypes/viewer-complete-ui-visual-atlas.html',
      )
      await mkdir(path.dirname(atlasPath), { recursive: true })
      await writeFile(atlasPath, '<!doctype html><title>approved atlas</title>')
      const atlasHash = '93e7ef93dfcf1e508fa5c55836cfd351da463b1f25cc9e153103352364974e67'
      const referencePath = path.join(
        temporaryRoot,
        'target/atlas-product-migration-reference',
        atlasHash,
        '1440x900',
        'FIL-01',
        'reference.png',
      )
      await mkdir(path.dirname(referencePath), { recursive: true })
      await writeRgbaPng(referencePath, {
        width: 1024,
        height: 720,
        data: Buffer.alloc(1024 * 720 * 4, 255),
      })

      await assert.rejects(
        resolveAtlasReferencePath({
          repoRoot: temporaryRoot,
          viewport: '1440x900',
          id: 'FIL-01',
        }),
        { code: 'CAPTURE_REFERENCE_DIMENSIONS' },
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('native acceptance CLI', () => {
  it('selects every Wave 1 capture exactly once in ledger order', () => {
    assert.equal(typeof nativeAcceptance.captureIdsForOptions, 'function')
    assert.deepEqual(
      nativeAcceptance.captureIdsForOptions({ mode: 'wave', selector: 1 }),
      [
        'LAU-01', 'LAU-02', 'LAU-03', 'LAU-04', 'LAU-05', 'LAU-06', 'LAU-07',
        'LAU-08', 'LAU-09', 'SID-01', 'SID-02', 'SID-03', 'SID-04', 'STR-01',
        'STR-02', 'STR-03', 'STR-04', 'STR-05', 'THU-01', 'THU-02', 'THU-03',
        'THU-04', 'THU-05', 'THU-06', 'THU-07', 'OTH-01', 'OTH-02', 'OTH-03',
        'SEA-01', 'SEA-02', 'SEA-03', 'SEA-04', 'SEA-05', 'FIL-01', 'FIL-02',
        'FIL-03', 'FIL-04', 'MEN-01', 'MEN-02', 'MEN-03',
      ],
    )
  })

  it('executes a selected wave as one bounded ordered capture batch', async () => {
    const captured = []
    const preflight = { commit: 'a'.repeat(40), marker: 'verified-preflight' }
    let result
    let failure

    try {
      result = await nativeAcceptance.runNativeAcceptanceCli(
        ['--wave', '1', '--viewport', '1440x900'],
        {
          repoRoot: actualRepoRoot,
          collectPreflight: async ({ options }) => {
            assert.equal(options.mode, 'wave')
            return preflight
          },
          captureRecipe: async ({ id, preflight: receivedPreflight }) => {
            assert.equal(receivedPreflight, preflight)
            captured.push(id)
            return {
              directory: `/evidence/${id}`,
              manifest: { id },
            }
          },
        },
      )
    } catch (error) {
      failure = error
    }

    assert.equal(failure, undefined)
    assert.equal(result.mode, 'capture-batch')
    assert.equal(result.viewport, '1440x900')
    assert.equal(result.count, 40)
    assert.deepEqual(captured, nativeAcceptance.captureIdsForOptions({ mode: 'wave', selector: 1 }))
    assert.deepEqual(result.captures[0], {
      id: 'LAU-01',
      directory: '/evidence/LAU-01',
      manifestPath: '/evidence/LAU-01/manifest.json',
    })
    assert.equal(result.captures.at(-1).id, 'MEN-03')
  })

  it('parses every supported selector without performing work', () => {
    assert.deepEqual(parseNativeAcceptanceCli(['--list'], { repoRoot: actualRepoRoot }), {
      mode: 'list',
      selector: null,
      viewport: null,
      outputRoot: path.join(actualRepoRoot, 'target/atlas-product-migration-acceptance'),
      destructive: false,
    })
    assert.deepEqual(
      parseNativeAcceptanceCli(['--preflight', '--viewport', '1024x720'], {
        repoRoot: actualRepoRoot,
      }),
      {
        mode: 'preflight',
        selector: null,
        viewport: '1024x720',
        outputRoot: path.join(actualRepoRoot, 'target/atlas-product-migration-acceptance'),
        destructive: false,
      },
    )
    assert.equal(
      parseNativeAcceptanceCli(['--', '--preflight', '--viewport', '1024x720'], {
        repoRoot: actualRepoRoot,
      }).mode,
      'preflight',
    )
    assert.equal(
      parseNativeAcceptanceCli(['--id', 'FIL-03', '--viewport', '1440x900'], {
        repoRoot: actualRepoRoot,
      }).selector,
      'FIL-03',
    )
    assert.equal(
      parseNativeAcceptanceCli(['--wave', '2', '--viewport', '1024x720'], {
        repoRoot: actualRepoRoot,
      }).selector,
      2,
    )
    assert.equal(
      parseNativeAcceptanceCli(['--all', '--viewport', '1024x720'], {
        repoRoot: actualRepoRoot,
      }).mode,
      'all',
    )
  })

  it('rejects unknown, conflicting, incomplete and unsafe arguments', () => {
    for (const argv of [
      ['--unknown'],
      ['--id', 'NOPE-01', '--viewport', '1024x720'],
      ['--id', 'FIL-01', '--wave', '1', '--viewport', '1024x720'],
      ['--id', 'FIL-01'],
      ['--wave', '5', '--viewport', '1024x720'],
      ['--all', '--viewport', '800x600'],
      ['--all', '--viewport', '1024x720', '--output-root', '/tmp/evidence'],
    ]) {
      assert.throws(
        () => parseNativeAcceptanceCli(argv, { repoRoot: actualRepoRoot }),
        AcceptanceError,
      )
    }
  })

  it('blocks dirty or mismatched capture but keeps list and preflight non-destructive', () => {
    const capture = parseNativeAcceptanceCli(
      ['--id', 'FIL-01', '--viewport', '1024x720'],
      { repoRoot: actualRepoRoot },
    )
    const list = parseNativeAcceptanceCli(['--list'], { repoRoot: actualRepoRoot })
    const preflight = parseNativeAcceptanceCli(
      ['--preflight', '--viewport', '1024x720'],
      { repoRoot: actualRepoRoot },
    )
    assert.equal(list.destructive, false)
    assert.equal(preflight.destructive, false)
    assert.throws(
      () =>
        validateCapturePreflight({
          options: capture,
          dirty: true,
          processes: [],
          windows: [],
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_DIRTY_WORKTREE' },
    )
    assert.throws(
      () =>
        validateCapturePreflight({
          options: capture,
          dirty: false,
          processes: parseProcessTable(`101 1 101 /Applications/Viewer.app/Contents/MacOS/viewer-desktop`),
          windows: [{ pid: 101, windowId: 4, width: 1024, height: 720 }],
          executablePath,
          controllerPid: 999,
        }),
      { code: 'PRECONDITION_VIEWER_PATH' },
    )
  })

  it('polls conditions without arbitrary recipe sleeps and caps at ten seconds', async () => {
    let checks = 0
    const result = await waitFor(
      () => {
        checks += 1
        return checks === 3 ? 'ready' : false
      },
      { timeoutMs: 100, intervalMs: 1 },
    )
    assert.equal(result, 'ready')
    await assert.rejects(
      waitFor(() => false, { timeoutMs: 2, intervalMs: 1 }),
      { code: 'PRECONDITION_WAIT_TIMEOUT' },
    )
    assert.throws(
      () => waitFor(() => true, { timeoutMs: 10_001, intervalMs: 1 }),
      { code: 'PRECONDITION_WAIT_LIMIT' },
    )
  })

  it('requires a condition to remain true for its full stability window', async () => {
    const acceptance = await import('./viewer-native-acceptance.mjs')
    assert.equal(typeof acceptance.waitForStable, 'function')
    let checks = 0

    const result = await acceptance.waitForStable(
      () => {
        checks += 1
        return checks !== 2
      },
      { timeoutMs: 100, intervalMs: 1, stableMs: 5 },
    )

    assert.equal(result, true)
    assert.ok(checks >= 6)
  })

  it('refuses open-panel navigation outside the current home fixture boundary', async () => {
    await assert.rejects(
      openProjectViaPanel({
        client: {},
        actions: [],
        projectPath: '/tmp/not-an-acceptance-fixture',
        window: { x: 0, y: 0, width: 1024, height: 720 },
      }),
      { code: 'SAFETY_FIXTURE_PATH' },
    )
  })

  it('opens the project chooser at most once and closes it after entry failure', async () => {
    const requests = []
    const client = {
      async request(command, payload) {
        requests.push({ command, payload })
        if (command === 'query') {
          if (payload.target?.name === '选择项目文件夹') {
            return {
              elements: [
                {
                  role: 'AXButton',
                  name: '选择项目文件夹',
                  frame: { x: 400, y: 300, width: 120, height: 36 },
                },
              ],
            }
          }
          throw new AcceptanceError(
            'STATE_FIXTURE_ENTRY_FAILED',
            'simulated fixture entry failure',
          )
        }
        return { performed: true, command }
      },
    }
    const actions = []
    const input = {
      client,
      actions,
      projectPath: path.join(os.homedir(), 'ViewerAcceptanceFixture'),
      window: { x: 0, y: 0, width: 1024, height: 720 },
    }
    await assert.rejects(openProjectViaPanel(input), {
      code: 'STATE_FIXTURE_ENTRY_FAILED',
    })
    assert.equal(
      requests.filter(
        (request) =>
          request.command === 'activate' &&
          request.payload?.target?.name === '选择项目文件夹',
      ).length,
      1,
    )
    assert.equal(
      requests.filter((request) => request.payload?.target?.name === 'Cancel')
        .length,
      1,
    )
    await assert.rejects(openProjectViaPanel(input), {
      code: 'STATE_OPEN_PANEL_REENTRY',
    })
    assert.equal(
      requests.filter(
        (request) =>
          request.command === 'activate' &&
          request.payload?.target?.name === '选择项目文件夹',
      ).length,
      1,
    )
  })

  it('opens the native chooser from the concise project-error recovery action', async () => {
    const requests = []
    const client = {
      async request(command, payload) {
        requests.push({ command, payload })
        if (command === 'query' && payload.target?.name === '选择项目文件夹') {
          throw new AcceptanceError('STATE_TARGET_NOT_FOUND', 'not idle')
        }
        if (command === 'query' && payload.target?.name === '重新选择') {
          return {
            elements: [
              {
                role: 'AXButton',
                name: '重新选择',
                frame: { x: 400, y: 300, width: 88, height: 36 },
              },
            ],
          }
        }
        if (command === 'query') {
          throw new AcceptanceError('STATE_FIXTURE_ENTRY_FAILED', 'stop after opening')
        }
        return { performed: true, command }
      },
    }
    await assert.rejects(
      openProjectViaPanel({
        client,
        actions: [],
        projectPath: path.join(os.homedir(), 'ViewerAcceptanceFixture'),
        window: { x: 0, y: 0, width: 1024, height: 720 },
      }),
      { code: 'STATE_FIXTURE_ENTRY_FAILED' },
    )
    assert.equal(
      requests.filter(
        (request) =>
          request.command === 'activate' && request.payload?.target?.name === '重新选择',
      ).length,
      1,
    )
  })

  it('confirms the selected project with a direct pointer click for transient launch states', async () => {
    const requests = []
    const projectPath = path.join(
      os.homedir(),
      'ViewerAcceptanceRuns',
      'run-123',
      '测试图',
    )
    const client = {
      async request(command, payload) {
        requests.push({ command, payload })
        if (command !== 'query') return { performed: true, command }
        if (payload.target?.name === '选择项目文件夹') {
          return {
            elements: [
              {
                role: 'AXButton',
                name: '选择项目文件夹',
                frame: { x: 400, y: 300, width: 120, height: 36 },
              },
            ],
          }
        }
        if (payload.target?.role === 'AXStaticText') {
          return {
            elements: [
              {
                role: 'AXStaticText',
                name: path.basename(os.homedir()),
                frame: { x: 100, y: 100, width: 80, height: 18 },
              },
            ],
          }
        }
        if (payload.target?.role === 'AXTextField') {
          return {
            elements: [
              {
                role: 'AXTextField',
                name: payload.target.name,
                frame: { x: 200, y: 200, width: 100, height: 18 },
              },
            ],
          }
        }
        if (payload.target?.role === 'AXButton' && payload.target?.name === 'Open') {
          return {
            elements: [
              {
                role: 'AXButton',
                name: 'Open',
                frame: { x: 800, y: 600, width: 80, height: 32 },
              },
            ],
          }
        }
        throw new AcceptanceError('STATE_TARGET_NOT_FOUND', 'unexpected query', {
          command,
          payload,
        })
      },
    }

    await openProjectViaPanel({
      client,
      actions: [],
      projectPath,
      window: { x: 0, y: 0, width: 1024, height: 720 },
      waitForWorkspace: false,
    })

    assert.equal(
      requests.some(
        (request) =>
          request.command === 'activate' && request.payload?.target?.name === 'Open',
      ),
      false,
    )
    assert.deepEqual(requests.at(-1), {
      command: 'pointer',
      payload: { kind: 'click', point: { x: 840, y: 616 } },
    })
  })
})

async function sourceFiles(root) {
  const entries = await readdir(root, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const candidate = path.join(root, entry.name)
    if (entry.isDirectory()) files.push(...(await sourceFiles(candidate)))
    else if (entry.isFile()) files.push(candidate)
  }
  return files
}

describe('package commands and non-shipping boundary', () => {
  it('exposes only the two explicit local acceptance commands', async () => {
    const packageJson = JSON.parse(
      await readFile(path.join(actualRepoRoot, 'package.json'), 'utf8'),
    )
    assert.equal(
      packageJson.scripts['test:native-acceptance'],
      'node --test scripts/viewer-native-acceptance.test.mjs',
    )
    assert.equal(
      packageJson.scripts['accept:native'],
      'node scripts/viewer-native-acceptance.mjs',
    )
  })

  it('keeps the controller and helper out of every shipping input', async () => {
    const tauriConfig = await readFile(
      path.join(actualRepoRoot, 'src-tauri/tauri.conf.json'),
      'utf8',
    )
    assert.doesNotMatch(tauriConfig, /viewer-native-acceptance/)

    const shippingFiles = [
      ...(await sourceFiles(path.join(actualRepoRoot, 'ui/src'))),
      ...(await sourceFiles(path.join(actualRepoRoot, 'src-tauri/src'))),
    ]
    for (const file of shippingFiles) {
      const source = await readFile(file, 'utf8')
      assert.doesNotMatch(source, /viewer-native-acceptance/, file)
    }
    const productionUi = path.join(actualRepoRoot, 'ui/dist')
    try {
      for (const file of await sourceFiles(productionUi)) {
        const source = await readFile(file)
        assert.equal(source.includes(Buffer.from('viewer-native-acceptance')), false, file)
      }
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error
    }
  })
})

async function runJsonLines(executable, args, requests) {
  const child = spawn(executable, args, { stdio: ['pipe', 'pipe', 'pipe'] })
  child.stdout.setEncoding('utf8')
  child.stderr.setEncoding('utf8')
  let stdout = ''
  let stderr = ''
  child.stdout.on('data', (chunk) => {
    stdout += chunk
  })
  child.stderr.on('data', (chunk) => {
    stderr += chunk
  })
  for (const request of requests) child.stdin.write(`${JSON.stringify(request)}\n`)
  child.stdin.end()
  const exitCode = await new Promise((resolve, reject) => {
    child.once('error', reject)
    child.once('close', resolve)
  })
  return {
    exitCode,
    stderr,
    responses: stdout
      .trim()
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line)),
  }
}

async function createProtocolFixture(temporaryRoot) {
  const fixturePath = path.join(temporaryRoot, 'protocol-fixture.mjs')
  await writeFile(
    fixturePath,
    `import { appendFileSync } from 'node:fs'
import readline from 'node:readline'

const mode = process.argv[2]
const input = readline.createInterface({ input: process.stdin })
input.on('line', (line) => {
  const request = JSON.parse(line)
  if (process.env.REQUEST_LOG) {
    appendFileSync(process.env.REQUEST_LOG, JSON.stringify(request) + '\\n')
  }
  if (mode === 'timeout') return
  if (mode === 'eof') process.exit(0)
  if (mode === 'malformed') {
    process.stdout.write('{not-json}\\n')
    return
  }
  if (mode === 'stderr') {
    process.stderr.write('fixture diagnostic\\n')
    process.exit(3)
  }
  const sequence = mode === 'out-of-order' ? request.sequence + 1 : request.sequence
  process.stdout.write(JSON.stringify({
    version: 1,
    sequence,
    ok: true,
    result: { mode: 'fixture', command: request.command },
  }) + '\\n')
  if (request.command === 'shutdown') process.exit(0)
})
`,
  )
  return fixturePath
}

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
    const runId = `run-123-${process.pid}-${Date.now()}`
    const runRoot = path.join(
      os.homedir(),
      'ViewerAcceptanceRuns',
      runId,
    )
    const child = path.join(runRoot, 'project', '衣服', 'A01')

    try {
      await mkdir(child, { recursive: true })
      assert.equal(
        await validateFixturePath(runRoot, { runId }),
        runRoot,
      )
      assert.equal(
        await validateFixturePath(child, { runId }),
        child,
      )
    } finally {
      await rm(runRoot, { recursive: true, force: true })
    }
  })

  it('rejects the baseline, repository root, sibling runs and parent escape', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-'))
    const runId = `run-123-${process.pid}-${Date.now()}`
    const fixtureRoot = path.join(
      temporaryRoot,
      'target',
      'atlas-product-migration-fixture',
    )
    const runsRoot = path.join(os.homedir(), 'ViewerAcceptanceRuns')
    const runRoot = path.join(runsRoot, runId)
    const siblingRun = path.join(runsRoot, `run-456-${process.pid}-${Date.now()}`)
    const rejected = [
      '/',
      temporaryRoot,
      path.join(fixtureRoot, 'ViewerAcceptance'),
      siblingRun,
      path.join(runRoot, '..', path.basename(siblingRun)),
      '$HOME/project',
      '~/project',
      path.join(runRoot, '*.jpg'),
    ]

    try {
      await mkdir(path.join(fixtureRoot, 'ViewerAcceptance'), { recursive: true })
      await mkdir(runRoot, { recursive: true })
      await mkdir(siblingRun, { recursive: true })

      for (const candidate of rejected) {
        await assert.rejects(
          validateFixturePath(candidate, { runId }),
          { code: 'SAFETY_FIXTURE_PATH' },
        )
      }
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
      await rm(runRoot, { recursive: true, force: true })
      await rm(siblingRun, { recursive: true, force: true })
    }
  })

  it('rejects a symbolic-link escape from the approved run', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-'))
    const outside = await mkdtemp(path.join(os.tmpdir(), 'viewer-acceptance-outside-'))
    const runId = `run-123-${process.pid}-${Date.now()}`
    const runRoot = path.join(
      os.homedir(),
      'ViewerAcceptanceRuns',
      runId,
    )
    const link = path.join(runRoot, 'escape')

    try {
      await mkdir(runRoot, { recursive: true })
      await writeFile(path.join(outside, 'secret.txt'), 'outside\n')
      await symlink(outside, link)

      await assert.rejects(
        validateFixturePath(path.join(link, 'secret.txt'), {
          runId,
        }),
        { code: 'SAFETY_FIXTURE_PATH' },
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
      await rm(runRoot, { recursive: true, force: true })
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

describe('protocol schema', () => {
  const window = {
    pid: 101,
    windowId: 44,
    x: 20,
    y: 30,
    width: 1024,
    height: 720,
  }
  const inspectRequest = {
    version: 1,
    sequence: 1,
    pid: 101,
    windowId: 44,
    command: 'inspect',
    timeoutMs: 3000,
    payload: {},
  }

  it('accepts the complete strict inspect envelope', () => {
    assert.equal(PROTOCOL_VERSION, 1)
    assert.deepEqual(
      [...ALLOWED_COMMANDS],
      [
        'inspect',
        'query',
        'activate',
        'focus',
        'setValue',
        'key',
        'pointer',
        'drag',
        'finderDrag',
        'capture',
        'shutdown',
      ],
    )
    assert.deepEqual(validateCommand(inspectRequest, { window }), inspectRequest)
  })

  it('rejects invalid common envelope values and extra fields', () => {
    const cases = [
      { ...inspectRequest, version: 2 },
      { ...inspectRequest, sequence: 0 },
      { ...inspectRequest, sequence: 1.5 },
      { ...inspectRequest, pid: 0 },
      { ...inspectRequest, windowId: 0 },
      { ...inspectRequest, command: 'shell' },
      { ...inspectRequest, timeoutMs: 99 },
      { ...inspectRequest, timeoutMs: 10001 },
      { ...inspectRequest, extra: true },
    ]

    for (const request of cases) {
      assert.throws(() => validateCommand(request, { window }), {
        code: 'SAFETY_PROTOCOL',
      })
    }
  })

  it('rejects extra payload fields for an otherwise valid command', () => {
    assert.throws(
      () =>
        validateCommand(
          { ...inspectRequest, payload: { ignored: true } },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
  })

  it('allows only the deterministic rightmost target disambiguator', () => {
    const request = {
      ...inspectRequest,
      command: 'query',
      payload: {
        target: { role: 'AXTextField', name: 'target', position: 'rightmost' },
      },
    }
    assert.deepEqual(validateCommand(request, { window }), request)
    assert.throws(
      () =>
        validateCommand(
          {
            ...request,
            payload: { target: { ...request.payload.target, position: 'first' } },
          },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
  })

  it('accepts a bounded native accessibility name prefix selector', () => {
    const request = {
      version: 1,
      sequence: 7,
      pid: 101,
      windowId: 44,
      command: 'query',
      timeoutMs: 3000,
      payload: { target: { namePrefix: '结果仍在更新' } },
    }

    assert.deepEqual(validateCommand(request, { window }), request)
    assert.throws(
      () =>
        validateCommand(
          { ...request, payload: { target: { namePrefix: '' } } },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
  })

  it('accepts bounded setValue text and rejects text above 4096 scalars', () => {
    const request = {
      ...inspectRequest,
      command: 'setValue',
      payload: {
        target: { role: 'AXTextField', name: '搜索' },
        text: '衣服/A01',
      },
    }
    assert.deepEqual(validateCommand(request, { window }), request)
    assert.throws(
      () =>
        validateCommand(
          {
            ...request,
            payload: { ...request.payload, text: '图'.repeat(4097) },
          },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
  })

  it('accepts only the approved key and modifier vocabulary', () => {
    const request = {
      ...inspectRequest,
      command: 'key',
      payload: { key: 'escape', modifiers: ['shift', 'command'] },
    }
    assert.deepEqual(validateCommand(request, { window }), request)
    for (const key of ['g', 'period', '0', '9']) {
      assert.deepEqual(
        validateCommand(
          {
            ...request,
            payload: { key, modifiers: ['shift', 'command'] },
          },
          { window },
        ).payload,
        { key, modifiers: ['shift', 'command'] },
      )
    }

    for (const payload of [
      { key: 'f1', modifiers: [] },
      { key: 'escape', modifiers: ['fn'] },
      { key: 'escape', modifiers: ['shift', 'shift'] },
      { key: 'escape', modifiers: [], script: 'rm' },
    ]) {
      assert.throws(
        () => validateCommand({ ...request, payload }, { window }),
        { code: 'SAFETY_COMMAND' },
      )
    }
  })

  it('validates pointer and drag coordinates against the current window', () => {
    const pointerRequest = {
      ...inspectRequest,
      command: 'pointer',
      payload: { kind: 'rightClick', point: { x: 100, y: 200 } },
    }
    const dragRequest = {
      ...inspectRequest,
      command: 'drag',
      payload: {
        from: { x: 100, y: 200 },
        to: { x: 300, y: 400 },
        durationMs: 400,
      },
    }
    assert.deepEqual(validateCommand(pointerRequest, { window }), pointerRequest)
    for (const payload of [
      { kind: 'move', point: { x: 512, y: 12 } },
      {
        kind: 'click',
        point: { x: 200, y: 300 },
        modifiers: ['command'],
      },
      {
        kind: 'leftDown',
        point: { x: 200, y: 300 },
        modifiers: ['option'],
      },
      {
        kind: 'leftDrag',
        point: { x: 300, y: 400 },
        modifiers: ['option'],
      },
      { kind: 'leftUp', point: { x: 300, y: 400 } },
      { kind: 'rightDown', point: { x: 200, y: 300 } },
      { kind: 'rightDrag', point: { x: 300, y: 400 } },
      { kind: 'rightUp', point: { x: 300, y: 400 } },
    ]) {
      assert.deepEqual(
        validateCommand({ ...pointerRequest, payload }, { window }).payload,
        payload,
      )
    }
    assert.deepEqual(
      validateCommand(
        {
          ...pointerRequest,
          payload: {
            kind: 'scroll',
            point: { x: 100, y: 200 },
            deltaY: -12,
          },
        },
        { window },
      ).payload.deltaY,
      -12,
    )
    assert.deepEqual(validateCommand(dragRequest, { window }), dragRequest)

    for (const payload of [
      { kind: 'leftUp', point: { x: 300, y: 400 }, modifiers: ['option'] },
      { kind: 'leftDrag', point: { x: 300, y: 400 }, modifiers: ['option', 'option'] },
    ]) {
      assert.throws(
        () => validateCommand({ ...pointerRequest, payload }, { window }),
        { code: 'SAFETY_COMMAND' },
      )
    }

    assert.throws(
      () =>
        validateCommand(
          {
            ...pointerRequest,
            payload: { kind: 'click', point: { x: 1024, y: 10 } },
          },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
    assert.throws(
      () =>
        validateCommand(
          {
            ...pointerRequest,
            payload: {
              kind: 'scroll',
              point: { x: 100, y: 200 },
              deltaY: 0,
            },
          },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
    assert.throws(
      () =>
        validateCommand(
          {
            ...dragRequest,
            payload: { ...dragRequest.payload, durationMs: 5001 },
          },
          { window },
        ),
      { code: 'SAFETY_COMMAND' },
    )
  })

  it('accepts only a home-scoped Finder drag into the approved Viewer window', () => {
    const request = {
      ...inspectRequest,
      command: 'finderDrag',
      payload: {
        path: path.join(
          os.homedir(),
          'ViewerAcceptanceRuns',
          'run-123',
          'entry-and-loading',
          '测试图',
        ),
        destination: { x: 512, y: 360 },
        release: false,
        durationMs: 500,
      },
    }

    assert.deepEqual(validateCommand(request, { window }), request)

    for (const payload of [
      { ...request.payload, path: 'ViewerAcceptanceRuns/run-123/测试图' },
      { ...request.payload, path: path.join(os.homedir(), 'Desktop', '测试图') },
      { ...request.payload, path: `${os.homedir()}/ViewerAcceptanceRuns/$RUN/测试图` },
      { ...request.payload, destination: { x: 1024, y: 360 } },
      { ...request.payload, release: 'false' },
      { ...request.payload, durationMs: 49 },
      { ...request.payload, durationMs: 5001 },
    ]) {
      assert.throws(
        () => validateCommand({ ...request, payload }, { window }),
        { code: 'SAFETY_COMMAND' },
      )
    }
  })
})

describe('Swift helper protocol', () => {
  it('compiles warning-free and correlates valid and rejected commands', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-swift-helper-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const inspect = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const unknown = { ...inspect, sequence: 2, command: 'shell' }

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(helperPath, ['--protocol-test'], [inspect, unknown])

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses, [
        {
          version: 1,
          sequence: 1,
          ok: true,
          result: { mode: 'protocol-test' },
        },
        {
          version: 1,
          sequence: 2,
          ok: false,
          error: { code: 'SAFETY_COMMAND', message: 'Unsupported command' },
        },
      ])
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('NativeAcceptanceClient', () => {
  const window = {
    pid: 101,
    windowId: 44,
    x: 20,
    y: 30,
    width: 1024,
    height: 720,
  }

  it('correlates ordered requests and closes with shutdown', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-client-'))
    const requestLog = path.join(temporaryRoot, 'requests.jsonl')
    let client

    try {
      const fixturePath = await createProtocolFixture(temporaryRoot)
      client = new NativeAcceptanceClient({
        executablePath: process.execPath,
        args: [fixturePath, 'valid'],
        env: { ...process.env, REQUEST_LOG: requestLog },
        pid: 101,
        window,
        defaultTimeoutMs: 500,
      })

      assert.deepEqual(await client.start(), { mode: 'fixture', command: 'inspect' })
      assert.deepEqual(
        await client.request('query', {
          target: { role: 'AXButton', name: '筛选' },
        }),
        { mode: 'fixture', command: 'query' },
      )
      await client.close()
      client = undefined

      const requests = (await readFile(requestLog, 'utf8'))
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line))
      assert.deepEqual(
        requests.map(({ sequence, command }) => ({ sequence, command })),
        [
          { sequence: 1, command: 'inspect' },
          { sequence: 2, command: 'query' },
          { sequence: 3, command: 'shutdown' },
        ],
      )
    } finally {
      await client?.terminate()
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  for (const [mode, code] of [
    ['out-of-order', 'SAFETY_PROTOCOL'],
    ['malformed', 'SAFETY_PROTOCOL'],
    ['eof', 'PRECONDITION_HELPER_EXIT'],
    ['timeout', 'PRECONDITION_HELPER_TIMEOUT'],
  ]) {
    it(`rejects ${mode} helper behavior`, async () => {
      const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-client-'))
      let client
      try {
        const fixturePath = await createProtocolFixture(temporaryRoot)
        client = new NativeAcceptanceClient({
          executablePath: process.execPath,
          args: [fixturePath, mode],
          pid: 101,
          window,
          defaultTimeoutMs: 150,
        })
        await assert.rejects(client.start(), { code })
      } finally {
        await client?.terminate()
        await rm(temporaryRoot, { recursive: true, force: true })
      }
    })
  }

  it('preserves helper stderr when the child exits', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-client-'))
    let client
    try {
      const fixturePath = await createProtocolFixture(temporaryRoot)
      client = new NativeAcceptanceClient({
        executablePath: process.execPath,
        args: [fixturePath, 'stderr'],
        pid: 101,
        window,
        defaultTimeoutMs: 500,
      })
      await assert.rejects(client.start(), (error) => {
        assert.equal(error.code, 'PRECONDITION_HELPER_EXIT')
        assert.match(error.details.stderr, /fixture diagnostic/)
        return true
      })
    } finally {
      await client?.terminate()
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('native helper build cache', () => {
  it('keys the warning-free Swift executable by the complete source hash', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-helper-build-'))
    const sourcePath = path.join(temporaryRoot, 'viewer-native-acceptance.swift')
    try {
      await copyFile(
        new URL('./viewer-native-acceptance.swift', import.meta.url),
        sourcePath,
      )
      const first = await buildNativeHelper({
        repoRoot: temporaryRoot,
        sourcePath,
      })
      const firstModifiedAt = (await stat(first.executablePath)).mtimeMs
      const second = await buildNativeHelper({
        repoRoot: temporaryRoot,
        sourcePath,
      })

      assert.deepEqual(second, first)
      assert.equal((await stat(second.executablePath)).mtimeMs, firstModifiedAt)
      assert.match(
        first.executablePath,
        /target\/native-acceptance-tools\/[0-9a-f]{64}\/viewer-native-acceptance-helper$/,
      )

      await writeFile(sourcePath, `${await readFile(sourcePath, 'utf8')}\n`)
      const changed = await buildNativeHelper({
        repoRoot: temporaryRoot,
        sourcePath,
      })
      assert.notEqual(changed.sourceHash, first.sourceHash)
      assert.notEqual(changed.executablePath, first.executablePath)
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('native validation', () => {
  it('shares PID, window, unique-target and coordinate guards with the fixture adapter', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-native-fixture-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      base,
      { ...base, sequence: 2, pid: 202 },
      {
        ...base,
        sequence: 3,
        command: 'query',
        payload: { target: { role: 'AXButton', name: '筛选' } },
      },
      {
        ...base,
        sequence: 4,
        command: 'query',
        payload: {
          target: {
            role: 'AXButton',
            name: '重复',
            position: 'rightmost',
          },
        },
      },
      {
        ...base,
        sequence: 5,
        command: 'query',
        payload: { target: { role: 'AXButton', name: '重复' } },
      },
      {
        ...base,
        sequence: 6,
        command: 'query',
        payload: { target: { role: 'AXButton', name: '不存在' } },
      },
      {
        ...base,
        sequence: 7,
        command: 'pointer',
        payload: { kind: 'click', point: { x: 1024, y: 10 } },
      },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses[0], {
        version: 1,
        sequence: 1,
        ok: true,
        result: {
          pid: 101,
          windowId: 44,
          title: 'Viewer Acceptance Fixture',
          frame: { x: 20, y: 30, width: 1024, height: 720 },
          focused: true,
          frontmost: true,
        },
      })
      assert.deepEqual(result.responses[1], {
        version: 1,
        sequence: 2,
        ok: false,
        error: {
          code: 'PRECONDITION_WINDOW_OWNER',
          message: 'Target window changed',
        },
      })
      assert.deepEqual(result.responses[2], {
        version: 1,
        sequence: 3,
        ok: true,
        result: {
          elements: [
            {
              role: 'AXButton',
              name: '筛选',
              identifier: 'toolbar-filter',
              enabled: true,
              focused: false,
              frame: { x: 900, y: 40, width: 80, height: 32 },
              path: [0, 0],
            },
          ],
        },
      })
      assert.deepEqual(result.responses[3], {
        version: 1,
        sequence: 4,
        ok: true,
        result: {
          elements: [
            {
              role: 'AXButton',
              name: '重复',
              identifier: 'duplicate-two',
              enabled: true,
              focused: false,
              frame: { x: 790, y: 40, width: 80, height: 32 },
              path: [0, 2],
            },
          ],
        },
      })
      assert.deepEqual(
        result.responses.slice(4).map((response) => response.error.code),
        [
          'STATE_TARGET_NOT_UNIQUE',
          'STATE_TARGET_NOT_FOUND',
          'SAFETY_POINT_OUTSIDE_WINDOW',
        ],
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('validates every action payload without emitting events in fixture mode', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-native-actions-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      {
        ...base,
        sequence: 1,
        command: 'activate',
        payload: { target: { role: 'AXButton', name: '筛选' } },
      },
      {
        ...base,
        sequence: 2,
        command: 'focus',
        payload: { target: { identifier: 'toolbar-filter' } },
      },
      {
        ...base,
        sequence: 3,
        command: 'setValue',
        payload: {
          target: { role: 'AXTextField', name: '搜索' },
          text: '衣服/A01',
        },
      },
      {
        ...base,
        sequence: 4,
        command: 'key',
        payload: { key: 'escape', modifiers: ['shift'] },
      },
      {
        ...base,
        sequence: 5,
        command: 'key',
        payload: { key: 'period', modifiers: ['shift', 'command'] },
      },
      {
        ...base,
        sequence: 6,
        command: 'pointer',
        payload: {
          kind: 'scroll',
          point: { x: 100, y: 200 },
          deltaY: 12,
        },
      },
      {
        ...base,
        sequence: 7,
        command: 'pointer',
        payload: { kind: 'rightClick', point: { x: 100, y: 200 } },
      },
      {
        ...base,
        sequence: 8,
        command: 'pointer',
        payload: { kind: 'move', point: { x: 512, y: 12 } },
      },
      {
        ...base,
        sequence: 9,
        command: 'pointer',
        payload: {
          kind: 'click',
          point: { x: 200, y: 300 },
          modifiers: ['command'],
        },
      },
      {
        ...base,
        sequence: 10,
        command: 'drag',
        payload: {
          from: { x: 100, y: 200 },
          to: { x: 300, y: 400 },
          durationMs: 400,
        },
      },
      {
        ...base,
        sequence: 11,
        command: 'capture',
        payload: { path: '/tmp/viewer-acceptance-product.png' },
      },
      {
        ...base,
        sequence: 12,
        command: 'shutdown',
        payload: {},
      },
      {
        ...base,
        sequence: 13,
        command: 'setValue',
        payload: {
          target: { role: 'AXTextField', name: '搜索' },
          text: '图'.repeat(4097),
        },
      },
      {
        ...base,
        sequence: 14,
        command: 'drag',
        payload: {
          from: { x: 100, y: 200 },
          to: { x: 1024, y: 400 },
          durationMs: 400,
        },
      },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(
        result.responses.slice(0, 12).map((response) => ({
          ok: response.ok,
          performed: response.result.performed,
          command: response.result.command,
        })),
        [
          { ok: true, performed: true, command: 'activate' },
          { ok: true, performed: true, command: 'focus' },
          { ok: true, performed: true, command: 'setValue' },
          { ok: true, performed: true, command: 'key' },
          { ok: true, performed: true, command: 'key' },
          { ok: true, performed: true, command: 'pointer' },
          { ok: true, performed: true, command: 'pointer' },
          { ok: true, performed: true, command: 'pointer' },
          { ok: true, performed: true, command: 'pointer' },
          { ok: true, performed: true, command: 'drag' },
          { ok: true, performed: true, command: 'capture' },
          { ok: true, performed: true, command: 'shutdown' },
        ],
      )
      assert.deepEqual(
        result.responses.slice(12).map((response) => response.error.code),
        ['SAFETY_COMMAND', 'SAFETY_POINT_OUTSIDE_WINDOW'],
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('validates held secondary-button input and the keyboard context-menu key in Swift', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-native-secondary-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      {
        ...base,
        command: 'pointer',
        payload: { kind: 'rightDown', point: { x: 200, y: 300 } },
      },
      {
        ...base,
        sequence: 2,
        command: 'pointer',
        payload: { kind: 'rightDrag', point: { x: 200, y: 212 } },
      },
      {
        ...base,
        sequence: 3,
        command: 'pointer',
        payload: { kind: 'rightUp', point: { x: 200, y: 212 } },
      },
      {
        ...base,
        sequence: 4,
        command: 'key',
        payload: { key: 'f10', modifiers: ['shift'] },
      },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(
        result.responses.map((response) => ({
          ok: response.ok,
          command: response.result?.command,
        })),
        [
          { ok: true, command: 'pointer' },
          { ok: true, command: 'pointer' },
          { ok: true, command: 'pointer' },
          { ok: true, command: 'key' },
        ],
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('validates Finder drag scope and destination in Swift fixture mode', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-finder-drag-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'finderDrag',
      timeoutMs: 3000,
      payload: {
        path: path.join(os.homedir(), 'ViewerAcceptanceRuns', 'run-123', '测试图'),
        destination: { x: 512, y: 360 },
        release: false,
        durationMs: 500,
      },
    }

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-fixture'],
        [
          base,
          {
            ...base,
            sequence: 2,
            payload: { ...base.payload, path: path.join(os.homedir(), 'Desktop', '测试图') },
          },
          {
            ...base,
            sequence: 3,
            payload: { ...base.payload, destination: { x: 1024, y: 360 } },
          },
        ],
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses[0].result, {
        performed: true,
        command: 'finderDrag',
      })
      assert.deepEqual(
        result.responses.slice(1).map((response) => response.error.code),
        ['SAFETY_COMMAND', 'SAFETY_POINT_OUTSIDE_WINDOW'],
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('native production guard', () => {
  it('rejects an invalid Viewer PID before attempting a native action', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-native-guard-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const request = {
      version: 1,
      sequence: 1,
      pid: 2_147_483_647,
      windowId: 1,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(helperPath, [], [request])

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses, [
        {
          version: 1,
          sequence: 1,
          ok: false,
          error: {
            code: 'PRECONDITION_VIEWER_PID',
            message: 'Viewer PID is not running',
          },
        },
      ])
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('MacAdapter command dispatch', () => {
  it('routes every validated command through the recording system adapter', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-mac-adapter-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      base,
      {
        ...base,
        sequence: 2,
        command: 'query',
        payload: { target: { identifier: 'toolbar-filter' } },
      },
      {
        ...base,
        sequence: 3,
        command: 'activate',
        payload: { target: { identifier: 'toolbar-filter' } },
      },
      {
        ...base,
        sequence: 4,
        command: 'focus',
        payload: { target: { identifier: 'toolbar-search' } },
      },
      {
        ...base,
        sequence: 5,
        command: 'setValue',
        payload: {
          target: { identifier: 'toolbar-search' },
          text: '衣服/A01',
        },
      },
      {
        ...base,
        sequence: 6,
        command: 'key',
        payload: { key: 'escape', modifiers: [] },
      },
      {
        ...base,
        sequence: 7,
        command: 'pointer',
        payload: { kind: 'click', point: { x: 100, y: 200 } },
      },
      {
        ...base,
        sequence: 8,
        command: 'drag',
        payload: {
          from: { x: 100, y: 200 },
          to: { x: 300, y: 400 },
          durationMs: 400,
        },
      },
      {
        ...base,
        sequence: 9,
        command: 'finderDrag',
        payload: {
          path: path.join(os.homedir(), 'ViewerAcceptanceRuns', 'run-123', '测试图'),
          destination: { x: 512, y: 360 },
          release: false,
          durationMs: 500,
        },
      },
      {
        ...base,
        sequence: 10,
        command: 'capture',
        payload: { path: '/tmp/viewer-mac-adapter.png' },
      },
      { ...base, sequence: 11, command: 'shutdown', payload: {} },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-mac-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.deepEqual(result.responses[0].result, {
        pid: 101,
        windowId: 44,
        title: 'Viewer Mac Adapter Fixture',
        frame: { x: 20, y: 30, width: 1024, height: 720 },
        focused: true,
        frontmost: true,
      })
      assert.deepEqual(result.responses[1].result.elements, [
        {
          role: 'AXButton',
          name: '筛选',
          identifier: 'toolbar-filter',
          enabled: true,
          focused: false,
          frame: { x: 900, y: 40, width: 80, height: 32 },
          path: [0, 0],
        },
      ])
      assert.deepEqual(
        result.responses.slice(2, 8).map((response) => response.result.command),
        ['activate', 'focus', 'setValue', 'key', 'pointer', 'drag'],
      )
      assert.deepEqual(result.responses[8].result, {
        command: 'finderDrag',
        performed: true,
        held: true,
      })
      assert.deepEqual(result.responses[9].result, {
        command: 'capture',
        performed: true,
        width: 2048,
        height: 1440,
        sha256: 'a'.repeat(64),
      })
      assert.deepEqual(result.responses[10].result, {
        command: 'shutdown',
        performed: true,
      })
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('focuses an owned background Viewer window before guarded input', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-mac-focus-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    const base = {
      version: 1,
      sequence: 1,
      pid: 101,
      windowId: 44,
      command: 'inspect',
      timeoutMs: 3000,
      payload: {},
    }
    const requests = [
      base,
      {
        ...base,
        sequence: 2,
        command: 'focus',
        payload: { target: { role: 'AXWindow', name: 'Viewer Mac Adapter Fixture' } },
      },
      { ...base, sequence: 3 },
      {
        ...base,
        sequence: 4,
        command: 'pointer',
        payload: { kind: 'click', point: { x: 100, y: 200 } },
      },
    ]

    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      const result = await runJsonLines(
        helperPath,
        ['--protocol-test', '--protocol-test-mac-background-fixture'],
        requests,
      )

      assert.equal(result.exitCode, 0)
      assert.equal(result.stderr, '')
      assert.equal(result.responses[0].result.frontmost, false)
      assert.deepEqual(result.responses[1].result, {
        performed: true,
        command: 'focus',
      })
      assert.equal(result.responses[2].result.frontmost, true)
      assert.deepEqual(result.responses[3].result, {
        performed: true,
        command: 'pointer',
      })
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('native window discovery', () => {
  it('returns structured owned layer-zero windows before protocol binding', async () => {
    const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), 'viewer-window-discovery-'))
    const helperPath = path.join(temporaryRoot, 'viewer-native-acceptance-helper')
    try {
      await execFileAsync('xcrun', [
        'swiftc',
        '-warnings-as-errors',
        new URL('./viewer-native-acceptance.swift', import.meta.url).pathname,
        '-o',
        helperPath,
      ])
      assert.deepEqual(
        await discoverNativeWindows({
          helperPath,
          pid: 101,
          protocolTest: true,
        }),
        [
          {
            pid: 101,
            windowId: 44,
            title: 'Viewer Discovery Fixture',
            x: 20,
            y: 30,
            width: 1024,
            height: 720,
          },
        ],
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})
