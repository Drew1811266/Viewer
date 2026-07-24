import { describe, expect, it } from 'vitest'
import { rangeSelection, toggleSelection } from './contentSelection'

describe('content selection', () => {
  const order = ['a', 'b', 'c', 'd']

  it('selects a forward range', () => {
    expect(rangeSelection(order, 'b', 'd')).toEqual(['b', 'c', 'd'])
  })

  it('normalizes a reverse range to display order', () => {
    expect(rangeSelection(order, 'd', 'b')).toEqual(['b', 'c', 'd'])
  })

  it('falls back to the target when the anchor is absent', () => {
    expect(rangeSelection(order, 'missing', 'c')).toEqual(['c'])
  })

  it('adds an unselected target', () => {
    expect(toggleSelection(['a'], 'c')).toEqual(['a', 'c'])
  })

  it('removes an already selected target', () => {
    expect(toggleSelection(['a', 'c'], 'c')).toEqual(['a'])
  })
})
