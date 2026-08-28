import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { constants } from 'node:fs'
import { access, cp, lstat, mkdtemp, open, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import {
  blake3Hex,
  listReviewStreams,
  readLatestCompletedReview,
} from './read-latest.mjs'

const fixtureProject = fileURLToPath(
  new URL('../../tests/fixtures/review-protocol/project', import.meta.url),
)
const mixedFixtureProject = fileURLToPath(
  new URL('../../tests/fixtures/review-protocol/project-v2-mixed', import.meta.url),
)
const reader = fileURLToPath(new URL('./read-latest.mjs', import.meta.url))
const STREAM_B = '00000000-0000-4000-8000-000000000102'
const ROUND_B = '00000000-0000-4000-8000-000000000202'
const ROUND_V2 = '00000000-0000-4000-8000-000000000204'
const ARTIFACT_V2 = '00000000-0000-4000-8000-000000000301-annotation.png'

async function disposableProject(source = fixtureProject) {
  const directory = await mkdtemp(path.join(tmpdir(), 'viewer-review-reader-'))
  const project = path.join(directory, 'project')
  await cp(source, project, { recursive: true })
  return {
    project,
    async [Symbol.asyncDispose]() {
      await rm(directory, { recursive: true, force: true })
    },
  }
}

function mixedIndex(project) {
  return path.join(project, '.viewer/reviews/index.json')
}

function mixedBundle(project) {
  return path.join(project, `.viewer/reviews/rounds/${ROUND_V2}`)
}

function mixedManifest(project) {
  return path.join(mixedBundle(project), 'round.json')
}

function mixedArtifact(project) {
  return path.join(mixedBundle(project), 'artifacts', ARTIFACT_V2)
}

test('BLAKE3 verification matches the canonical empty and abc vectors', () => {
  assert.equal(
    blake3Hex(Buffer.alloc(0)),
    'af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262',
  )
  assert.equal(
    blake3Hex(Buffer.from('abc')),
    '6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85',
  )
})

test('BLAKE3 verification matches official block, chunk, and tree-boundary vectors', () => {
  // The input pattern and 32-byte prefixes come from BLAKE3-team/BLAKE3 test_vectors.
  for (const [length, expected] of [
    [64, '4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98'],
    [65, 'de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee'],
    [1024, '42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7'],
    [1025, 'd00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444'],
    [2048, 'e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a'],
    [3072, 'b98cb0ff3623be03326b373de6b9095218513e64f1ee2edd2525c7ad1e5cffd2'],
    [5121, '628bd2cb2004694adaab7bbd778a25df25c47b9d4155a55f8fbd79f2fe154cff'],
  ]) {
    const input = Buffer.from(Array.from({ length }, (_, index) => index % 251))
    assert.equal(blake3Hex(input), expected, `input length ${length}`)
  }
})

async function mutateJson(file, mutation) {
  const document = JSON.parse(await readFile(file, 'utf8'))
  mutation(document)
  await writeFile(file, `${JSON.stringify(document, null, 2)}\n`)
}

async function mutateMixedManifest(project, mutation) {
  await mutateJson(mixedManifest(project), mutation)
  const digest = blake3Hex(await readFile(mixedManifest(project)))
  await mutateJson(mixedIndex(project), (document) => {
    document.streams[0].completedRounds[1].blake3 = digest
  })
}

async function makeOversize(file, bytes) {
  const handle = await open(file, 'w')
  try {
    await handle.truncate(bytes)
  } finally {
    await handle.close()
  }
}

test('reads the selected Stream head and ignores a newer Draft', async () => {
  const round = await readLatestCompletedReview({
    projectRoot: fixtureProject,
    reviewStreamId: STREAM_B,
  })

  assert.equal(round.status, 'completed')
  assert.equal(round.reviewRoundId, ROUND_B)
  assert.notEqual(round.reviewRoundId, '00000000-0000-4000-8000-000000000203')
})

test('reads mixed v1 and v2 history and verifies the indexed bundle', async () => {
  const result = await readLatestCompletedReview({ projectRoot: mixedFixtureProject })

  assert.equal(result.protocolVersion, 'viewer.review/2')
  assert.equal(result.reviewRoundId, ROUND_V2)
  assert.equal(result.feedback[0].text, '右手结构需要修正')
  assert.equal(result.feedback[0].targets[0].anchor.kind, 'imageStroke')
  assert.match(result.artifacts[0].relativePath, /^artifacts\/[0-9a-f-]+-annotation\.png$/)
  assert.match(result.artifacts[0].blake3, /^[0-9a-f]{64}$/)
})

test('dispatches a legacy v1 Round recorded by a v2 catalog', async () => {
  await using fixture = await disposableProject(mixedFixtureProject)
  await mutateJson(mixedIndex(fixture.project), (document) => {
    document.streams[0].completedRounds.reverse()
    document.streams[0].latestCompletedRoundId = ROUND_B
  })

  const result = await readLatestCompletedReview({ projectRoot: fixture.project })

  assert.equal(result.protocolVersion, 'viewer.review/1')
  assert.equal(result.reviewRoundId, ROUND_B)
})

test('rejects a v2 catalog digest mismatch before parsing the manifest', async () => {
  await using fixture = await disposableProject(mixedFixtureProject)
  await mutateJson(mixedIndex(fixture.project), (document) => {
    document.streams[0].completedRounds[1].blake3 = 'f'.repeat(64)
  })
  await writeFile(mixedManifest(fixture.project), 'not JSON')

  await assert.rejects(
    readLatestCompletedReview({ projectRoot: fixture.project }),
    /digest/i,
  )
})

test('rejects mismatched v2 manifest identity and escaping catalog locations', async () => {
  await using identity = await disposableProject(mixedFixtureProject)
  const replacement = '00000000-0000-4000-8000-000000000205'
  await cp(
    mixedBundle(identity.project),
    path.join(identity.project, `.viewer/reviews/rounds/${replacement}`),
    { recursive: true },
  )
  await mutateJson(mixedIndex(identity.project), (document) => {
    document.streams[0].completedRounds[1].reviewRoundId = replacement
    document.streams[0].completedRounds[1].location = `rounds/${replacement}/round.json`
    document.streams[0].latestCompletedRoundId = replacement
  })
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: identity.project }),
    /round identity/i,
  )

  await using escaping = await disposableProject(mixedFixtureProject)
  await mutateJson(mixedIndex(escaping.project), (document) => {
    document.streams[0].completedRounds[1].location = 'rounds/../outside/round.json'
  })
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: escaping.project }),
    { code: 'integrity' },
  )
})

test('rejects symlinked v2 bundle components and artifacts', async () => {
  await using bundleFixture = await disposableProject(mixedFixtureProject)
  const bundle = mixedBundle(bundleFixture.project)
  const outsideBundle = `${bundle}.outside`
  await cp(bundle, outsideBundle, { recursive: true })
  await rm(bundle, { recursive: true })
  await symlink(outsideBundle, bundle)
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: bundleFixture.project }),
    { code: 'integrity' },
  )

  await using artifactFixture = await disposableProject(mixedFixtureProject)
  const artifact = mixedArtifact(artifactFixture.project)
  const outsideArtifact = `${artifact}.outside`
  await writeFile(outsideArtifact, await readFile(artifact))
  await rm(artifact)
  await symlink(outsideArtifact, artifact)
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: artifactFixture.project }),
    { code: 'integrity' },
  )
})

test('rejects missing and extra undeclared v2 artifacts', async () => {
  await using missing = await disposableProject(mixedFixtureProject)
  await rm(mixedArtifact(missing.project))
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: missing.project }),
    { code: 'integrity' },
  )

  await using extra = await disposableProject(mixedFixtureProject)
  await writeFile(path.join(mixedBundle(extra.project), 'artifacts', 'undeclared.png'), 'extra')
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: extra.project }),
    { code: 'integrity' },
  )
})

test('rejects oversized v2 JSON and PNG before decoding them', async () => {
  for (const [target, message] of [
    [mixedManifest, { code: 'limit_exceeded' }],
    [mixedArtifact, { code: 'limit_exceeded' }],
  ]) {
    await using fixture = await disposableProject(mixedFixtureProject)
    await makeOversize(target(fixture.project), 64 * 1024 * 1024 + 1)
    await assert.rejects(
      readLatestCompletedReview({ projectRoot: fixture.project }),
      message,
    )
  }
})

test('rejects malformed v2 anchors and artifact metadata after verifying the manifest', async () => {
  for (const [mutation, message] of [
    [(document) => { document.feedback[0].targets[0].anchor.kind = 'imageRegion' }, { code: 'integrity' }],
    [(document) => {
      document.feedback[0].targets[0].anchor.points = [{ x: 0.2, y: 0.2 }, { x: 0.2, y: 0.3 }]
    }, { code: 'integrity' }],
    [(document) => {
      document.feedback[0].targets[0].anchor.points = Array.from({ length: 2049 }, () => ({ x: 0.2, y: 0.3 }))
    }, { code: 'limit_exceeded' }],
    [(document) => { document.artifacts[0].relativePath = 'artifacts/../outside.png' }, { code: 'integrity' }],
    [(document) => { document.artifacts[0].relativePath = `artifacts/nested/${ARTIFACT_V2}` }, { code: 'integrity' }],
    [(document) => { document.artifacts[0].width = 4_294_967_295 }, { code: 'integrity' }],
    [(document) => { document.artifacts[0].mediaType = 'image/jpeg' }, { code: 'integrity' }],
    [(document) => { document.artifacts[0].annotations[1].ordinal = 3 }, { code: 'integrity' }],
    [(document) => { document.artifacts[0].annotations[0].ordinal = 2; document.artifacts[0].annotations[1].ordinal = 1 }, { code: 'integrity' }],
    [(document) => { document.artifacts[0].annotations.pop() }, { code: 'integrity' }],
    [(document) => { document.artifacts = [] }, { code: 'integrity' }],
    [(document) => { document.artifacts[0].width += 1 }, { code: 'integrity' }],
    [(document) => { document.artifacts[0].blake3 = '0'.repeat(64) }, { code: 'integrity' }],
  ]) {
    await using fixture = await disposableProject(mixedFixtureProject)
    await mutateMixedManifest(fixture.project, mutation)
    await assert.rejects(readLatestCompletedReview({ projectRoot: fixture.project }), message)
  }
})

test('rejects artifact corruption and a digest-valid non-PNG artifact', async () => {
  await using corrupt = await disposableProject(mixedFixtureProject)
  await writeFile(mixedArtifact(corrupt.project), 'corrupt artifact')
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: corrupt.project }),
    { code: 'integrity' },
  )

  await using notPng = await disposableProject(mixedFixtureProject)
  const bytes = Buffer.from('not a PNG even when its digest is correct')
  await writeFile(mixedArtifact(notPng.project), bytes)
  await mutateMixedManifest(notPng.project, (document) => {
    document.artifacts[0].blake3 = blake3Hex(bytes)
  })
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: notPng.project }),
    { code: 'integrity' },
  )
})

test('rejects v2 outcomes that omit the feedback attached to their asset', async () => {
  for (const mutation of [
    (document) => { document.outcomes[0].feedbackIds.pop() },
    (document) => { document.outcomes[0].kind = 'pass'; document.outcomes[0].feedbackIds = [] },
  ]) {
    await using fixture = await disposableProject(mixedFixtureProject)
    await mutateMixedManifest(fixture.project, mutation)
    await assert.rejects(
      readLatestCompletedReview({ projectRoot: fixture.project }),
      { code: 'integrity' },
    )
  }
})

test('enforces the domain-wide 200000 stroke-point limit before returning a Round', async () => {
  await using fixture = await disposableProject(mixedFixtureProject)
  await mutateMixedManifest(fixture.project, (document) => {
    const template = document.feedback[0]
    const strokes = Array.from({ length: 98 }, (_, index) => ({
      ...structuredClone(template),
      feedbackId: `00000000-0000-4000-8000-${(10000 + index).toString(16).padStart(12, '0')}`,
      createdAtMs: 1100 + index,
    }))
    for (const feedback of strokes) {
      feedback.targets[0].anchor.points = Array.from({ length: 2048 }, (_, index) => ({
        x: 0.1 + (index % 2) * 0.2,
        y: 0.1 + (index % 3) * 0.2,
      }))
    }
    document.feedback = [...strokes, ...document.feedback.slice(2)]
    document.outcomes[0].feedbackIds = strokes.map((feedback) => feedback.feedbackId)
    document.artifacts[0].annotations = strokes.map((feedback, index) => ({
      ordinal: index + 1,
      feedbackId: feedback.feedbackId,
    }))
  })
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: fixture.project }),
    { code: 'limit_exceeded' },
  )
})

test('rejects unknown catalog and indexed Round protocols without guessing', async () => {
  for (const mutation of [
    (document) => { document.protocolVersion = 'viewer.review/99' },
    (document) => { document.streams[0].completedRounds[1].protocolVersion = 'viewer.review/99' },
  ]) {
    await using fixture = await disposableProject(mixedFixtureProject)
    await mutateJson(mixedIndex(fixture.project), mutation)
    await assert.rejects(
      readLatestCompletedReview({ projectRoot: fixture.project }),
      /unsupported review protocol version/i,
    )
  }
})

test('accepts optional local source identity and unavailable bounds only for unreviewable images', async () => {
  await using fixture = await disposableProject()
  const round = path.join(fixture.project, `.viewer/reviews/rounds/${ROUND_B}.json`)
  await mutateJson(round, (document) => {
    document.assets[0].sourceEntityId = '00000000-0000-4000-8000-000000000051'
    document.assets[2].media = { kind: 'image' }
    document.outcomes[2] = {
      assetVersionId: document.assets[2].assetVersionId,
      kind: 'unreviewable',
      feedbackIds: [],
      failure: 'decodeFailed',
    }
  })

  const completed = await readLatestCompletedReview({
    projectRoot: fixture.project,
    reviewStreamId: STREAM_B,
  })

  assert.equal(completed.assets[0].sourceEntityId, '00000000-0000-4000-8000-000000000051')
  assert.deepEqual(completed.assets[2].media, { kind: 'image' })
})

test('rejects invalid source identity, partial image bounds, and unavailable bounds on pass', async () => {
  for (const mutate of [
    (document) => {
      document.assets[0].sourceEntityId = 'not-an-entity-id'
    },
    (document) => {
      document.assets[0].media = { kind: 'image', width: 100 }
    },
    (document) => {
      document.assets[2].media = { kind: 'image' }
    },
  ]) {
    await using fixture = await disposableProject()
    const round = path.join(fixture.project, `.viewer/reviews/rounds/${ROUND_B}.json`)
    await mutateJson(round, mutate)

    await assert.rejects(
      readLatestCompletedReview({ projectRoot: fixture.project, reviewStreamId: STREAM_B }),
      { code: 'integrity' },
    )
  }
})

test('lists Streams and selects one exact production scope', async () => {
  assert.deepEqual(await listReviewStreams({ projectRoot: fixtureProject }), [
    {
      reviewStreamId: '00000000-0000-4000-8000-000000000101',
      taskId: 'task-a',
      batchId: 'batch-a',
      latestCompletedRoundId: '00000000-0000-4000-8000-000000000201',
    },
    {
      reviewStreamId: STREAM_B,
      taskId: 'task-b',
      batchId: 'batch-b',
      latestCompletedRoundId: ROUND_B,
    },
  ])
  assert.equal(
    (
      await readLatestCompletedReview({
        projectRoot: fixtureProject,
        taskId: 'task-b',
        batchId: 'batch-b',
      })
    ).reviewRoundId,
    ROUND_B,
  )
})

test('refuses to guess when multiple Streams exist', async () => {
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: fixtureProject }),
    /multiple review streams; provide --stream or --task and --batch; use --list/i,
  )
})

test('rejects an indexed round whose identities do not match', async () => {
  for (const [field, value, message] of [
    ['reviewStreamId', '00000000-0000-4000-8000-000000000999', /stream identity/i],
    ['reviewRoundId', '00000000-0000-4000-8000-000000000999', /round identity/i],
    ['projectId', '00000000-0000-4000-8000-000000000999', /project identity/i],
  ]) {
    await using fixture = await disposableProject()
    const round = path.join(fixture.project, `.viewer/reviews/rounds/${ROUND_B}.json`)
    await mutateJson(round, (document) => {
      document[field] = value
    })
    await assert.rejects(
      readLatestCompletedReview({ projectRoot: fixture.project, reviewStreamId: STREAM_B }),
      message,
    )
  }
})

test('rejects malformed nested Completed values instead of returning partially validated JSON', async () => {
  await using fixture = await disposableProject()
  const round = path.join(fixture.project, `.viewer/reviews/rounds/${ROUND_B}.json`)
  await mutateJson(round, (document) => {
    document.feedback[0].targets[1].anchor.x = '0.6'
  })

  await assert.rejects(
    readLatestCompletedReview({ projectRoot: fixture.project, reviewStreamId: STREAM_B }),
    { code: 'integrity' },
  )
})

test('rejects symlinked index and round files', async () => {
  for (const target of ['index', 'round']) {
    await using fixture = await disposableProject()
    const file =
      target === 'index'
        ? path.join(fixture.project, '.viewer/reviews/index.json')
        : path.join(fixture.project, `.viewer/reviews/rounds/${ROUND_B}.json`)
    const replacement = `${file}.outside`
    await writeFile(replacement, await readFile(file))
    await rm(file)
    await symlink(replacement, file)
    await assert.rejects(
      readLatestCompletedReview({ projectRoot: fixture.project, reviewStreamId: STREAM_B }),
      { code: 'integrity' },
    )
  }
})

test('rejects oversized index and round files before parsing', async () => {
  for (const [relative, bytes, message] of [
    ['.viewer/reviews/index.json', 16 * 1024 * 1024 + 1, { code: 'limit_exceeded' }],
    [`.viewer/reviews/rounds/${ROUND_B}.json`, 64 * 1024 * 1024 + 1, { code: 'limit_exceeded' }],
  ]) {
    await using fixture = await disposableProject()
    await makeOversize(path.join(fixture.project, relative), bytes)
    await assert.rejects(
      readLatestCompletedReview({ projectRoot: fixture.project, reviewStreamId: STREAM_B }),
      message,
    )
  }
})

test('rejects unsupported versions, missing heads, and ambiguous production selectors', async () => {
  await using unsupported = await disposableProject()
  await mutateJson(path.join(unsupported.project, '.viewer/reviews/index.json'), (document) => {
    document.protocolVersion = 'viewer.review/99'
  })
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: unsupported.project, reviewStreamId: STREAM_B }),
    /unsupported review protocol version/i,
  )

  await using missingHead = await disposableProject()
  await mutateJson(path.join(missingHead.project, '.viewer/reviews/index.json'), (document) => {
    document.streams[1].completedRoundIds = []
    document.streams[1].latestCompletedRoundId = null
  })
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: missingHead.project, reviewStreamId: STREAM_B }),
    /no completed review head/i,
  )

  await using ambiguous = await disposableProject()
  await mutateJson(path.join(ambiguous.project, '.viewer/reviews/index.json'), (document) => {
    document.streams[0].taskId = 'task-b'
    document.streams[0].batchId = 'batch-b'
  })
  await assert.rejects(
    readLatestCompletedReview({
      projectRoot: ambiguous.project,
      taskId: 'task-b',
      batchId: 'batch-b',
    }),
    { code: 'integrity' },
  )
})

test('selector arguments are exact and cannot be combined', async () => {
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: fixtureProject, taskId: 'task-b' }),
    /task and batch must be provided together/i,
  )
  await assert.rejects(
    readLatestCompletedReview({
      projectRoot: fixtureProject,
      reviewStreamId: STREAM_B,
      taskId: 'task-b',
      batchId: 'batch-b',
    }),
    /choose either review stream id or task and batch/i,
  )
})

test('CLI writes Completed JSON or a stable error with the documented exit code', async () => {
  const success = JSON.parse(
    execFileSync(process.execPath, [reader, '--project', fixtureProject, '--stream', STREAM_B], {
      encoding: 'utf8',
    }),
  )
  assert.equal(success.reviewRoundId, ROUND_B)

  const listed = JSON.parse(
    execFileSync(process.execPath, [reader, '--project', fixtureProject, '--list'], {
      encoding: 'utf8',
    }),
  )
  assert.equal(listed.length, 2)

  const ambiguous = spawnSync(process.execPath, [reader, '--project', fixtureProject], {
    encoding: 'utf8',
  })
  assert.equal(ambiguous.status, 1)
  assert.equal(ambiguous.stdout, '')
  assert.match(ambiguous.stderr, /use --list/i)
})

test('fixture Draft exists but is never opened as a completed result', async () => {
  const draft = path.join(
    fixtureProject,
    '.viewer/reviews/drafts/00000000-0000-4000-8000-000000000203.json',
  )
  await access(draft, constants.R_OK)
  const result = await readLatestCompletedReview({
    projectRoot: fixtureProject,
    reviewStreamId: STREAM_B,
  })
  assert.equal(result.status, 'completed')
  assert.equal(result.reviewRoundId, ROUND_B)
})
