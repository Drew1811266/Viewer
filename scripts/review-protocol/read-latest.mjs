#!/usr/bin/env node

import { constants } from 'node:fs'
import { lstat, open, realpath } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

const REVIEW_PROTOCOL = 'viewer.review/1'
const MAX_INDEX_BYTES = 16 * 1024 * 1024
const MAX_ROUND_BYTES = 64 * 1024 * 1024
const MAX_STREAMS = 10_000
const MAX_ROUNDS_PER_STREAM = 10_000
const MAX_ASSETS = 50_000
const MAX_FEEDBACK = 10_000
const MAX_TARGETS = 10_000
const MAX_FEEDBACK_TEXT_BYTES = 65_536
const MAX_U32 = 4_294_967_295
const reviewabilityFailures = new Set([
  'unsupported', 'damaged', 'unreadable', 'permissionDenied', 'missing', 'decodeFailed',
])
const uuidPattern = /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/
const productionIdPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/

export async function listReviewStreams({ projectRoot } = {}) {
  const { catalog } = await readCatalog(projectRoot)
  return catalog.streams.map((stream) => ({
    reviewStreamId: stream.reviewStreamId,
    ...(stream.taskId === undefined ? {} : { taskId: stream.taskId, batchId: stream.batchId }),
    latestCompletedRoundId: stream.latestCompletedRoundId,
  }))
}

export async function readLatestCompletedReview({
  projectRoot,
  reviewStreamId,
  taskId,
  batchId,
} = {}) {
  validateSelector({ reviewStreamId, taskId, batchId })
  const { reviewsRoot, catalog } = await readCatalog(projectRoot)
  const selected = selectStream(catalog.streams, { reviewStreamId, taskId, batchId })
  if (selected.latestCompletedRoundId === null) {
    throw new Error('selected review stream has no completed review head')
  }

  const roundId = selected.latestCompletedRoundId
  const roundPath = path.join(reviewsRoot, 'rounds', `${roundId}.json`)
  const round = parseJson(
    await readBoundedFile(roundPath, MAX_ROUND_BYTES, 'completed round'),
    'completed round',
  )
  validateCompletedRound(round)
  if (round.projectId !== catalog.projectId) {
    throw new Error('completed round project identity does not match the review index')
  }
  if (round.reviewStreamId !== selected.reviewStreamId) {
    throw new Error('completed round stream identity does not match the selected review stream')
  }
  if (round.reviewRoundId !== roundId) {
    throw new Error('completed round identity does not match the indexed round filename')
  }
  if ((round.taskId ?? null) !== (selected.taskId ?? null)
      || (round.batchId ?? null) !== (selected.batchId ?? null)) {
    throw new Error('completed round production identity does not match the selected review stream')
  }
  return round
}

function validateSelector({ reviewStreamId, taskId, batchId }) {
  const hasStream = reviewStreamId !== undefined
  const hasTask = taskId !== undefined
  const hasBatch = batchId !== undefined
  if (hasTask !== hasBatch) throw new Error('task and batch must be provided together')
  if (hasStream && hasTask) {
    throw new Error('choose either review stream id or task and batch')
  }
  if (hasStream) requireUuid(reviewStreamId, 'review stream id')
  if (hasTask) {
    requireProductionId(taskId, 'task id')
    requireProductionId(batchId, 'batch id')
  }
}

function selectStream(streams, selector) {
  let matches
  if (selector.reviewStreamId !== undefined) {
    matches = streams.filter((stream) => stream.reviewStreamId === selector.reviewStreamId)
    if (matches.length === 0) throw new Error('review stream was not found')
    if (matches.length > 1) throw new Error('multiple review streams use the requested stream id')
  } else if (selector.taskId !== undefined) {
    matches = streams.filter(
      (stream) => stream.taskId === selector.taskId && stream.batchId === selector.batchId,
    )
    if (matches.length === 0) throw new Error('review stream for task and batch was not found')
    if (matches.length > 1) throw new Error('multiple review streams match task and batch')
  } else {
    if (streams.length === 0) throw new Error('no review streams are available')
    if (streams.length > 1) {
      throw new Error(
        'multiple review streams; provide --stream or --task and --batch; use --list to inspect streams',
      )
    }
    matches = streams
  }
  return matches[0]
}

async function readCatalog(projectRoot) {
  const root = await resolveProjectRoot(projectRoot)
  const viewerRoot = path.join(root, '.viewer')
  const reviewsRoot = path.join(viewerRoot, 'reviews')
  await requireSafeDirectory(viewerRoot, '.viewer directory')
  await requireSafeDirectory(reviewsRoot, 'review repository directory')
  const catalog = parseJson(
    await readBoundedFile(path.join(reviewsRoot, 'index.json'), MAX_INDEX_BYTES, 'review index'),
    'review index',
  )
  validateCatalog(catalog)
  return { reviewsRoot, catalog }
}

async function resolveProjectRoot(projectRoot) {
  if (typeof projectRoot !== 'string' || projectRoot.length === 0) {
    throw new Error('project root is required')
  }
  const candidate = path.resolve(projectRoot)
  await requireSafeDirectory(candidate, 'project root')
  const resolved = await realpath(candidate)
  await requireSafeDirectory(resolved, 'project root')
  return resolved
}

async function requireSafeDirectory(directory, label) {
  let metadata
  try {
    metadata = await lstat(directory)
  } catch {
    throw new Error(`${label} is unavailable`)
  }
  if (metadata.isSymbolicLink()) throw new Error(`${label} cannot be a symbolic link`)
  if (!metadata.isDirectory()) throw new Error(`${label} must be a directory`)
}

async function readBoundedFile(file, maximumBytes, label) {
  let metadata
  try {
    metadata = await lstat(file)
  } catch {
    throw new Error(`${label} is unavailable`)
  }
  if (metadata.isSymbolicLink()) throw new Error(`${label} cannot be a symbolic link`)
  if (!metadata.isFile()) throw new Error(`${label} must be a regular file`)
  if (metadata.size > maximumBytes) throw new Error(`${label} exceeds its size limit`)

  let handle
  try {
    handle = await open(file, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0))
    const openedMetadata = await handle.stat()
    if (!openedMetadata.isFile()) throw new Error(`${label} must be a regular file`)
    if (openedMetadata.size > maximumBytes) throw new Error(`${label} exceeds its size limit`)
    const bytes = await handle.readFile()
    if (bytes.length > maximumBytes) throw new Error(`${label} exceeds its size limit`)
    return bytes
  } catch (error) {
    if (error instanceof Error && error.message.startsWith(label)) throw error
    if (error?.code === 'ELOOP') throw new Error(`${label} cannot be a symbolic link`)
    throw new Error(`${label} is unavailable`)
  } finally {
    await handle?.close()
  }
}

function parseJson(bytes, label) {
  try {
    return JSON.parse(bytes.toString('utf8'))
  } catch {
    throw new Error(`${label} is not valid JSON`)
  }
}

function validateCatalog(catalog) {
  requireObject(catalog, 'review index')
  requireExactKeys(catalog, ['protocolVersion', 'projectId', 'streams'], 'review index')
  if (catalog.protocolVersion !== REVIEW_PROTOCOL) {
    throw new Error('unsupported review protocol version; repository remains read-only')
  }
  requireUuid(catalog.projectId, 'review index project id')
  requireArray(catalog.streams, 'review index streams', 0, MAX_STREAMS)
  const streamIds = new Set()
  for (const [index, stream] of catalog.streams.entries()) {
    const label = `review stream ${index}`
    requireObject(stream, label)
    requireExactKeys(
      stream,
      ['reviewStreamId', 'taskId', 'batchId', 'completedRoundIds', 'latestCompletedRoundId'],
      label,
      ['taskId', 'batchId'],
    )
    requireUuid(stream.reviewStreamId, `${label} id`)
    if (streamIds.has(stream.reviewStreamId)) throw new Error('review index has duplicate stream identities')
    streamIds.add(stream.reviewStreamId)
    const hasTask = stream.taskId !== undefined
    if (hasTask !== (stream.batchId !== undefined)) {
      throw new Error(`${label} task and batch must occur together`)
    }
    if (hasTask) {
      requireProductionId(stream.taskId, `${label} task id`)
      requireProductionId(stream.batchId, `${label} batch id`)
    }
    requireArray(stream.completedRoundIds, `${label} completed round ids`, 0, MAX_ROUNDS_PER_STREAM)
    const roundIds = new Set()
    for (const roundId of stream.completedRoundIds) {
      requireUuid(roundId, `${label} completed round id`)
      if (roundIds.has(roundId)) throw new Error(`${label} has duplicate completed round ids`)
      roundIds.add(roundId)
    }
    if (stream.latestCompletedRoundId !== null) {
      requireUuid(stream.latestCompletedRoundId, `${label} latest completed round id`)
    }
    const expectedHead = stream.completedRoundIds.at(-1) ?? null
    if (stream.latestCompletedRoundId !== expectedHead) {
      throw new Error(`${label} completed history and head are inconsistent`)
    }
  }
}

function validateCompletedRound(round) {
  requireObject(round, 'completed round')
  requireExactKeys(
    round,
    [
      'protocolVersion', 'status', 'projectId', 'reviewStreamId', 'reviewRoundId', 'taskId',
      'batchId', 'previousCompletedRoundId', 'createdAtMs', 'completedAtMs', 'assets',
      'feedback', 'outcomes',
    ],
    'completed round',
    ['taskId', 'batchId', 'previousCompletedRoundId'],
  )
  if (round.protocolVersion !== REVIEW_PROTOCOL) {
    throw new Error('unsupported review protocol version; repository remains read-only')
  }
  if (round.status !== 'completed') throw new Error('indexed review round is not completed')
  requireUuid(round.projectId, 'completed round project id')
  requireUuid(round.reviewStreamId, 'completed round stream id')
  requireUuid(round.reviewRoundId, 'completed round id')
  const hasTask = round.taskId !== undefined
  if (hasTask !== (round.batchId !== undefined)) {
    throw new Error('completed round task and batch must occur together')
  }
  if (hasTask) {
    requireProductionId(round.taskId, 'completed round task id')
    requireProductionId(round.batchId, 'completed round batch id')
  }
  if (round.previousCompletedRoundId !== undefined) {
    requireUuid(round.previousCompletedRoundId, 'previous completed round id')
  }
  requireTimestamp(round.createdAtMs, 'completed round creation time')
  requireTimestamp(round.completedAtMs, 'completed round completion time')
  if (round.completedAtMs < round.createdAtMs) throw new Error('completed round timestamps are invalid')
  const assets = validateAssets(round.assets)
  const feedback = validateFeedback(
    round.feedback,
    assets,
    round.createdAtMs,
    round.completedAtMs,
  )
  validateOutcomes(round.outcomes, round.assets, feedback)
}

function validateAssets(assets) {
  requireArray(assets, 'completed round assets', 1, MAX_ASSETS)
  const ids = new Set()
  const paths = new Set()
  for (const [index, asset] of assets.entries()) {
    const label = `asset ${index}`
    requireObject(asset, label)
    requireExactKeys(
      asset,
      [
        'assetVersionId', 'sourceEntityId', 'relativePath', 'evidence', 'media', 'producerAssetId',
        'parentAssetVersionId',
      ],
      label,
      ['sourceEntityId', 'producerAssetId', 'parentAssetVersionId'],
    )
    requireUuid(asset.assetVersionId, `${label} id`)
    if (asset.sourceEntityId !== undefined) {
      requireUuid(asset.sourceEntityId, `${label} source entity id`)
    }
    if (ids.has(asset.assetVersionId)) throw new Error('completed round has duplicate asset identities')
    ids.add(asset.assetVersionId)
    if (typeof asset.relativePath !== 'string' || !isSafeRelativePath(asset.relativePath)) {
      throw new Error(`${label} relative path is invalid`)
    }
    if (paths.has(asset.relativePath)) throw new Error('completed round has duplicate asset paths')
    paths.add(asset.relativePath)
    requireObject(asset.evidence, `${label} evidence`)
    requireExactKeys(
      asset.evidence,
      ['sizeBytes', 'modifiedNs', 'blake3'],
      `${label} evidence`,
      ['blake3'],
    )
    requireNonnegativeInteger(asset.evidence.sizeBytes, `${label} size`)
    if (typeof asset.evidence.modifiedNs !== 'string'
        || !/^-?(?:0|[1-9][0-9]*)$/.test(asset.evidence.modifiedNs)) {
      throw new Error(`${label} modified time is invalid`)
    }
    if (asset.evidence.blake3 !== undefined
        && (typeof asset.evidence.blake3 !== 'string' || !/^[0-9a-f]{64}$/.test(asset.evidence.blake3))) {
      throw new Error(`${label} digest is invalid`)
    }
    requireObject(asset.media, `${label} media`)
    if (asset.media.kind === 'image') {
      requireExactKeys(
        asset.media,
        ['kind', 'width', 'height'],
        `${label} media`,
        ['width', 'height'],
      )
      const hasWidth = asset.media.width !== undefined
      if (hasWidth !== (asset.media.height !== undefined)) {
        throw new Error(`${label} image dimensions are incomplete`)
      }
      if (hasWidth) {
        requirePositiveInteger(asset.media.width, `${label} image width`, MAX_U32)
        requirePositiveInteger(asset.media.height, `${label} image height`, MAX_U32)
      }
    } else if (asset.media.kind === 'video') {
      requireExactKeys(
        asset.media,
        ['kind', 'durationUs', 'displayWidth', 'displayHeight'],
        `${label} media`,
        ['durationUs', 'displayWidth', 'displayHeight'],
      )
      if (asset.media.durationUs !== undefined) requirePositiveInteger(asset.media.durationUs, `${label} duration`)
      const hasWidth = asset.media.displayWidth !== undefined
      if (hasWidth !== (asset.media.displayHeight !== undefined)) {
        throw new Error(`${label} video dimensions are incomplete`)
      }
      if (hasWidth) {
        requirePositiveInteger(asset.media.displayWidth, `${label} video width`, MAX_U32)
        requirePositiveInteger(asset.media.displayHeight, `${label} video height`, MAX_U32)
      }
    } else {
      throw new Error(`${label} media kind is invalid`)
    }
    if (asset.producerAssetId !== undefined) {
      requireProductionId(asset.producerAssetId, `${label} producer asset id`)
    }
    if (asset.parentAssetVersionId !== undefined) {
      requireUuid(asset.parentAssetVersionId, `${label} parent asset id`)
      if (asset.parentAssetVersionId === asset.assetVersionId) {
        throw new Error(`${label} cannot be its own parent`)
      }
    }
  }
  return new Map(assets.map((asset) => [asset.assetVersionId, asset]))
}

function validateFeedback(feedback, assets, roundCreatedAtMs, roundCompletedAtMs) {
  requireArray(feedback, 'completed round feedback', 0, MAX_FEEDBACK)
  const feedbackById = new Map()
  for (const [index, item] of feedback.entries()) {
    const label = `feedback ${index}`
    requireObject(item, label)
    requireExactKeys(item, ['feedbackId', 'text', 'createdAtMs', 'targets'], label)
    requireUuid(item.feedbackId, `${label} id`)
    if (feedbackById.has(item.feedbackId)) throw new Error('completed round has duplicate feedback identities')
    if (typeof item.text !== 'string' || item.text.trim().length === 0
        || Buffer.byteLength(item.text, 'utf8') > MAX_FEEDBACK_TEXT_BYTES) {
      throw new Error(`${label} text is invalid`)
    }
    requireTimestamp(item.createdAtMs, `${label} creation time`)
    if (item.createdAtMs < roundCreatedAtMs || item.createdAtMs > roundCompletedAtMs) {
      throw new Error(`${label} creation time is outside the completed round`)
    }
    requireArray(item.targets, `${label} targets`, 1, MAX_TARGETS)
    const targetKeys = new Set()
    for (const target of item.targets) {
      requireObject(target, `${label} target`)
      requireExactKeys(target, ['assetVersionId', 'anchor'], `${label} target`)
      requireUuid(target.assetVersionId, `${label} target asset id`)
      const asset = assets.get(target.assetVersionId)
      if (!asset) throw new Error(`${label} targets an unknown asset`)
      requireObject(target.anchor, `${label} target anchor`)
      validateAnchor(target.anchor, asset.media, `${label} target anchor`)
      const targetKey = JSON.stringify(target)
      if (targetKeys.has(targetKey)) throw new Error(`${label} has duplicate targets`)
      targetKeys.add(targetKey)
    }
    feedbackById.set(item.feedbackId, item)
  }
  return feedbackById
}

function validateAnchor(anchor, media, label) {
  if (anchor.kind === 'asset') {
    requireExactKeys(anchor, ['kind'], label)
  } else if (anchor.kind === 'imageRegion') {
    requireExactKeys(anchor, ['kind', 'x', 'y', 'width', 'height'], label)
    if (media.kind !== 'image') throw new Error(`${label} does not match image media`)
    for (const key of ['x', 'y', 'width', 'height']) {
      if (typeof anchor[key] !== 'number' || !Number.isFinite(anchor[key])) {
        throw new Error(`${label} ${key} is invalid`)
      }
    }
    if (anchor.x < 0 || anchor.y < 0 || anchor.width <= 0 || anchor.height <= 0
        || anchor.x + anchor.width > 1 || anchor.y + anchor.height > 1) {
      throw new Error(`${label} region is outside the image`)
    }
  } else if (anchor.kind === 'videoPoint') {
    requireExactKeys(anchor, ['kind', 'positionUs'], label)
    if (media.kind !== 'video') throw new Error(`${label} does not match video media`)
    requireNonnegativeInteger(anchor.positionUs, `${label} position`)
    if (media.durationUs !== undefined && anchor.positionUs > media.durationUs) {
      throw new Error(`${label} is outside the video`)
    }
  } else if (anchor.kind === 'videoRange') {
    requireExactKeys(anchor, ['kind', 'startUs', 'endUs'], label)
    if (media.kind !== 'video') throw new Error(`${label} does not match video media`)
    requireNonnegativeInteger(anchor.startUs, `${label} start`)
    requirePositiveInteger(anchor.endUs, `${label} end`)
    if (anchor.startUs >= anchor.endUs
        || media.durationUs !== undefined && anchor.endUs > media.durationUs) {
      throw new Error(`${label} range is invalid`)
    }
  } else {
    throw new Error(`${label} kind is invalid`)
  }
}

function validateOutcomes(outcomes, assets, feedbackById) {
  requireArray(outcomes, 'completed round outcomes', 1, MAX_ASSETS)
  if (outcomes.length !== assets.length) throw new Error('completed round outcomes do not cover every asset')
  const assetIds = new Set(assets.map((asset) => asset.assetVersionId))
  const assetsById = new Map(assets.map((asset) => [asset.assetVersionId, asset]))
  const outcomeIds = new Set()
  for (const [index, outcome] of outcomes.entries()) {
    const label = `outcome ${index}`
    requireObject(outcome, label)
    requireExactKeys(
      outcome,
      ['assetVersionId', 'kind', 'feedbackIds', 'failure'],
      label,
      outcome.kind === 'unreviewable' ? [] : ['failure'],
    )
    requireUuid(outcome.assetVersionId, `${label} asset id`)
    if (!assetIds.has(outcome.assetVersionId) || outcomeIds.has(outcome.assetVersionId)) {
      throw new Error('completed round outcome identities are invalid')
    }
    outcomeIds.add(outcome.assetVersionId)
    requireArray(outcome.feedbackIds, `${label} feedback ids`, 0, MAX_FEEDBACK)
    const uniqueFeedbackIds = new Set(outcome.feedbackIds)
    if (uniqueFeedbackIds.size !== outcome.feedbackIds.length
        || outcome.feedbackIds.some((id) => !uuidPattern.test(id) || !feedbackById.has(id))) {
      throw new Error(`${label} feedback identity is invalid`)
    }
    if (outcome.kind === 'pass') {
      if (outcome.feedbackIds.length !== 0 || outcome.failure !== undefined) throw new Error(`${label} pass payload is invalid`)
    } else if (outcome.kind === 'revise') {
      if (outcome.feedbackIds.length === 0 || outcome.failure !== undefined) throw new Error(`${label} revise payload is invalid`)
    } else if (outcome.kind === 'unreviewable') {
      if (outcome.feedbackIds.length !== 0 || !reviewabilityFailures.has(outcome.failure)) {
        throw new Error(`${label} unreviewable payload is invalid`)
      }
    } else {
      throw new Error(`${label} kind is invalid`)
    }
    const asset = assetsById.get(outcome.assetVersionId)
    if (asset.media.kind === 'image'
        && asset.media.width === undefined
        && outcome.kind !== 'unreviewable') {
      throw new Error(`${label} unavailable image bounds require an unreviewable outcome`)
    }
  }
}

function requireObject(value, label) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`)
  }
}

function requireExactKeys(value, allowed, label, optional = []) {
  const allowedSet = new Set(allowed)
  const optionalSet = new Set(optional)
  for (const key of Object.keys(value)) {
    if (!allowedSet.has(key)) throw new Error(`${label} contains an unknown field`)
  }
  for (const key of allowed) {
    if (!optionalSet.has(key) && !(key in value)) throw new Error(`${label} is missing ${key}`)
  }
}

function requireArray(value, label, minimum, maximum) {
  if (!Array.isArray(value) || value.length < minimum || value.length > maximum) {
    throw new Error(`${label} has an invalid item count`)
  }
}

function requireUuid(value, label) {
  if (typeof value !== 'string' || !uuidPattern.test(value)) throw new Error(`${label} is invalid`)
}

function requireProductionId(value, label) {
  if (typeof value !== 'string' || !productionIdPattern.test(value)) throw new Error(`${label} is invalid`)
}

function requireTimestamp(value, label) {
  requireNonnegativeInteger(value, label)
}

function requireNonnegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`${label} is invalid`)
}

function requirePositiveInteger(value, label, maximum = Number.MAX_SAFE_INTEGER) {
  if (!Number.isSafeInteger(value) || value <= 0 || value > maximum) {
    throw new Error(`${label} is invalid`)
  }
}

function isSafeRelativePath(value) {
  return value.length > 0
    && !value.startsWith('/')
    && !value.includes('\\')
    && value.split('/').every((segment) => segment !== '' && segment !== '.' && segment !== '..'
      && segment.toLowerCase() !== '.viewer')
}

function parseArguments(argumentsList) {
  const options = {}
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index]
    if (argument === '--list') {
      if (options.list) throw new Error('--list may be provided only once')
      options.list = true
      continue
    }
    const keys = new Map([
      ['--project', 'projectRoot'],
      ['--stream', 'reviewStreamId'],
      ['--task', 'taskId'],
      ['--batch', 'batchId'],
    ])
    const key = keys.get(argument)
    if (!key) throw new Error(`unknown argument: ${argument}`)
    if (options[key] !== undefined) throw new Error(`${argument} may be provided only once`)
    const value = argumentsList[index + 1]
    if (value === undefined || value.startsWith('--')) throw new Error(`${argument} requires a value`)
    options[key] = value
    index += 1
  }
  if (options.projectRoot === undefined) throw new Error('--project is required')
  if (options.list && (options.reviewStreamId || options.taskId || options.batchId)) {
    throw new Error('--list cannot be combined with a Stream selector')
  }
  return options
}

async function main(argumentsList) {
  try {
    const options = parseArguments(argumentsList)
    const result = options.list
      ? await listReviewStreams(options)
      : await readLatestCompletedReview(options)
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
    return 0
  } catch (error) {
    process.stderr.write(`error: ${error instanceof Error ? error.message : 'review reader failed'}\n`)
    return 1
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await main(process.argv.slice(2))
}
