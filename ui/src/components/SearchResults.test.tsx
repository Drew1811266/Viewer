import { fireEvent, render, screen, within } from '@testing-library/react'
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
    total: 128,
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
        relativePath: '衣服 / A01 / shoe.jpg',
        name: 'shoe.jpg',
        kind: 'jpeg',
        size: 10,
        modifiedNs: '1',
        marker: { reviewState: null, favorite: true },
        imageMetadata: { width: 1200, height: 800 },
        matchedField: 'filename',
        score: 10,
        groupRelativePath: '衣服 / A01',
        matchRanges: [{ start: 0, end: 4 }],
      },
      {
        entityId: '2',
        relativePath: '衣服 / A01 / prompt.txt',
        name: 'prompt.txt',
        kind: 'text',
        size: 20,
        modifiedNs: '2',
        marker: { reviewState: 'keep', favorite: false },
        imageMetadata: null,
        matchedField: 'body',
        score: 5,
        groupRelativePath: '衣服 / A01',
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
        searching
      />,
    )
    const region = screen.getByRole('region', { name: '搜索结果区域' })
    expect(within(region).getByText('“shoe” · 128 个结果')).toBeVisible()
    expect(within(region).getByRole('group', { name: '衣服 / A01' })).toBeVisible()
    expect(within(region).getByRole('navigation', { name: '搜索结果分页' })).toBeVisible()
    expect(within(region).getByText('文件名与路径匹配')).toHaveClass('search-result-context')
    const indexing = screen.getByRole('status')
    expect(indexing).toHaveClass('search-indexing-banner')
    expect(indexing).toHaveTextContent('图片 8/10 · 文本 1/2')
    expect(screen.getByText('shoe', { selector: 'mark' })).toBeVisible()
    const result = screen.getByRole('option', { name: 'shoe.jpg 衣服 / A01 / shoe.jpg' })
    expect(result.closest('.search-result-row')).toHaveStyle({ height: '48px' })
    expect(
      screen.getByRole('group', { name: '衣服 / A01' }).closest('.search-result-row'),
    ).toHaveStyle({ height: '28px' })
    expect(result).not.toHaveAttribute('tabindex')
    expect(result).toHaveClass('search-result-item')
    expect(within(result).getByText('JPG')).toHaveClass('viewer-status-tag')
    expect(within(result).getByText('1200 × 800 · 10 B')).toHaveClass('search-result-metadata')
    expect(within(result).getByText('未标记 · 收藏')).toHaveClass('viewer-status-tag')
    expect(screen.getByTestId('search-snippet-2')).toHaveTextContent('片'.repeat(160))
    expect(screen.getByTestId('search-snippet-2')).not.toHaveTextContent('片'.repeat(161))
    expect(visible).toHaveBeenCalledWith(expect.arrayContaining(['1', '2']))
  })

  it('uses a flat list surface instead of wrapping search results in a card', () => {
    render(
      <SearchResults
        page={page()}
        query={{ ...query, layout: 'flat' }}
        snippets={{}}
        offset={0}
        limit={200}
        onPageChange={vi.fn()}
        onVisibleHits={vi.fn()}
        onClearFilters={vi.fn()}
        onSearchProject={vi.fn()}
        onReturnToFolder={vi.fn()}
        searching={false}
      />,
    )

    expect(screen.getByRole('listbox', { name: '搜索结果' })).toHaveClass(
      'search-result-list',
      'search-result-list-flat',
    )
  })

  it('keeps the same formal result row structure in flat layout', () => {
    render(
      <SearchResults
        page={page()}
        query={{ ...query, layout: 'flat' }}
        snippets={{}}
        offset={0}
        limit={200}
        onPageChange={vi.fn()}
        onVisibleHits={vi.fn()}
        onClearFilters={vi.fn()}
        onSearchProject={vi.fn()}
        onReturnToFolder={vi.fn()}
        searching={false}
      />,
    )

    expect(screen.queryByRole('group', { name: '衣服 / A01' })).not.toBeInTheDocument()
    expect(screen.getAllByRole('option')).toHaveLength(2)
    for (const result of screen.getAllByRole('option')) {
      expect(result).toHaveClass('search-result-item')
      expect(result.querySelector('.search-result-path')).not.toBeNull()
      expect(result.querySelector('.search-result-metadata')).not.toBeNull()
      expect(result.querySelector('.viewer-status-tag')).not.toBeNull()
    }
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
        searching={false}
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
        searching={false}
      />,
    )
    expect(screen.getByText(/shoe/)).toBeVisible()
    expect(screen.getByText(/当前目录及其后代/)).toBeVisible()
    expect(screen.getByText(/1 个筛选条件/)).toBeVisible()
    expect(screen.getByRole('heading', { name: '没有找到结果' }).closest('section')).toHaveClass(
      'viewer-empty-state',
    )
    expect(screen.getByRole('button', { name: '清除筛选' })).toHaveAttribute('data-tone', 'primary')
    fireEvent.click(screen.getByRole('button', { name: '清除筛选' }))
    fireEvent.click(screen.getByRole('button', { name: '搜索整个项目' }))
    expect(clear).toHaveBeenCalledOnce()
    expect(expand).toHaveBeenCalledOnce()
  })
})
