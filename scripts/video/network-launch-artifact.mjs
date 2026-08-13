import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import path from 'node:path'

import { collectAppRuntimeIdentity } from './build-attestation.mjs'
import { sha256 } from './source-identity.mjs'

export const DENY_NETWORK_SANDBOX_PROFILE = '(version 1)(allow default)(deny network*)'

export function writeNetworkLaunchArtifact({
  outputPath,
  appPath,
  binding,
  launch,
  launcherPid,
  targetPid,
  windowIdentity,
}) {
  const appExecutable = path.join(appPath, binding.bundleIdentity.executableRelativePath)
  if (
    launch.networkPolicy !== 'deny-all' ||
    launch.sandboxProfile !== DENY_NETWORK_SANDBOX_PROFILE ||
    !same(launch.argumentsList, ['-p', DENY_NETWORK_SANDBOX_PROFILE, appExecutable])
  ) {
    throw new Error('offline launch evidence requires the canonical deny-all sandbox launch')
  }
  if (!same(collectAppRuntimeIdentity(appPath), binding.appRuntime)) {
    throw new Error('network launch app/runtime identity does not match the evidence binding')
  }
  if (!validPid(launcherPid) || !validPid(targetPid) || !validWindow(windowIdentity)) {
    throw new Error('network launch process or window identity is incomplete')
  }
  const artifact = {
    schemaVersion: 1,
    runId: binding.runId,
    sourceIdentitySha256: sha256(JSON.stringify(binding.sourceIdentity)),
    buildAttestationSha256: binding.buildAttestationSha256,
    bundleAuditSha256: binding.bundleAuditSha256,
    appRuntime: binding.appRuntime,
    bundleIdentity: binding.bundleIdentity,
    machine: binding.machine,
    networkPolicy: 'deny-all',
    launch: {
      executable: launch.command,
      arguments: launch.argumentsList,
      targetExecutable: appExecutable,
      sandboxProfile: DENY_NETWORK_SANDBOX_PROFILE,
      sandboxProfileSha256: sha256(DENY_NETWORK_SANDBOX_PROFILE),
    },
    launcherPid,
    targetPid,
    windowIdentity: structuredClone(windowIdentity),
  }
  mkdirSync(path.dirname(outputPath), { recursive: true })
  const temporary = `${outputPath}.tmp-${process.pid}`
  writeFileSync(temporary, `${JSON.stringify(artifact, null, 2)}\n`, { mode: 0o600 })
  renameSync(temporary, outputPath)
  return artifact
}

export function loadVerifiedNetworkLaunchArtifact({
  artifactPath,
  appPath,
  binding,
  expectedSandboxExecutable = '/usr/bin/sandbox-exec',
}) {
  const bytes = readFileSync(artifactPath)
  const artifact = JSON.parse(bytes)
  const appExecutable = path.join(appPath, binding.bundleIdentity.executableRelativePath)
  if (artifact.schemaVersion !== 1 || artifact.runId !== binding.runId) {
    throw new Error('network launch artifact run binding differs')
  }
  if (artifact.sourceIdentitySha256 !== sha256(JSON.stringify(binding.sourceIdentity))) {
    throw new Error('network launch artifact source identity differs')
  }
  if (artifact.buildAttestationSha256 !== binding.buildAttestationSha256) {
    throw new Error('network launch artifact build attestation differs')
  }
  if (artifact.bundleAuditSha256 !== binding.bundleAuditSha256) {
    throw new Error('network launch artifact audit digest differs')
  }
  if (
    !same(artifact.appRuntime, binding.appRuntime) ||
    !same(artifact.appRuntime, collectAppRuntimeIdentity(appPath)) ||
    !same(artifact.bundleIdentity, binding.bundleIdentity)
  ) {
    throw new Error('network launch artifact app/runtime identity differs')
  }
  if (!same(artifact.machine, binding.machine)) {
    throw new Error('network launch artifact machine identity differs')
  }
  if (
    artifact.networkPolicy !== 'deny-all' ||
    artifact.launch?.executable !== expectedSandboxExecutable ||
    artifact.launch?.targetExecutable !== appExecutable ||
    artifact.launch?.sandboxProfile !== DENY_NETWORK_SANDBOX_PROFILE ||
    artifact.launch?.sandboxProfileSha256 !== sha256(DENY_NETWORK_SANDBOX_PROFILE) ||
    !same(artifact.launch?.arguments, ['-p', DENY_NETWORK_SANDBOX_PROFILE, appExecutable])
  ) {
    throw new Error('network launch artifact sandbox profile or deny-all policy differs')
  }
  if (!validPid(artifact.launcherPid) || !validPid(artifact.targetPid) || !validWindow(artifact.windowIdentity)) {
    throw new Error('network launch artifact process or window identity is incomplete')
  }
  return { artifact, sha256: sha256(bytes) }
}

function validPid(value) {
  return Number.isInteger(value) && value > 0
}

function validWindow(value) {
  return (
    Number.isInteger(value?.windowId) &&
    value.windowId > 0 &&
    typeof value.title === 'string' &&
    value.title.length > 0 &&
    Number.isFinite(value.width) &&
    value.width > 0 &&
    Number.isFinite(value.height) &&
    value.height > 0
  )
}

function same(left, right) {
  return JSON.stringify(left) === JSON.stringify(right)
}
