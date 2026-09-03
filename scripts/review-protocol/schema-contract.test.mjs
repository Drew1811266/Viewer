import assert from 'node:assert/strict'
import { readFile, stat } from 'node:fs/promises'
import test from 'node:test'
import { validateFixture } from './schema-fixture-validation.mjs'
import { createProjectV3Case } from './v3-fixtures.mjs'
import { blake3Hex } from './read-latest.mjs'

async function schema(name) {
  return JSON.parse(await readFile(`docs/protocol/${name}`, 'utf8'))
}

test('v2 schemas declare closed bounded anchor and artifact contracts', async () => {
  const [draft, index, round] = await Promise.all([
    schema('viewer-review-draft-v2.schema.json'),
    schema('viewer-review-index-v2.schema.json'),
    schema('viewer-review-round-v2.schema.json'),
  ])
  const anchor = round.$defs.anchor
  const stroke = anchor.oneOf.find((entry) => entry.properties.kind.const === 'imageStroke')

  assert.equal(draft.additionalProperties, false)
  assert.equal(index.additionalProperties, false)
  assert.equal(round.additionalProperties, false)
  assert.equal(stroke.properties.points.minItems, 2)
  assert.equal(stroke.properties.points.maxItems, 2048)
  assert.equal(stroke.properties.points.items.additionalProperties, false)
  assert.equal(round.$defs.normalizedCoordinate.minimum, 0)
  assert.equal(round.$defs.normalizedCoordinate.maximum, 1)
  assert.equal(round.$defs.digest.pattern, '^[0-9a-f]{64}$')
  assert.equal(round.$defs.artifact.additionalProperties, false)
  assert.equal(round.$defs.artifact.properties.mediaType.const, 'image/png')
  assert.equal(index.$defs.roundRecord.properties.location.pattern, '^rounds/(?!.*(?:^|/)\\.\\.?/)(?!/)(?!.*\\\\).+$')
  assert.equal(round.$defs.artifact.properties.relativePath.pattern, '^artifacts/(?!.*(?:^|/)\\.\\.?/)(?!/)(?!.*\\\\).+$')
})

test('v3 golden documents validate, while unknown fields and wrong roles are rejected', async () => {
  for (const [file, schemaName] of [
    ['review-state-v3', 'viewer-review-state-v3'], ['review-index-v3', 'viewer-review-index-v3'],
    ['review-archive-v3', 'viewer-review-archive-v3'], ['review-read-result-v3', 'viewer-review-read-result-v3'],
    ['review-usage-v1', 'viewer-review-usage-v1'],
  ]) {
    const definition = await schema(`${schemaName}.schema.json`)
    const value = JSON.parse(await readFile(`tests/fixtures/review-protocol/${file}.valid.json`, 'utf8'))
    assert.equal(await validateFixture(value, definition), true, file)
    assert.equal(await validateFixture({ ...value, outcomes: [] }, definition), false, `${file}: unexpected outcomes`)
    assert.equal(await validateFixture({ ...value, protocolVersion: 'viewer.review/99' }, definition), false, `${file}: future major`)
  }
  const definition = await schema('viewer-review-read-result-v3.schema.json')
  const value = JSON.parse(await readFile('tests/fixtures/review-protocol/review-read-result-v3.valid.json', 'utf8'))
  assert.equal(await validateFixture({ ...value, role: 'history' }, definition), false)
})

test('v4 adds the complete image markup anchor set without weakening v3', async () => {
  const [stateV3, stateV4] = await Promise.all([
    schema('viewer-review-state-v3.schema.json'),
    schema('viewer-review-state-v4.schema.json'),
  ])
  const kinds = stateV4.$defs.anchor.oneOf.map(entry => entry.properties.kind.const)

  assert.deepEqual(kinds, [
    'asset', 'imagePoint', 'imageArrow', 'imageStroke',
    'imageRect', 'imageEllipse', 'videoPoint', 'videoRange',
  ])
  assert.equal(
    stateV3.$defs.anchor.oneOf.some(entry => entry.properties.kind.const === 'imagePoint'),
    false,
  )

  const legacyState = JSON.parse(
    await readFile('tests/fixtures/review-protocol/review-state-v3.valid.json', 'utf8'),
  )
  const v4State = structuredClone(legacyState)
  v4State.protocolVersion = 'viewer.review/4'
  v4State.feedback[0].targets[0].anchor = {
    kind: 'imageArrow',
    tail: { x: 0.1, y: 0.2 },
    head: { x: 0.8, y: 0.7 },
  }
  assert.equal(await validateFixture(v4State, stateV4), true)
  assert.equal(await validateFixture(v4State, stateV3), false)

  const malformedArrow = structuredClone(v4State)
  malformedArrow.feedback[0].targets[0].anchor.viewportX = 20
  assert.equal(await validateFixture(malformedArrow, stateV4), false)

  for (const [file, schemaName] of [
    ['review-index-v3', 'viewer-review-index-v4'],
    ['review-archive-v3', 'viewer-review-archive-v4'],
    ['review-read-result-v3', 'viewer-review-read-result-v4'],
  ]) {
    const value = JSON.parse(
      await readFile(`tests/fixtures/review-protocol/${file}.valid.json`, 'utf8'),
    )
    value.protocolVersion = 'viewer.review/4'
    assert.equal(
      await validateFixture(value, await schema(`${schemaName}.schema.json`)),
      true,
      schemaName,
    )
  }
})

test('owned v3 cases have real content-addressed references and clean up without changing legacy fixtures', async () => {
  const legacyPath = 'tests/fixtures/review-protocol/project-v2-mixed/.viewer/reviews/index.json'
  const legacyBefore = await readFile(legacyPath)
  for (const name of ['current_nonempty', 'current_empty', 'partial_archive', 'later_edit', 'pending_source', 'legacy_mixed']) {
    const fixture = await createProjectV3Case(name)
    try {
      const base = `${fixture.projectRoot}/.viewer/reviews`
      const index = JSON.parse(await readFile(`${base}/index.json`, 'utf8'))
      assert.equal(await validateFixture(index, await schema('viewer-review-index-v3.schema.json')), true, name)
      const stream = index.streams.find(item => item.reviewStreamId === fixture.streamId)
      let reference = stream.currentRef
      let nodes = 0
      while (reference) {
        const bytes = await readFile(`${base}/states/${reference.snapshotId}.json`)
        assert.equal(blake3Hex(bytes), reference.blake3)
        const state = JSON.parse(bytes)
        assert.equal(await validateFixture(state, await schema('viewer-review-state-v3.schema.json')), true, name)
        for (const binding of state.evidence) {
          if (binding.capability.kind !== 'image') continue
          const png = await readFile(`${base}/evidence/${binding.capability.base.blake3}.png`)
          assert.equal(blake3Hex(png), binding.capability.base.blake3)
        }
        if (nodes === 0 && name === 'current_empty') assert.deepEqual(state.feedback, [])
        reference = state.parent
        assert.ok(++nodes <= 3)
      }
      assert.equal(nodes, fixture.snapshotIds.length)
      if (name === 'legacy_mixed') {
        for (const legacy of index.streams.flatMap(item => item.legacyRefs)) {
          assert.equal(blake3Hex(await readFile(`${base}/${legacy.location}`)), legacy.blake3)
        }
      }
    } finally { await fixture.cleanup() }
    await assert.rejects(stat(fixture.projectRoot), { code: 'ENOENT' })
  }
  assert.deepEqual(await readFile(legacyPath), legacyBefore)
  await assert.rejects(createProjectV3Case('not-a-case'))
})
