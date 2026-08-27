#!/usr/bin/env node

import { constants } from 'node:fs'
import { lstat, open, opendir, realpath } from 'node:fs/promises'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

const REVIEW_PROTOCOL_V1 = 'viewer.review/1'
const REVIEW_PROTOCOL_V2 = 'viewer.review/2'
const supportedReviewProtocols = new Set([REVIEW_PROTOCOL_V1, REVIEW_PROTOCOL_V2])
const MAX_INDEX_BYTES = 16 * 1024 * 1024
const MAX_ROUND_BYTES = 64 * 1024 * 1024
const MAX_ARTIFACT_BYTES = 64 * 1024 * 1024
const MAX_ARTIFACT_PIXELS = 16_777_216
const MAX_BUNDLE_BYTES = 4 * 1024 * 1024 * 1024
const MAX_STREAMS = 10_000
const MAX_ROUNDS_PER_STREAM = 10_000
const MAX_ASSETS = 50_000
const MAX_FEEDBACK = 10_000
const MAX_TARGETS = 10_000
const MAX_STROKE_POINTS = 2_048
const MAX_STROKE_POINTS_PER_ROUND = 200_000
const MAX_FEEDBACK_TEXT_BYTES = 65_536
const MAX_U32 = 4_294_967_295
const reviewabilityFailures = new Set([
  'unsupported', 'damaged', 'unreadable', 'permissionDenied', 'missing', 'decodeFailed',
])
const uuidPattern = /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/
const productionIdPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/
const digestPattern = /^[0-9a-f]{64}$/
const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])

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
  const record = completedRecord(selected, roundId, catalog.protocolVersion)
  const roundPath = await resolveSafeLocation(
    reviewsRoot,
    record.location,
    'completed round',
  )
  const roundBytes = await readBoundedFile(roundPath, MAX_ROUND_BYTES, 'completed round')
  if (record.blake3 !== null && blake3Hex(roundBytes) !== record.blake3) {
    throw new Error('completed round digest does not match the review index')
  }
  const round = parseJson(roundBytes, 'completed round')
  validateCompletedRound(round, record.protocolVersion)
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
  if (record.protocolVersion === REVIEW_PROTOCOL_V2) {
    await validateArtifactFiles(path.dirname(roundPath), round.artifacts)
  }
  return round
}

function completedRecord(stream, roundId, catalogProtocol) {
  if (catalogProtocol === REVIEW_PROTOCOL_V1) {
    return {
      reviewRoundId: roundId,
      protocolVersion: REVIEW_PROTOCOL_V1,
      location: `rounds/${roundId}.json`,
      blake3: null,
    }
  }
  const record = stream.completedRounds.find((candidate) => candidate.reviewRoundId === roundId)
  if (!record) throw new Error('selected review stream head is not indexed')
  return record
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

async function resolveSafeLocation(root, relativeLocation, label) {
  if (typeof relativeLocation !== 'string' || !isSafeRepositoryLocation(relativeLocation)) {
    throw new Error(`${label} location is invalid`)
  }
  let current = root
  const components = relativeLocation.split('/')
  for (const [index, component] of components.entries()) {
    current = path.join(current, component)
    let metadata
    try {
      metadata = await lstat(current)
    } catch {
      throw new Error(`${label} is unavailable`)
    }
    if (metadata.isSymbolicLink()) {
      throw new Error(`${label} path component cannot be a symbolic link`)
    }
    if (index < components.length - 1 && !metadata.isDirectory()) {
      throw new Error(`${label} path component must be a directory`)
    }
    if (index === components.length - 1 && !metadata.isFile()) {
      throw new Error(`${label} must be a regular file`)
    }
  }
  return current
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
    const chunks = []
    let totalBytes = 0
    while (true) {
      const buffer = Buffer.allocUnsafe(Math.min(64 * 1024, maximumBytes - totalBytes + 1))
      const { bytesRead } = await handle.read(buffer, 0, buffer.length, null)
      if (bytesRead === 0) break
      totalBytes += bytesRead
      if (totalBytes > maximumBytes) throw new Error(`${label} exceeds its size limit`)
      chunks.push(buffer.subarray(0, bytesRead))
    }
    return Buffer.concat(chunks, totalBytes)
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
  if (!supportedReviewProtocols.has(catalog.protocolVersion)) {
    throw new Error('unsupported review protocol version; repository remains read-only')
  }
  requireUuid(catalog.projectId, 'review index project id')
  requireArray(catalog.streams, 'review index streams', 0, MAX_STREAMS)
  if (catalog.protocolVersion === REVIEW_PROTOCOL_V1) validateCatalogV1Streams(catalog.streams)
  else validateCatalogV2Streams(catalog.streams)
}

function validateCatalogV1Streams(streams) {
  const streamIds = new Set()
  for (const [index, stream] of streams.entries()) {
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

function validateCatalogV2Streams(streams) {
  const streamIds = new Set()
  const productionScopes = new Set()
  const roundIds = new Set()
  for (const [index, stream] of streams.entries()) {
    const label = `review stream ${index}`
    requireObject(stream, label)
    requireExactKeys(
      stream,
      ['reviewStreamId', 'taskId', 'batchId', 'completedRounds', 'latestCompletedRoundId'],
      label,
      ['taskId', 'batchId'],
    )
    requireUuid(stream.reviewStreamId, `${label} id`)
    if (streamIds.has(stream.reviewStreamId)) {
      throw new Error('review index has duplicate stream identities')
    }
    streamIds.add(stream.reviewStreamId)
    const hasTask = stream.taskId !== undefined
    if (hasTask !== (stream.batchId !== undefined)) {
      throw new Error(`${label} task and batch must occur together`)
    }
    if (hasTask) {
      requireProductionId(stream.taskId, `${label} task id`)
      requireProductionId(stream.batchId, `${label} batch id`)
      const productionKey = `${stream.taskId}\0${stream.batchId}`
      if (productionScopes.has(productionKey)) {
        throw new Error('review index has duplicate production scopes')
      }
      productionScopes.add(productionKey)
    }
    requireArray(stream.completedRounds, `${label} completed rounds`, 0, MAX_ROUNDS_PER_STREAM)
    for (const [recordIndex, record] of stream.completedRounds.entries()) {
      const recordLabel = `${label} completed round ${recordIndex}`
      requireObject(record, recordLabel)
      requireExactKeys(
        record,
        ['reviewRoundId', 'protocolVersion', 'location', 'blake3'],
        recordLabel,
      )
      requireUuid(record.reviewRoundId, `${recordLabel} id`)
      if (roundIds.has(record.reviewRoundId)) {
        throw new Error('review index has duplicate completed round identities')
      }
      roundIds.add(record.reviewRoundId)
      if (!supportedReviewProtocols.has(record.protocolVersion)) {
        throw new Error('unsupported review protocol version; repository remains read-only')
      }
      if (typeof record.blake3 !== 'string' || !digestPattern.test(record.blake3)) {
        throw new Error(`${recordLabel} digest is invalid`)
      }
      const expectedLocation = record.protocolVersion === REVIEW_PROTOCOL_V1
        ? `rounds/${record.reviewRoundId}.json`
        : `rounds/${record.reviewRoundId}/round.json`
      if (record.location !== expectedLocation || !isSafeRepositoryLocation(record.location)) {
        throw new Error(`${recordLabel} location is invalid`)
      }
    }
    if (stream.latestCompletedRoundId !== null) {
      requireUuid(stream.latestCompletedRoundId, `${label} latest completed round id`)
    }
    const expectedHead = stream.completedRounds.at(-1)?.reviewRoundId ?? null
    if (stream.latestCompletedRoundId !== expectedHead) {
      throw new Error(`${label} completed history and head are inconsistent`)
    }
  }
}

function validateCompletedRound(round, expectedProtocol) {
  requireObject(round, 'completed round')
  const isV2 = expectedProtocol === REVIEW_PROTOCOL_V2
  requireExactKeys(
    round,
    [
      'protocolVersion', 'status', 'projectId', 'reviewStreamId', 'reviewRoundId', 'taskId',
      'batchId', 'previousCompletedRoundId', 'createdAtMs', 'completedAtMs', 'assets',
      'feedback', 'outcomes', ...(isV2 ? ['artifacts'] : []),
    ],
    'completed round',
    ['taskId', 'batchId', 'previousCompletedRoundId'],
  )
  if (!supportedReviewProtocols.has(round.protocolVersion)) {
    throw new Error('unsupported review protocol version; repository remains read-only')
  }
  if (round.protocolVersion !== expectedProtocol) {
    throw new Error('completed round protocol does not match the review index')
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
    expectedProtocol,
  )
  validateOutcomes(round.outcomes, round.assets, feedback)
  if (isV2) validateArtifactRecords(round.artifacts, assets, feedback)
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

function validateFeedback(feedback, assets, roundCreatedAtMs, roundCompletedAtMs, protocolVersion) {
  requireArray(feedback, 'completed round feedback', 0, MAX_FEEDBACK)
  const feedbackById = new Map()
  let strokePoints = 0
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
      validateAnchor(target.anchor, asset.media, `${label} target anchor`, protocolVersion)
      if (target.anchor.kind === 'imageStroke') {
        strokePoints += target.anchor.points.length
        if (strokePoints > MAX_STROKE_POINTS_PER_ROUND) {
          throw new Error('completed round stroke point limit was exceeded')
        }
      }
      const targetKey = JSON.stringify(target)
      if (targetKeys.has(targetKey)) throw new Error(`${label} has duplicate targets`)
      targetKeys.add(targetKey)
    }
    feedbackById.set(item.feedbackId, item)
  }
  return feedbackById
}

function validateAnchor(anchor, media, label, protocolVersion) {
  if (anchor.kind === 'asset') {
    requireExactKeys(anchor, ['kind'], label)
  } else if (anchor.kind === 'imageRegion' && protocolVersion === REVIEW_PROTOCOL_V1
      || anchor.kind === 'imageRect' && protocolVersion === REVIEW_PROTOCOL_V2) {
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
  } else if (anchor.kind === 'imageStroke' && protocolVersion === REVIEW_PROTOCOL_V2) {
    requireExactKeys(anchor, ['kind', 'points'], label)
    if (media.kind !== 'image') throw new Error(`${label} does not match image media`)
    requireArray(anchor.points, `${label} points`, 2, MAX_STROKE_POINTS)
    let minX = Number.POSITIVE_INFINITY
    let maxX = Number.NEGATIVE_INFINITY
    let minY = Number.POSITIVE_INFINITY
    let maxY = Number.NEGATIVE_INFINITY
    for (const [index, point] of anchor.points.entries()) {
      requireObject(point, `${label} point ${index}`)
      requireExactKeys(point, ['x', 'y'], `${label} point ${index}`)
      for (const key of ['x', 'y']) {
        if (typeof point[key] !== 'number' || !Number.isFinite(point[key])
            || point[key] < 0 || point[key] > 1) {
          throw new Error(`${label} point ${key} is invalid`)
        }
      }
      minX = Math.min(minX, point.x)
      maxX = Math.max(maxX, point.x)
      minY = Math.min(minY, point.y)
      maxY = Math.max(maxY, point.y)
    }
    if (maxX <= minX || maxY <= minY) throw new Error(`${label} stroke has no area`)
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
    const expectedFeedbackIds = [...feedbackById.values()]
      .filter((feedback) => feedback.targets.some(
        (target) => target.assetVersionId === outcome.assetVersionId,
      ))
      .map((feedback) => feedback.feedbackId)
    if (expectedFeedbackIds.length !== outcome.feedbackIds.length
        || expectedFeedbackIds.some((id, feedbackIndex) => outcome.feedbackIds[feedbackIndex] !== id)) {
      throw new Error(`${label} feedback does not match the asset targets`)
    }
    if (asset.media.kind === 'image'
        && asset.media.width === undefined
        && outcome.kind !== 'unreviewable') {
      throw new Error(`${label} unavailable image bounds require an unreviewable outcome`)
    }
  }
}

function validateArtifactRecords(artifacts, assets, feedbackById) {
  requireArray(artifacts, 'completed round artifacts', 0, MAX_ASSETS)
  const expectedByAsset = new Map()
  for (const feedback of feedbackById.values()) {
    for (const target of feedback.targets) {
      if (target.anchor.kind !== 'imageRect' && target.anchor.kind !== 'imageStroke') continue
      const expected = expectedByAsset.get(target.assetVersionId) ?? new Set()
      expected.add(feedback.feedbackId)
      expectedByAsset.set(target.assetVersionId, expected)
    }
  }

  const seenAssets = new Set()
  const seenPaths = new Set()
  for (const [index, artifact] of artifacts.entries()) {
    const label = `artifact ${index}`
    requireObject(artifact, label)
    requireExactKeys(
      artifact,
      ['assetVersionId', 'relativePath', 'blake3', 'mediaType', 'width', 'height', 'annotations'],
      label,
    )
    requireUuid(artifact.assetVersionId, `${label} asset id`)
    const asset = assets.get(artifact.assetVersionId)
    if (!asset || asset.media.kind !== 'image' || seenAssets.has(artifact.assetVersionId)) {
      throw new Error(`${label} asset identity is invalid`)
    }
    seenAssets.add(artifact.assetVersionId)
    const canonicalPath = `artifacts/${artifact.assetVersionId}-annotation.png`
    if (artifact.relativePath !== canonicalPath
        || !isSafeRepositoryLocation(artifact.relativePath)
        || seenPaths.has(artifact.relativePath)) {
      throw new Error(`${label} relative path is invalid`)
    }
    seenPaths.add(artifact.relativePath)
    if (typeof artifact.blake3 !== 'string' || !digestPattern.test(artifact.blake3)) {
      throw new Error(`${label} digest is invalid`)
    }
    if (artifact.mediaType !== 'image/png') throw new Error(`${label} media type is invalid`)
    requirePositiveInteger(artifact.width, `${label} width`, MAX_U32)
    requirePositiveInteger(artifact.height, `${label} height`, MAX_U32)
    if (artifact.width * artifact.height > MAX_ARTIFACT_PIXELS) {
      throw new Error(`${label} pixel count exceeds its limit`)
    }
    requireArray(artifact.annotations, `${label} annotations`, 1, MAX_FEEDBACK)
    const expectedFeedback = expectedByAsset.get(artifact.assetVersionId) ?? new Set()
    const canonicalFeedback = [...expectedFeedback].map((id) => feedbackById.get(id))
      .sort((left, right) => left.createdAtMs - right.createdAtMs
        || (left.feedbackId < right.feedbackId ? -1 : left.feedbackId > right.feedbackId ? 1 : 0))
    const ordinals = new Set()
    const feedbackIds = new Set()
    for (const [annotationIndex, annotation] of artifact.annotations.entries()) {
      const annotationLabel = `${label} annotation ${annotationIndex}`
      requireObject(annotation, annotationLabel)
      requireExactKeys(annotation, ['ordinal', 'feedbackId'], annotationLabel)
      requirePositiveInteger(annotation.ordinal, `${annotationLabel} ordinal`, MAX_U32)
      requireUuid(annotation.feedbackId, `${annotationLabel} feedback id`)
      if (ordinals.has(annotation.ordinal) || feedbackIds.has(annotation.feedbackId)
          || !expectedFeedback.has(annotation.feedbackId)
          || canonicalFeedback[annotation.ordinal - 1]?.feedbackId !== annotation.feedbackId) {
        throw new Error(`${annotationLabel} mapping is invalid`)
      }
      ordinals.add(annotation.ordinal)
      feedbackIds.add(annotation.feedbackId)
    }
    if (feedbackIds.size !== expectedFeedback.size
        || [...expectedFeedback].some((id) => !feedbackIds.has(id))
        || artifact.annotations.some((_, annotationIndex) => !ordinals.has(annotationIndex + 1))) {
      throw new Error(`${label} annotation mapping is incomplete`)
    }
  }
  if (seenAssets.size !== expectedByAsset.size
      || [...expectedByAsset.keys()].some((assetId) => !seenAssets.has(assetId))) {
    throw new Error('completed round artifacts do not cover every image annotation')
  }
}

async function validateArtifactFiles(roundDirectory, artifacts) {
  await requireSafeDirectory(roundDirectory, 'completed round bundle')
  const artifactsDirectory = path.join(roundDirectory, 'artifacts')
  await requireSafeDirectory(artifactsDirectory, 'review artifacts directory')

  const bundleEntries = await readSafeDirectoryEntries(roundDirectory, 'completed round bundle', 2)
  const allowedBundleEntries = new Set(['round.json', 'artifacts'])
  if (bundleEntries.length !== allowedBundleEntries.size
      || bundleEntries.some((entry) => !allowedBundleEntries.has(entry.name))) {
    throw new Error('completed round bundle contains an unexpected entry')
  }

  const expectedNames = new Set(
    artifacts.map((artifact) => path.posix.basename(artifact.relativePath)),
  )
  const artifactEntries = await readSafeDirectoryEntries(
    artifactsDirectory,
    'review artifacts directory',
  )
  for (const entry of artifactEntries) {
    if (!entry.isFile() || !expectedNames.has(entry.name)) {
      throw new Error('completed round contains an undeclared review artifact')
    }
  }
  if (artifactEntries.length !== expectedNames.size) {
    throw new Error('completed round is missing a review artifact')
  }

  let totalBytes = 0
  for (const [index, artifact] of artifacts.entries()) {
    const label = `review artifact ${index}`
    const artifactPath = await resolveSafeLocation(roundDirectory, artifact.relativePath, label)
    const bytes = await readBoundedFile(
      artifactPath,
      Math.min(MAX_ARTIFACT_BYTES, MAX_BUNDLE_BYTES - totalBytes),
      label,
    )
    totalBytes += bytes.length
    if (blake3Hex(bytes) !== artifact.blake3) {
      throw new Error(`${label} digest does not match its manifest`)
    }
    const dimensions = pngDimensions(bytes, label)
    if (dimensions.width !== artifact.width || dimensions.height !== artifact.height) {
      throw new Error(`${label} dimensions do not match its manifest`)
    }
  }
}

async function readSafeDirectoryEntries(directory, label, maximumEntries = MAX_ASSETS) {
  let handle
  try {
    handle = await opendir(directory)
  } catch {
    throw new Error(`${label} is unavailable`)
  }
  const entries = []
  for await (const entry of handle) {
    if (entry.isSymbolicLink()) throw new Error(`${label} cannot contain a symbolic link`)
    entries.push(entry)
    if (entries.length > maximumEntries) throw new Error(`${label} exceeds its entry limit`)
  }
  return entries
}

function pngDimensions(bytes, label) {
  if (bytes.length < 24 || !bytes.subarray(0, pngSignature.length).equals(pngSignature)
      || bytes.toString('ascii', 12, 16) !== 'IHDR') {
    throw new Error(`${label} is not a PNG image`)
  }
  const width = bytes.readUInt32BE(16)
  const height = bytes.readUInt32BE(20)
  if (width === 0 || height === 0 || width * height > MAX_ARTIFACT_PIXELS) {
    throw new Error(`${label} dimensions are invalid`)
  }
  return { width, height }
}

// Unkeyed, 32-byte BLAKE3 for the bounded protocol files. The reference algorithm
// and test vectors are published at https://github.com/BLAKE3-team/BLAKE3.
const blake3Iv = Uint32Array.from([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
])
const blake3Permutation = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
const blake3Schedules = [Array.from({ length: 16 }, (_, index) => index)]
for (let round = 1; round < 7; round += 1) {
  const previous = blake3Schedules[round - 1]
  blake3Schedules.push(blake3Permutation.map((index) => previous[index]))
}

export function blake3Hex(bytes) {
  if (!(bytes instanceof Uint8Array)) throw new TypeError('BLAKE3 input must be bytes')
  const chunkCount = Math.max(1, Math.ceil(bytes.length / 1024))
  const stack = []
  for (let chunkIndex = 0; chunkIndex < chunkCount - 1; chunkIndex += 1) {
    let value = blake3ChainingValue(
      blake3ChunkOutput(bytes.subarray(chunkIndex * 1024, (chunkIndex + 1) * 1024), chunkIndex),
    )
    let totalChunks = chunkIndex + 1
    while (totalChunks % 2 === 0) {
      value = blake3ChainingValue(blake3ParentOutput(stack.pop(), value))
      totalChunks /= 2
    }
    stack.push(value)
  }
  let output = blake3ChunkOutput(bytes.subarray((chunkCount - 1) * 1024), chunkCount - 1)
  while (stack.length > 0) {
    output = blake3ParentOutput(stack.pop(), blake3ChainingValue(output))
  }
  const words = blake3Compress({ ...output, counter: 0, flags: output.flags | 8 })
  const digest = Buffer.alloc(32)
  for (let index = 0; index < 8; index += 1) digest.writeUInt32LE(words[index], index * 4)
  return digest.toString('hex')
}

function blake3ChunkOutput(bytes, counter) {
  const blockCount = Math.max(1, Math.ceil(bytes.length / 64))
  let inputCv = blake3Iv
  let output
  for (let index = 0; index < blockCount; index += 1) {
    const block = bytes.subarray(index * 64, (index + 1) * 64)
    const blockWords = new Uint32Array(16)
    for (let offset = 0; offset < block.length; offset += 1) {
      blockWords[offset >>> 2] |= block[offset] << ((offset & 3) * 8)
    }
    output = {
      inputCv,
      blockWords,
      counter,
      blockLength: block.length,
      flags: (index === 0 ? 1 : 0) | (index === blockCount - 1 ? 2 : 0),
    }
    if (index < blockCount - 1) inputCv = blake3ChainingValue(output)
  }
  return output
}

function blake3ParentOutput(left, right) {
  const blockWords = new Uint32Array(16)
  blockWords.set(left)
  blockWords.set(right, 8)
  return { inputCv: blake3Iv, blockWords, counter: 0, blockLength: 64, flags: 4 }
}

function blake3ChainingValue(output) {
  return blake3Compress(output).slice(0, 8)
}

function blake3Compress({ inputCv, blockWords, counter, blockLength, flags }) {
  const state = new Uint32Array(16)
  state.set(inputCv)
  state.set(blake3Iv.subarray(0, 4), 8)
  state[12] = counter >>> 0
  state[13] = Math.floor(counter / 0x1_0000_0000)
  state[14] = blockLength
  state[15] = flags
  for (const schedule of blake3Schedules) {
    const word = (index) => blockWords[schedule[index]]
    blake3Mix(state, 0, 4, 8, 12, word(0), word(1))
    blake3Mix(state, 1, 5, 9, 13, word(2), word(3))
    blake3Mix(state, 2, 6, 10, 14, word(4), word(5))
    blake3Mix(state, 3, 7, 11, 15, word(6), word(7))
    blake3Mix(state, 0, 5, 10, 15, word(8), word(9))
    blake3Mix(state, 1, 6, 11, 12, word(10), word(11))
    blake3Mix(state, 2, 7, 8, 13, word(12), word(13))
    blake3Mix(state, 3, 4, 9, 14, word(14), word(15))
  }
  for (let index = 0; index < 8; index += 1) {
    state[index] ^= state[index + 8]
    state[index + 8] ^= inputCv[index]
  }
  return state
}

function blake3Mix(state, a, b, c, d, x, y) {
  state[a] = state[a] + state[b] + x
  state[d] = rotateRight(state[d] ^ state[a], 16)
  state[c] = state[c] + state[d]
  state[b] = rotateRight(state[b] ^ state[c], 12)
  state[a] = state[a] + state[b] + y
  state[d] = rotateRight(state[d] ^ state[a], 8)
  state[c] = state[c] + state[d]
  state[b] = rotateRight(state[b] ^ state[c], 7)
}

function rotateRight(value, amount) {
  return (value >>> amount) | (value << (32 - amount))
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
    && !value.includes('\0')
    && !/^[A-Za-z]:/.test(value)
    && value.split('/').every((segment) => segment !== '' && segment !== '.' && segment !== '..'
      && segment.toLowerCase() !== '.viewer')
}

function isSafeRepositoryLocation(value) {
  return typeof value === 'string'
    && isSafeRelativePath(value)
    && !value.includes('\0')
    && !/^[A-Za-z]:/.test(value)
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
