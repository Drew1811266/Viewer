// Owned temporary projects only. Static media and legacy bytes are never opened for writing.
import { copyFile, cp, mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { blake3Hex } from './read-latest.mjs'

const fixtureRoot = fileURLToPath(new URL('../../tests/fixtures/review-protocol/', import.meta.url))
const imagePath = fileURLToPath(new URL('../../tests/fixtures/images/alpha.png', import.meta.url))
const readGolden = async name => JSON.parse(await readFile(join(fixtureRoot, `${name}.json`), 'utf8'))
const id = value => `00000000-0000-4000-8000-${String(value).padStart(12, '0')}`
const bytesOf = value => Buffer.from(`${JSON.stringify(value, null, 2)}\n`)
const keysOf = state => state.feedback.flatMap(feedback => feedback.targets.map(target => ({
  feedbackId: feedback.feedbackId, textRevisionId: feedback.textRevisionId,
  targetId: target.targetId, targetRevisionId: target.targetRevisionId,
})))

export async function createProjectV3Case(name) {
  const cases = await readGolden('continuous-review-cases')
  const scenario = cases.find(item => item.name === name)
  if (!scenario) throw new Error('unknown continuous review fixture')
  const projectRoot = await mkdtemp(join(tmpdir(), 'viewer-review-v3-'))
  const cleanup = () => rm(projectRoot, { recursive: true, force: true })
  try {
    const repository = join(projectRoot, '.viewer/reviews')
    for (const directory of ['states', 'archives', 'evidence']) await mkdir(join(repository, directory), { recursive: true })
    const state = await readGolden('review-state-v3.valid')
    const index = await readGolden('review-index-v3.valid')
    const archive = await readGolden('review-archive-v3.valid')
    const png = await readFile(imagePath)
    const pngDigest = blake3Hex(png)
    await writeFile(join(repository, 'evidence', `${pngDigest}.png`), png, { flag: 'wx' })
    for (const asset of state.assets) {
      const destination = join(projectRoot, asset.relativePath)
      await copyFile(imagePath, destination)
      const metadata = await stat(destination, { bigint: true })
      asset.evidence = { sizeBytes: Number(metadata.size), modifiedNs: String(metadata.mtimeNs), blake3: pngDigest }
      const binding = state.evidence.find(item => item.assetVersionId === asset.assetVersionId)
      binding.capability.base = { blake3: pngDigest, sizeBytes: png.length, width: png.readUInt32BE(16), height: png.readUInt32BE(20) }
    }
    const snapshotIds = []
    const saveState = async value => {
      const bytes = bytesOf(value)
      await writeFile(join(repository, 'states', `${value.snapshotId}.json`), bytes, { flag: 'wx' })
      snapshotIds.push(value.snapshotId)
      return { snapshotId: value.snapshotId, blake3: blake3Hex(bytes) }
    }
    const basis = await saveState(state)
    const basisKeys = keysOf(state)
    let current = structuredClone(state)
    let currentRef = basis
    const nextIdentity = () => {
      current.parent = currentRef
      current.snapshotId = id(501 + snapshotIds.length)
      current.commandId = id(701 + snapshotIds.length)
      current.payloadDigest = blake3Hex(bytesOf({ name, snapshot: current.snapshotId }))
    }
    if (scenario.laterText || scenario.pending) {
      nextIdentity()
      if (scenario.laterText) {
        current.feedback[0].text = scenario.laterText
        current.feedback[0].textRevisionId = id(403)
      }
      if (scenario.pending) {
        current.feedback[0].targets.forEach((target, ordinal) => {
          target.targetRevisionId = id(431 + ordinal)
          target.availability = { kind: 'needsConfirmation', reasons: ['sourceChanged'] }
        })
      }
      current.changes = keysOf(current).map((key, ordinal) => ({ targetId: key.targetId, before: basisKeys[ordinal], after: key,
        kind: scenario.pending ? 'availabilityChanged' : 'edited', archiveId: null, historicalKey: null }))
      currentRef = await saveState(current)
    }
    let archiveId = null
    if (scenario.archiveTargets.length) {
      const selected = basisKeys.filter(key => scenario.archiveTargets.includes(key.targetId))
      const currentKeys = keysOf(current)
      archive.beforeRef = currentRef
      archive.groups = [{ usageBasis: scenario.laterText ? { snapshot: basis, source: { kind: 'userSelected' } } : null, targets: selected }]
      archive.removed = scenario.laterText ? [] : selected
      archive.retained = scenario.laterText ? selected.map(key => ({ basis: key, current: currentKeys.find(item => item.targetId === key.targetId), disposition: 'retainLaterEdit' })) : []
      nextIdentity()
      archive.resultSnapshotId = current.snapshotId
      current.feedback.forEach(feedback => { feedback.targets = feedback.targets.filter(target => scenario.currentTargets.includes(target.targetId)) })
      current.feedback = current.feedback.filter(item => item.targets.length)
      current.changes = selected.map(key => ({ targetId: key.targetId, before: currentKeys.find(item => item.targetId === key.targetId),
        after: keysOf(current).find(item => item.targetId === key.targetId) ?? null, kind: 'archived', archiveId: archive.archiveId, historicalKey: key }))
      currentRef = await saveState(current)
      const bytes = bytesOf(archive)
      await writeFile(join(repository, 'archives', `${archive.archiveId}.json`), bytes, { flag: 'wx' })
      index.streams[0].archiveRefs = [{ archiveId: archive.archiveId, location: `archives/${archive.archiveId}.json`, blake3: blake3Hex(bytes) }]
      archiveId = archive.archiveId
    }
    index.streams[0].currentRef = currentRef
    if (name === 'legacy_mixed') {
      const legacyRoot = join(fixtureRoot, 'project-v2-mixed/.viewer/reviews')
      const legacy = JSON.parse(await readFile(join(legacyRoot, 'index.json'), 'utf8'))
      await cp(join(legacyRoot, 'rounds'), join(repository, 'rounds'), { recursive: true, errorOnExist: true, force: false })
      for (const stream of legacy.streams) index.streams.push({
        reviewStreamId: stream.reviewStreamId, taskId: stream.taskId, batchId: stream.batchId,
        currentRef: null, archiveRefs: [], usageRefs: [],
        legacyRefs: stream.completedRounds.map(record => ({ roundId: record.reviewRoundId, protocolVersion: record.protocolVersion, location: record.location, blake3: record.blake3 })),
      })
    }
    await writeFile(join(repository, 'index.json'), bytesOf(index), { flag: 'wx' })
    return { projectRoot, streamId: current.reviewStreamId, snapshotIds, archiveId, cleanup }
  } catch (error) {
    await cleanup()
    throw error
  }
}
