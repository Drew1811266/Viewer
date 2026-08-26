import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

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
