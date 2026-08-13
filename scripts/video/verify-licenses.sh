#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "$0")" && pwd -P)
repo_root=$(cd "$script_dir/../.." && pwd -P)
notices=${VIEWER_VIDEO_NOTICES_FILE:-$repo_root/THIRD_PARTY_NOTICES.md}
health=${VIEWER_VIDEO_HEALTH_FILE:-$repo_root/docs/quality/DEPENDENCY_HEALTH.md}
acknowledgements=${VIEWER_VIDEO_ACKNOWLEDGEMENTS_FILE:-$repo_root/ACKNOWLEDGEMENTS.md}
source_offer=${VIEWER_VIDEO_SOURCE_OFFER_FILE:-$script_dir/source-offer.txt}
lgpl_text=${VIEWER_VIDEO_LGPL_TEXT_FILE:-$script_dir/LGPL-2.1-or-later.txt}

node --input-type=module - "$script_dir/runtime.lock.json" "$notices" "$health" "$acknowledgements" "$source_offer" "$lgpl_text" <<'NODE'
import { readFile } from 'node:fs/promises'

const [lockPath, noticesPath, healthPath, acknowledgementsPath, offerPath, lgplPath] = process.argv.slice(2)
const [lockText, notices, health, acknowledgements, offer, lgpl] = await Promise.all(
  [lockPath, noticesPath, healthPath, acknowledgementsPath, offerPath, lgplPath].map((path) => readFile(path, 'utf8')),
)
const lock = JSON.parse(lockText)
const requiredMetadata = ['Security owner', 'Review date', 'Upgrade procedure', 'Enabled build options']

for (const component of lock.components) {
  for (const [documentName, document] of [['notices', notices], ['dependency health', health]]) {
    const values = [component.name, component.version, component.license, component.sourceUrl, component.sha256]
    const completeRow = document.split(/\r?\n/).some((line) => values.every((value) => line.includes(value)))
    if (!completeRow) throw new Error(`${component.name} is incomplete in ${documentName}`)
  }
}
for (const label of requiredMetadata) {
  if (!health.includes(label)) throw new Error(`dependency health is missing ${label}`)
}
for (const option of [...Object.entries(lock.mpv.mesonOptions).map(([key, value]) => `-D${key}=${value}`), ...lock.ffmpeg.configureOptions]) {
  if (!health.includes(option)) throw new Error(`dependency health is missing build option ${option}`)
}
if (!acknowledgements.includes('ViewerVideoRuntime')) throw new Error('acknowledgements omit ViewerVideoRuntime')
if (!offer.includes('runtime-lock.mjs list') || !offer.includes('build-macos-runtime.sh')) throw new Error('source offer is not reproducible')
if (!lgpl.includes('GNU LESSER GENERAL PUBLIC LICENSE') || !lgpl.includes('Version 2.1, February 1999') || !lgpl.includes('END OF TERMS AND CONDITIONS')) {
  throw new Error('LGPL-2.1-or-later text is incomplete')
}
NODE

printf 'Viewer video runtime licenses verified\n'
