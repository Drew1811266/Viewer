import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { ACCEPTANCE_STATE_DEFINITIONS } from './acceptance/acceptanceStateCatalog'

const stateId =
  '(?:(?:LAU|SID|STR|THU|OTH|SEA|FIL|MEN|RAD|PRE|COM|DOC|INF|DIA|TAS|RES|A11Y)-\\d+|workspace-video-(?:expanded|unavailable)|video-(?:preparing|playing-controls|paused-controls|timeline-pending|timeline-ready|ended|failed-retry|fullscreen-controls|reduced-motion)|settings-video-cache)'
const statePattern = new RegExp(`^\\| (${stateId}) \\|`, 'gm')
const rowPattern = new RegExp(`^\\| ${stateId} \\|`)

function ids(path: string): string[] {
  const source = readFileSync(resolve(import.meta.dirname, path), 'utf8')
  return [...source.matchAll(statePattern)].map((match) => match[1] as string)
}

function rows(path: string): string[][] {
  return readFileSync(resolve(import.meta.dirname, path), 'utf8')
    .split('\n')
    .filter((line) => rowPattern.test(line))
    .map((line) => line.split('|').map((cell) => cell.trim()))
}

describe('atlas-to-product migration coverage', () => {
  it('tracks every audited state exactly once', () => {
    const audit = ids('../../docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md')
    const ledger = ids('../../docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md')
    expect(audit).toHaveLength(104)
    expect(new Set(audit).size).toBe(104)
    expect(ledger).toHaveLength(104)
    expect(new Set(ledger).size).toBe(104)
    expect([...ledger].sort()).toEqual([...audit].sort())
    expect(
      ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup !== 'review').map(
        ({ id }) => id,
      ),
    ).toEqual(ledger)
  })

  it('records automated evidence for every state before native acceptance', () => {
    const ledgerRows = rows(
      '../../docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md',
    )
    expect(ledgerRows).toHaveLength(104)
    for (const row of ledgerRows) expect(row[6]).not.toBe('not-recorded')
  })
})
