import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import path from 'node:path'

import { sha256 } from './source-identity.mjs'
import { collectAppRuntimeIdentity } from './build-attestation.mjs'
import { loadVerifiedNetworkLaunchArtifact } from './network-launch-artifact.mjs'

const [appPath, matrixPath, diagnosticPath, auditPath, launchPath, outputPath] =
  process.argv.slice(2)
if (!appPath || !matrixPath || !diagnosticPath || !auditPath || !launchPath || !outputPath) {
  throw new Error(
    'usage: emit-development-smoke.mjs <Viewer.app> <matrix.json> <diagnostic.json> <bundle-audit.json> <network-launch.json> <output.json>',
  )
}

const matrixBytes = readFileSync(matrixPath)
const diagnosticBytes = readFileSync(diagnosticPath)
const matrix = JSON.parse(matrixBytes)
const diagnostic = JSON.parse(diagnosticBytes)
if (JSON.stringify(matrix.binding) !== JSON.stringify(diagnostic.binding)) {
  throw new Error('smoke inputs do not share one evidence binding')
}
const binding = matrix.binding
if (binding?.schemaVersion !== 1) throw new Error('smoke inputs have no evidence binding')
const auditBytes = readFileSync(auditPath)
const audit = JSON.parse(auditBytes)
const auditSha256 = sha256(auditBytes)
if (
  audit.schemaVersion !== 1 ||
  audit.mode !== 'development' ||
  auditSha256 !== binding.bundleAuditSha256 ||
  audit.attestationSha256 !== binding.buildAttestationSha256 ||
  JSON.stringify(audit.appRuntime) !== JSON.stringify(binding.appRuntime) ||
  JSON.stringify(audit.bundleIdentity) !== JSON.stringify(binding.bundleIdentity) ||
  JSON.stringify(collectAppRuntimeIdentity(appPath)) !== JSON.stringify(binding.appRuntime) ||
  Object.values(audit.checks ?? {}).length !== 5 ||
  !Object.values(audit.checks).every((value) => value === true)
) {
  throw new Error('smoke verifier audit artifact does not match the native evidence binding')
}
const launch = loadVerifiedNetworkLaunchArtifact({
  artifactPath: launchPath,
  appPath,
  binding,
})
if (launch.sha256 !== binding.networkLaunchSha256) {
  throw new Error('smoke network launch artifact digest does not match the native evidence binding')
}
const evidence = {
  schemaVersion: 1,
  status: 'passed',
  binding,
  matrixSha256: sha256(matrixBytes),
  diagnosticSha256: sha256(diagnosticBytes),
  audit: {
    mode: audit.mode,
    artifactSha256: auditSha256,
    attestationSha256: audit.attestationSha256,
    appRuntime: audit.appRuntime,
    bundleIdentity: audit.bundleIdentity,
    checks: audit.checks,
  },
  launch: {
    artifactSha256: launch.sha256,
    networkPolicy: launch.artifact.networkPolicy,
    executable: launch.artifact.launch.executable,
    arguments: launch.artifact.launch.arguments,
    sandboxProfileSha256: launch.artifact.launch.sandboxProfileSha256,
    launcherPid: launch.artifact.launcherPid,
    targetPid: launch.artifact.targetPid,
    windowIdentity: launch.artifact.windowIdentity,
  },
  networkDisabled: launch.artifact.networkPolicy === 'deny-all',
  bundledRuntimeOnly: audit.checks.inventoryAndHashes && audit.checks.bundledRuntimeOfflineProbe,
  hostDependenciesAbsent:
    audit.checks.loaderContainment && audit.checks.appleAbsoluteDependenciesOnly,
}
mkdirSync(path.dirname(outputPath), { recursive: true })
const temporary = `${outputPath}.tmp-${process.pid}`
writeFileSync(temporary, `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 })
renameSync(temporary, outputPath)
console.log(`PASS development smoke evidence ${outputPath}`)
