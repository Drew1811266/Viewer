import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { ACCEPTANCE_STATE_DEFINITIONS, acceptanceDefinition } from './acceptanceStateCatalog'

interface LedgerState {
  id: string
  wave: number
  referenceState: string
}

function ledgerStates(): LedgerState[] {
  const source = readFileSync(
    resolve(
      import.meta.dirname,
      '../../../docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md',
    ),
    'utf8',
  )
  return source
    .split('\n')
    .filter((line) => /^\| [A-Z0-9]+-\d{2} \|/.test(line))
    .map((line) => {
      const cells = line
        .slice(1, -1)
        .split(' | ')
        .map((cell) => cell.trim())
      const id = cells[0]
      const waveMatch = cells[1]?.match(/^Wave ([1-4])$/)
      const referenceMatch = cells[2]?.match(/^`([^`]+)`$/)
      if (
        id === undefined ||
        waveMatch === undefined ||
        waveMatch === null ||
        referenceMatch === undefined ||
        referenceMatch === null
      ) {
        throw new Error(`Malformed Viewer acceptance ledger row: ${line}`)
      }
      return {
        id,
        wave: Number(waveMatch[1]),
        referenceState: referenceMatch[1] as string,
      }
    })
}

describe('Viewer visual acceptance state catalog', () => {
  it('matches every ledger ID, wave and atlas reference state exactly once', () => {
    const expected = ledgerStates()
    const actual = ACCEPTANCE_STATE_DEFINITIONS.map(({ id, wave, referenceState }) => ({
      id,
      wave,
      referenceState,
    }))

    expect(actual).toHaveLength(92)
    expect(new Set(actual.map(({ id }) => id)).size).toBe(92)
    expect(actual).toEqual(expected)
  })

  it('fails closed for an unknown visual acceptance state', () => {
    expect(() => acceptanceDefinition('PRE-99')).toThrow('Unknown Viewer acceptance state: PRE-99')
  })
})
