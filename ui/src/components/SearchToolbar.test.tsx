import { fireEvent, render, screen } from '@testing-library/react'
import { readFileSync } from 'node:fs'
import { describe, expect, it, vi } from 'vitest'
import type { SearchQueryModel } from '../api/types'
import { initialSearchQuery } from '../state/viewerReducer'
import SearchToolbar from './SearchToolbar'

const appCss = readFileSync('src/styles/app.css', 'utf8')

function query(overrides: Partial<SearchQueryModel> = {}): SearchQueryModel {
  return {
    ...initialSearchQuery,
    filters: { ...initialSearchQuery.filters },
    ...overrides,
  }
}

describe('SearchToolbar', () => {
  it('keeps exactly one 100px minimum width in the search input governing rule', () => {
    const searchInputRule = appCss.match(/\.search-field input\s*\{([^}]*)\}/)?.[1] ?? ''
    expect(searchInputRule.match(/min-width:\s*100px/g) ?? []).toHaveLength(1)
  })

  it('exposes named option and view menus with keyboard-controlled expanded state', () => {
    render(
      <SearchToolbar
        query={query()}
        folders={[]}
        focusRequest={0}
        onTextChange={vi.fn()}
        onScopeChange={vi.fn()}
        onFiltersChange={vi.fn()}
        onSortChange={vi.fn()}
        onLayoutChange={vi.fn()}
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
      />,
    )
    const optionsMenu = screen.getByRole('button', { name: '筛选与排序' })
    const viewMenu = screen.getByRole('button', { name: '结果视图' })
    expect(optionsMenu).toHaveAttribute('aria-expanded', 'false')
    expect(viewMenu).toHaveAttribute('aria-expanded', 'false')

    fireEvent.keyDown(optionsMenu, { key: 'Enter' })
    expect(optionsMenu).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('combobox', { name: '搜索范围' })).toBeVisible()
    fireEvent.keyDown(optionsMenu, { key: ' ' })
    expect(optionsMenu).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('combobox', { name: '搜索范围' })).not.toBeInTheDocument()

    fireEvent.keyDown(viewMenu, { key: ' ' })
    expect(viewMenu).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('button', { name: '展平结果' })).toBeVisible()
    fireEvent.keyDown(viewMenu, { key: 'Enter' })
    expect(viewMenu).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('button', { name: '展平结果' })).not.toBeInTheDocument()
  })

  it('focuses from Cmd-F intent and exposes fuzzy query plus project/subtree scope', () => {
    const onTextChange = vi.fn()
    const onScopeChange = vi.fn()
    const rendered = render(
      <SearchToolbar
        query={query()}
        folders={[
          {
            entityId: 'folder-1',
            parentEntityId: null,
            relativePath: 'catalog/id-1',
            name: 'id-1',
            marker: { reviewState: null, favorite: false },
          },
        ]}
        focusRequest={0}
        onTextChange={onTextChange}
        onScopeChange={onScopeChange}
        onFiltersChange={vi.fn()}
        onSortChange={vi.fn()}
        onLayoutChange={vi.fn()}
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
      />,
    )
    rendered.rerender(
      <SearchToolbar
        query={query()}
        folders={[]}
        focusRequest={1}
        onTextChange={onTextChange}
        onScopeChange={onScopeChange}
        onFiltersChange={vi.fn()}
        onSortChange={vi.fn()}
        onLayoutChange={vi.fn()}
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
      />,
    )
    const search = screen.getByRole('searchbox', { name: '搜索项目' })
    expect(search).toBeVisible()
    expect(search).toHaveFocus()
    fireEvent.change(search, { target: { value: '蓝色运动鞋' } })
    expect(onTextChange).toHaveBeenCalledWith('蓝色运动鞋')

    rendered.rerender(
      <SearchToolbar
        query={query()}
        folders={[
          {
            entityId: 'folder-1',
            parentEntityId: null,
            relativePath: 'catalog/id-1',
            name: 'id-1',
            marker: { reviewState: null, favorite: false },
          },
        ]}
        focusRequest={1}
        onTextChange={onTextChange}
        onScopeChange={onScopeChange}
        onFiltersChange={vi.fn()}
        onSortChange={vi.fn()}
        onLayoutChange={vi.fn()}
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
      />,
    )
    expect(screen.queryByRole('combobox', { name: '搜索范围' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByText('筛选与排序'))
    fireEvent.change(screen.getByRole('combobox', { name: '搜索范围' }), {
      target: { value: 'folder-1' },
    })
    expect(onScopeChange).toHaveBeenCalledWith('folder-1')
  })

  it('offers every frozen filter, removable chips, clear all, sorting, and layout', () => {
    const onFiltersChange = vi.fn()
    const onSortChange = vi.fn()
    const onLayoutChange = vi.fn()
    const onRemoveFilter = vi.fn()
    const onClearFilters = vi.fn()
    render(
      <SearchToolbar
        query={query({
          filters: {
            ...initialSearchQuery.filters,
            kinds: ['jpeg'],
            reviewStates: ['keep'],
            favoriteOnly: true,
          },
        })}
        folders={[]}
        focusRequest={0}
        onTextChange={vi.fn()}
        onScopeChange={vi.fn()}
        onFiltersChange={onFiltersChange}
        onSortChange={onSortChange}
        onLayoutChange={onLayoutChange}
        onRemoveFilter={onRemoveFilter}
        onClearFilters={onClearFilters}
      />,
    )
    expect(screen.getByRole('button', { name: '移除 JPEG 筛选' })).toBeVisible()
    expect(screen.queryByRole('combobox', { name: '排序方式' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByText('筛选与排序'))
    expect(screen.getByRole('combobox', { name: '搜索范围' })).toBeVisible()
    expect(screen.getByRole('combobox', { name: '排序方式' })).toBeVisible()
    for (const label of [
      'JPEG',
      'PNG',
      'Markdown',
      'TXT',
      '文件夹',
      '保留',
      '待定',
      '淘汰',
      '收藏',
      '未标记',
      '横向',
      '纵向',
      '方形',
      '最小宽度',
      '最大宽度',
      '最小高度',
      '最大高度',
      '最小文件大小',
      '最大文件大小',
      '最早修改时间',
      '最晚修改时间',
    ]) {
      expect(screen.getByLabelText(label)).toBeInTheDocument()
    }
    fireEvent.click(screen.getByLabelText('PNG'))
    expect(onFiltersChange).toHaveBeenCalledWith(
      expect.objectContaining({ kinds: ['jpeg', 'png'] }),
    )
    fireEvent.change(screen.getByLabelText('最小宽度'), { target: { value: '1200' } })
    expect(onFiltersChange).toHaveBeenCalledWith(
      expect.objectContaining({ widthMin: 1200 }),
    )

    fireEvent.click(screen.getByRole('button', { name: '移除 JPEG 筛选' }))
    expect(onRemoveFilter).toHaveBeenCalledWith({ kind: 'file_kind', value: 'jpeg' })
    fireEvent.click(screen.getByRole('button', { name: '清除全部筛选' }))
    expect(onClearFilters).toHaveBeenCalledOnce()

    fireEvent.change(screen.getByRole('combobox', { name: '排序方式' }), {
      target: { value: 'size' },
    })
    expect(onSortChange).toHaveBeenCalledWith({ key: 'size', direction: 'ascending' })
    fireEvent.click(screen.getByRole('button', { name: '切换为降序' }))
    expect(onSortChange).toHaveBeenCalledWith({
      key: 'natural_name',
      direction: 'descending',
    })
    fireEvent.click(screen.getByText('结果视图'))
    fireEvent.click(screen.getByRole('button', { name: '展平结果' }))
    expect(onLayoutChange).toHaveBeenCalledWith('flat')
  })
})
