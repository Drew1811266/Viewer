import { mkdirSync, readFileSync, readdirSync, renameSync, statSync, writeFileSync } from 'node:fs'
import path from 'node:path'

import { collectSourceIdentity, sha256 } from './source-identity.mjs'

const executableRelativePath = 'Contents/MacOS/viewer-desktop'
const runtimeRelativePath = 'Contents/Resources/ViewerVideoRuntime'

export function collectAppRuntimeIdentity(appPath) {
  const runtime = path.join(appPath, runtimeRelativePath)
  const inventoryPath = path.join(runtime, 'runtime.inventory.sha256')
  const inventoryBytes = readFileSync(inventoryPath)
  const files = inventoryBytes
    .toString()
    .trim()
    .split('\n')
    .filter(Boolean)
    .map((line) => {
      const match = /^([a-f0-9]{64})  ([^/].*)$/.exec(line)
      if (!match) throw new Error('runtime inventory contains an invalid entry')
      const actualSha256 = sha256(readFileSync(path.join(runtime, match[2])))
      if (actualSha256 !== match[1]) {
        throw new Error(`app/runtime identity differs from inventory: ${match[2]}`)
      }
      return { path: match[2], sha256: actualSha256 }
    })
  return {
    appInfoPlistSha256: sha256(readFileSync(path.join(appPath, 'Contents/Info.plist'))),
    appExecutableSha256: sha256(readFileSync(path.join(appPath, executableRelativePath))),
    runtimeInventorySha256: sha256(inventoryBytes),
    runtimeLockSha256: sha256(readFileSync(path.join(runtime, 'runtime.lock.json'))),
    runtimeFiles: files,
    runtimeTree: collectFileTree(runtime),
  }
}

function collectFileTree(root, relativeRoot = '') {
  return readdirSync(path.join(root, relativeRoot), { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))
    .flatMap((entry) => {
      const relative = path.join(relativeRoot, entry.name)
      if (entry.isDirectory()) return collectFileTree(root, relative)
      const filePath = path.join(root, relative)
      const metadata = statSync(filePath)
      if (!entry.isFile()) throw new Error(`app/runtime identity contains a non-file: ${relative}`)
      return [{ path: relative, mode: metadata.mode & 0o777, sha256: sha256(readFileSync(filePath)) }]
    })
}

export function createBuildAttestation({
  sourceRoot,
  appPath,
  bundleIdentity,
  buildConfig,
}) {
  const sourceIdentity = collectSourceIdentity(sourceRoot)
  return {
    schemaVersion: 1,
    sourceIdentity,
    sourceIdentitySha256: sha256(JSON.stringify(sourceIdentity)),
    appRuntime: collectAppRuntimeIdentity(appPath),
    bundleIdentity: structuredClone(bundleIdentity),
    buildConfig: structuredClone(buildConfig),
  }
}

export function writeBuildAttestation(outputPath, attestation) {
  return writeAtomicJson(outputPath, attestation)
}

export function loadVerifiedBuildAttestation({
  sourceRoot,
  appPath,
  attestationPath,
  bundleIdentity,
  buildConfig,
}) {
  const bytes = readFileSync(attestationPath)
  const attestation = JSON.parse(bytes)
  if (attestation.schemaVersion !== 1) throw new Error('build attestation is missing or unsupported')
  const currentSource = collectSourceIdentity(sourceRoot)
  if (!same(attestation.sourceIdentity, currentSource)) {
    throw new Error('build attestation source identity does not match current source identity')
  }
  if (attestation.sourceIdentitySha256 !== sha256(JSON.stringify(currentSource))) {
    throw new Error('build attestation source identity digest is invalid')
  }
  if (!same(attestation.appRuntime, collectAppRuntimeIdentity(appPath))) {
    throw new Error('build attestation app/runtime identity does not match the final bundle')
  }
  if (!same(attestation.bundleIdentity, bundleIdentity) || !same(attestation.buildConfig, buildConfig)) {
    throw new Error('build attestation bundle identity or build config does not match')
  }
  return { attestation, sha256: sha256(bytes) }
}

export function writeAuditArtifact({ appPath, attestationPath, outputPath, mode }) {
  if (!['development', 'signed'].includes(mode)) throw new Error('unsupported bundle audit mode')
  const attestationBytes = readFileSync(attestationPath)
  const attestation = JSON.parse(attestationBytes)
  const appRuntime = collectAppRuntimeIdentity(appPath)
  if (!same(appRuntime, attestation.appRuntime)) {
    throw new Error('bundle audit app/runtime identity differs from build attestation')
  }
  return writeAtomicJson(outputPath, {
    schemaVersion: 1,
    mode,
    attestationSha256: sha256(attestationBytes),
    appRuntime,
    bundleIdentity: attestation.bundleIdentity,
    checks: {
      inventoryAndHashes: true,
      architectureMatch: true,
      loaderContainment: true,
      appleAbsoluteDependenciesOnly: true,
      bundledRuntimeOfflineProbe: true,
    },
  })
}

export function loadVerifiedAuditArtifact(options) {
  const attested = loadVerifiedBuildAttestation(options)
  const bytes = readFileSync(options.auditPath)
  const artifact = JSON.parse(bytes)
  if (
    artifact.schemaVersion !== 1 ||
    !['development', 'signed'].includes(artifact.mode) ||
    artifact.attestationSha256 !== attested.sha256 ||
    !same(artifact.appRuntime, attested.attestation.appRuntime) ||
    !same(artifact.bundleIdentity, attested.attestation.bundleIdentity) ||
    Object.values(artifact.checks ?? {}).length !== 5 ||
    !Object.values(artifact.checks).every((value) => value === true)
  ) {
    throw new Error('bundle audit artifact does not match the attested app/runtime identity')
  }
  return { artifact, sha256: sha256(bytes), attestation: attested }
}

export async function prepareAcceptanceBundle({
  skipBuild,
  build,
  signRuntime,
  refreshInventory,
  signApp,
  createOrVerifyAttestation,
  audit,
  verifyAudit,
}) {
  if (!skipBuild) {
    await build()
    await signRuntime()
    await refreshInventory()
    await signApp()
  }
  await createOrVerifyAttestation(skipBuild)
  await audit()
  return verifyAudit()
}

export function performanceResetPlan(fixtureId) {
  return { fixtureId, action: '重新挂载当前样本' }
}

function writeAtomicJson(outputPath, value) {
  mkdirSync(path.dirname(outputPath), { recursive: true })
  const temporary = `${outputPath}.tmp-${process.pid}`
  writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 })
  renameSync(temporary, outputPath)
  return value
}

function same(left, right) {
  return JSON.stringify(left) === JSON.stringify(right)
}
