#!/usr/bin/env node

import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { blake3Hex } from './blake3.mjs'
export { blake3Hex } from './blake3.mjs'
import { openSafeProject, isSafeRepositoryLocation } from './safe-read.mjs'

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
  const { reader, catalog } = await readCatalog(projectRoot)
  const selected = selectStream(catalog.streams, { reviewStreamId, taskId, batchId })
  if (selected.latestCompletedRoundId === null) {
    throw new Error('selected review stream has no completed review head')
  }

  const roundId = selected.latestCompletedRoundId
  const record = completedRecord(selected, roundId, catalog.protocolVersion)
  const roundBytes = await reader.readRepositoryBytes(record.location, MAX_ROUND_BYTES, 'completed round')
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
    await validateArtifactFiles(reader, path.posix.dirname(record.location), round.artifacts)
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
  const reader = await openSafeProject(projectRoot)
  const { data: catalog } = await reader.readRepositoryJson('index.json', MAX_INDEX_BYTES, 'review index')
  validateCatalog(catalog)
  return { reader, catalog }
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

async function validateArtifactFiles(reader, roundDirectory, artifacts) {
  const artifactsDirectory = path.posix.join(roundDirectory, 'artifacts')
  const bundleEntries = await reader.readRepositoryDirectory(roundDirectory, 2, 'completed round bundle')
  const allowedBundleEntries = new Set(['round.json', 'artifacts'])
  if (bundleEntries.length !== allowedBundleEntries.size
      || bundleEntries.some((entry) => !allowedBundleEntries.has(entry.name))) {
    throw new Error('completed round bundle contains an unexpected entry')
  }

  const expectedNames = new Set(
    artifacts.map((artifact) => path.posix.basename(artifact.relativePath)),
  )
  const artifactEntries = await reader.readRepositoryDirectory(
    artifactsDirectory, MAX_ASSETS, 'review artifacts directory',
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
    const { bytes } = await reader.readRepositoryPng(
      path.posix.join(roundDirectory, artifact.relativePath),
      artifact,
      label,
      Math.min(MAX_ARTIFACT_BYTES, MAX_BUNDLE_BYTES - totalBytes),
    )
    totalBytes += bytes.length
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
    && !value.includes('\0')
    && !/^[A-Za-z]:/.test(value)
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
