import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { constants } from 'node:fs'
import { access, cp, lstat, mkdtemp, open, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import {
  listReviewStreams,
  readLatestCompletedReview,
} from './read-latest.mjs'

const fixtureProject = fileURLToPath(
  new URL('../../tests/fixtures/review-protocol/project', import.meta.url),
)
const reader = fileURLToPath(new URL('./read-latest.mjs', import.meta.url))
const STREAM_B = '00000000-0000-4000-8000-000000000102'
const ROUND_B = '00000000-0000-4000-8000-000000000202'

async function disposableProject() {
  const directory = await mkdtemp(path.join(tmpdir(), 'viewer-review-reader-'))
  const project = path.join(directory, 'project')
  await cp(fixtureProject, project, { recursive: true })
  return {
    project,
    async [Symbol.asyncDispose]() {
      await rm(directory, { recursive: true, force: true })
    },
  }
}

async function mutateJson(file, mutation) {
  const document = JSON.parse(await readFile(file, 'utf8'))
  mutation(document)
  await writeFile(file, `${JSON.stringify(document, null, 2)}\n`)
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
    /anchor/i,
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
      /symbolic link/i,
    )
  }
})

test('rejects oversized index and round files before parsing', async () => {
  for (const [relative, bytes, message] of [
    ['.viewer/reviews/index.json', 16 * 1024 * 1024 + 1, /index.*size limit/i],
    [`.viewer/reviews/rounds/${ROUND_B}.json`, 64 * 1024 * 1024 + 1, /round.*size limit/i],
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
    document.protocolVersion = 'viewer.review/2'
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
    /multiple review streams match task and batch/i,
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
