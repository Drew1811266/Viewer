import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import path from 'node:path'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import {
  IMAGE_RENDER_ACCEPTANCE_IDS,
  NATIVE_SMOKE_IDS,
  buildNativeImageRenderAcceptancePlan,
} from '../viewer-native-acceptance.mjs'

const repoRoot = path.resolve(fileURLToPath(new URL('../..', import.meta.url)))

describe('native image renderer acceptance selection', () => {
  it('covers preview, review authoring, magnifier and all extended markup states', () => {
    assert.deepEqual(IMAGE_RENDER_ACCEPTANCE_IDS, [
      'PRE-08',
      'RVW-16',
      'RVW-17',
      'RVW-18',
      'RVW-19',
      'RVW-20',
      'RVW-24',
      'RVW-33',
      'RVW-34',
      'RVW-35',
      'RVW-36',
      'RVW-37',
    ])
  })

  it('pairs the native surface smoke journey with every selected visual state', () => {
    const plan = buildNativeImageRenderAcceptancePlan()
    const catalog = JSON.parse(
      readFileSync(path.join(repoRoot, 'ui/src/acceptance/acceptanceStateCatalog.json'), 'utf8'),
    )
    const catalogIds = new Set(catalog.map((entry) => entry.id))

    assert.equal(plan.renderer, 'native-wgpu-metal')
    assert.deepEqual(plan.nativeSmokeIds, ['preview-native'])
    assert.ok(NATIVE_SMOKE_IDS.includes('preview-native'))
    assert.deepEqual(plan.visualStateIds, IMAGE_RENDER_ACCEPTANCE_IDS)
    for (const id of IMAGE_RENDER_ACCEPTANCE_IDS) assert.ok(catalogIds.has(id), id)
  })
})
