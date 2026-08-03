import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const statePattern =
  /^\| ((?:LAU|SID|STR|THU|OTH|SEA|FIL|MEN|RAD|PRE|COM|DOC|INF|DIA|TAS|RES|A11Y)-\d+) \|/gm

function ids(path: string): string[] {
  const source = readFileSync(resolve(import.meta.dirname, path), 'utf8')
  return [...source.matchAll(statePattern)].map((match) => match[1] as string)
}

describe('atlas-to-product migration coverage', () => {
  it('tracks every audited state exactly once', () => {
    const audit = ids('../../docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md')
    const ledger = ids('../../docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md')
    expect(audit).toHaveLength(89)
    expect(new Set(audit).size).toBe(89)
    expect(ledger).toHaveLength(89)
    expect(new Set(ledger).size).toBe(89)
    expect([...ledger].sort()).toEqual([...audit].sort())
  })
})
