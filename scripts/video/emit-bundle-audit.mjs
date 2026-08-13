import { writeAuditArtifact } from './build-attestation.mjs'

const [mode, appPath, attestationPath, outputPath] = process.argv.slice(2)
if (!mode || !appPath || !attestationPath || !outputPath) {
  throw new Error(
    'usage: emit-bundle-audit.mjs <development|signed> <Viewer.app> <build-attestation.json> <output.json>',
  )
}
writeAuditArtifact({ appPath, attestationPath, outputPath, mode })
console.log(`PASS bundle audit artifact ${outputPath}`)
