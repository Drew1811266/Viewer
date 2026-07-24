import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { SearchPage, SearchQueryModel } from '../api/types'
import { initialSearchQuery } from '../state/viewerReducer'
import SearchResults from './SearchResults'

const query: SearchQueryModel = {
  ...initialSearchQuery,
  text: 'shoe',
  scopeFolderId: 'folder-1',
  filters: { ...initialSearchQuery.filters, favoriteOnly: true },
}

function page(): SearchPage {
  return {
    revision: 1,
    total: 240,
    progress: {
      imagesTotal: 10,
      imagesReady: 8,
      imagesFailed: 0,
      textTotal: 2,
      textReady: 1,
      textSkipped: 0,
      textFailed: 0,
      complete: false,
    },
    hits: [
      {
        entityId: '1',
        relativePath: 'catalog/id-1/shoe.jpg',
        name: 'shoe.jpg',
        kind: 'jpeg',
        size: 10,
        modifiedNs: '1',
        marker: { reviewState: null, favorite: true },
        imageMetadata: { width: 1200, height: 800 },
        matchedField: 'filename',
        score: 10,
        groupRelativePath: 'catalog/id-1',
        matchRanges: [{ start: 0, end: 4 }],
      },
      {
        entityId: '2',
        relativePath: 'catalog/id-1/prompt.txt',
        name: 'prompt.txt',
        kind: 'text',
        size: 20,
        modifiedNs: '2',
        marker: { reviewState: 'keep', favorite: false },
        imageMetadata: null,
        matchedField: 'body',
        score: 5,
        groupRelativePath: 'catalog/id-1',
        matchRanges: [],
      },
    ],
  }
}

describe('SearchResults', () => {
  it('renders partial grouped results, highlights matches, and bounds plain snippets', () => {
    const visible = vi.fn()
    render(
      <SearchResults
        page={page()}
        query={query}
        snippets={{ '2': '片'.repeat(200) }}
        offset={0}
        limit={200}
        onPageChange={vi.fn()}
        onVisibleHits={visible}
        onClearFilters={vi.fn()}
        onSearchProject={vi.fn()}
        onReturnToFolder={vi.fn()}
      />,
    )
    expect(screen.getByText('结果仍在更新')).toBeVisible()
    expect(screen.getByRole('heading', { name: 'catalog/id-1' })).toBeVisible()
    expect(screen.getByText('shoe', { selector: 'mark' })).toBeVisible()
    expect(
      screen.getByRole('option', { name: 'shoe.jpg catalog/id-1/shoe.jpg' }),
    ).not.toHaveAttribute('tabindex')
    expect(screen.getByTestId('search-snippet-2')).toHaveTextContent('片'.repeat(160))
    expect(screen.getByTestId('search-snippet-2')).not.toHaveTextContent('片'.repeat(161))
    expect(visible).toHaveBeenCalledWith(expect.arrayContaining(['1', '2']))
  })

  it('paginates and offers a return to the selected folder context', () => {
    const pageChange = vi.fn()
    const returnToFolder = vi.fn()
    render(
      <SearchResults
        page={page()}
        query={query}
        snippets={{}}
        offset={0}
        limit={200}
        onPageChange={pageChange}
        onVisibleHits={vi.fn()}
        onClearFilters={vi.fn()}
        onSearchProject={vi.fn()}
        onReturnToFolder={returnToFolder}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '下一页' }))
    expect(pageChange).toHaveBeenCalledWith(200)
    fireEvent.click(screen.getByRole('button', { name: '返回文件夹内容' }))
    expect(returnToFolder).toHaveBeenCalledOnce()
  })

  it('explains a zero state and offers clear plus project-wide expansion actions', () => {
    const clear = vi.fn()
    const expand = vi.fn()
    render(
      <SearchResults
        page={{ ...page(), total: 0, hits: [] }}
        query={query}
        snippets={{}}
        offset={0}
        limit={200}
        onPageChange={vi.fn()}
        onVisibleHits={vi.fn()}
        onClearFilters={clear}
        onSearchProject={expand}
        onReturnToFolder={vi.fn()}
      />,
    )
    expect(screen.getByText(/shoe/)).toBeVisible()
    expect(screen.getByText(/当前目录及其后代/)).toBeVisible()
    expect(screen.getByText(/1 个筛选条件/)).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '清除筛选' }))
    fireEvent.click(screen.getByRole('button', { name: '搜索整个项目' }))
    expect(clear).toHaveBeenCalledOnce()
    expect(expand).toHaveBeenCalledOnce()
  })
})
