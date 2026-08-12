import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

const version = '1.27.0'
const sources = [
  ['alert-triangle', 'triangle-alert'],
  'arrow-down',
  'arrow-up',
  'check',
  'chevron-down',
  'chevron-left',
  'chevron-right',
  'chevron-up',
  'circle',
  'circle-dot',
  'columns-2',
  'copy',
  'ellipsis',
  'eye',
  'folder-input',
  'folder-output',
  'grip-vertical',
  'info',
  'layout-grid',
  'lock',
  'maximize',
  'minus',
  'minimize',
  'move',
  'panel-right',
  'pause',
  'pencil',
  'play',
  'plus',
  'refresh-cw',
  'rotate-cw',
  'search',
  'settings',
  'sliders-horizontal',
  'skip-back',
  'skip-forward',
  'star',
  'trash-2',
  'volume-2',
  'volume-x',
  'x',
  'zoom-in',
]
const target = path.resolve(process.cwd(), 'ui/src/assets/icons/lucide')
const repository = 'https://raw.githubusercontent.com/lucide-icons/lucide'
const base = `${repository}/refs/tags/${version}`

await mkdir(target, { recursive: true })
for (const entry of sources) {
  const [name, sourceName] = Array.isArray(entry) ? entry : [entry, entry]
  const response = await fetch(`${base}/icons/${sourceName}.svg`)
  if (!response.ok) throw new Error(`Lucide ${name}: HTTP ${response.status}`)
  const source = await response.text()
  if (!source.startsWith('<svg') || !source.includes('viewBox="0 0 24 24"')) {
    throw new Error(`Lucide ${name}: invalid SVG source`)
  }
  await writeFile(path.join(target, `${name}.svg`), source)
}

const licenseResponse = await fetch(`${base}/LICENSE`)
if (!licenseResponse.ok) throw new Error(`Lucide license: HTTP ${licenseResponse.status}`)
const license = await licenseResponse.text()
if (!license.includes('ISC License') || !license.includes('The MIT License')) {
  throw new Error('Lucide license: expected ISC and Feather MIT notices')
}
await writeFile(path.join(target, 'LICENSE.txt'), license)

console.log(`Vendored ${sources.length} Lucide ${version} icons into ${target}`)
