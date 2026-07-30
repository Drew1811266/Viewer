import { readFileSync } from 'node:fs'
import { fireEvent, render, screen } from '@testing-library/react'
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

  it('exposes a named filter menu with keyboard-controlled expanded state', () => {
    render(
      <SearchToolbar
        query={query()}
        folders={[]}
        focusRequest={0}
        onTextChange={vi.fn()}
        onScopeChange={vi.fn()}
        onFiltersChange={vi.fn()}
        onSortChange={vi.fn()}
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
      />,
    )
    const optionsMenu = screen.getByRole('button', { name: '筛选' })
    expect(optionsMenu).toHaveAttribute('aria-expanded', 'false')

    fireEvent.keyDown(optionsMenu, { key: 'Enter' })
    expect(optionsMenu).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('combobox', { name: '搜索范围' })).toBeVisible()
    fireEvent.keyDown(optionsMenu, { key: ' ' })
    expect(optionsMenu).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('combobox', { name: '搜索范围' })).not.toBeInTheDocument()
  })

  it('keeps common filters visible and reveals advanced conditions on demand', () => {
    render(
      <SearchToolbar
        query={query()}
        folders={[]}
        focusRequest={0}
        onTextChange={vi.fn()}
        onScopeChange={vi.fn()}
        onFiltersChange={vi.fn()}
        onSortChange={vi.fn()}
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: /筛选/ }))
    expect(screen.getByRole('group', { name: '文件类型' })).toBeVisible()
    expect(screen.getByRole('group', { name: '审阅状态' })).toBeVisible()
    expect(screen.getByLabelText('最小宽度')).not.toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '高级条件' }))
    expect(screen.getByLabelText('最小宽度')).toBeVisible()
    expect(screen.getByLabelText('最晚修改时间')).toBeVisible()
  })

  it('returns focus to the filter trigger after closing the popover', () => {
    render(
      <SearchToolbar
        query={query()}
        folders={[]}
        focusRequest={0}
        onTextChange={vi.fn()}
        onScopeChange={vi.fn()}
        onFiltersChange={vi.fn()}
        onSortChange={vi.fn()}
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
      />,
    )

    const trigger = screen.getByRole('button', { name: '筛选' })
    fireEvent.click(trigger)
    const close = screen.getByRole('button', { name: '关闭筛选' })
    close.focus()
    expect(close).toHaveFocus()
    fireEvent.click(close)
    expect(document.activeElement).toBe(trigger)
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
        onRemoveFilter={vi.fn()}
        onClearFilters={vi.fn()}
      />,
    )
    expect(screen.queryByRole('combobox', { name: '搜索范围' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '筛选' }))
    fireEvent.change(screen.getByRole('combobox', { name: '搜索范围' }), {
      target: { value: 'folder-1' },
    })
    expect(onScopeChange).toHaveBeenCalledWith('folder-1')
  })

  it('offers every frozen filter, removable chips, clear all, and sorting', () => {
    const onFiltersChange = vi.fn()
    const onSortChange = vi.fn()
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
        onRemoveFilter={onRemoveFilter}
        onClearFilters={onClearFilters}
      />,
    )
    expect(screen.queryByRole('button', { name: '移除 JPEG 筛选' })).not.toBeInTheDocument()
    expect(screen.queryByRole('combobox', { name: '排序方式' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '筛选，3 项已启用' }))
    expect(screen.getByRole('combobox', { name: '搜索范围' })).toBeVisible()
    expect(screen.getByRole('combobox', { name: '排序方式' })).toBeVisible()
    expect(screen.getByRole('button', { name: '移除 JPEG 筛选' })).toBeVisible()
    for (const label of [
      'JPEG',
      'PNG',
      '其它图片',
      'Markdown',
      'TXT',
      '其它文件',
      '文件夹',
      '保留',
      '待定',
      '淘汰',
      '收藏',
      '未标记',
    ]) {
      expect(screen.getByLabelText(label)).toBeInTheDocument()
    }
    fireEvent.click(screen.getByLabelText('PNG'))
    expect(onFiltersChange).toHaveBeenCalledWith(
      expect.objectContaining({ kinds: ['jpeg', 'png'] }),
    )
    fireEvent.click(screen.getByRole('button', { name: '高级条件' }))
    fireEvent.change(screen.getByLabelText('最小宽度'), { target: { value: '1200' } })
    expect(onFiltersChange).toHaveBeenCalledWith(expect.objectContaining({ widthMin: 1200 }))

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
  })
})
