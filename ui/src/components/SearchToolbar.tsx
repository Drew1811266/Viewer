import { useEffect, useMemo, useRef, useState } from 'react'
import type {
  FileKind,
  FolderTreeItem,
  ImageOrientation,
  ReviewState,
  SearchFilters,
  SearchLayout,
  SearchQueryModel,
  SearchSort,
} from '../api/types'
import type { SearchFilterChip } from '../state/viewerReducer'

interface SearchToolbarProps {
  query: SearchQueryModel
  folders: FolderTreeItem[]
  focusRequest: number
  onTextChange: (text: string) => void
  onScopeChange: (folderId: string | null) => void
  onFiltersChange: (filters: SearchFilters) => void
  onSortChange: (sort: SearchSort) => void
  onLayoutChange: (layout: SearchLayout) => void
  onRemoveFilter: (chip: SearchFilterChip) => void
  onClearFilters: () => void
}

const FILE_KINDS: Array<[FileKind, string]> = [
  ['jpeg', 'JPEG'],
  ['png', 'PNG'],
  ['markdown', 'Markdown'],
  ['text', 'TXT'],
  ['directory', '文件夹'],
]
const REVIEW_STATES: Array<[ReviewState, string]> = [
  ['keep', '保留'],
  ['pending', '待定'],
  ['reject', '淘汰'],
]
const ORIENTATIONS: Array<[ImageOrientation, string]> = [
  ['landscape', '横向'],
  ['portrait', '纵向'],
  ['square', '方形'],
]

export default function SearchToolbar({
  query,
  folders,
  focusRequest,
  onTextChange,
  onScopeChange,
  onFiltersChange,
  onSortChange,
  onLayoutChange,
  onRemoveFilter,
  onClearFilters,
}: SearchToolbarProps) {
  const searchRef = useRef<HTMLInputElement>(null)
  const [optionsOpen, setOptionsOpen] = useState(false)
  const [viewOpen, setViewOpen] = useState(false)
  useEffect(() => {
    if (focusRequest > 0) searchRef.current?.focus()
  }, [focusRequest])
  const chips = useMemo(() => filterChips(query.filters), [query.filters])

  return (
    <section className="search-toolbar" aria-label="搜索和筛选">
      <label className="search-field">
        <span className="visually-hidden">搜索项目</span>
        <span aria-hidden="true">⌕</span>
        <input
          ref={searchRef}
          type="search"
          aria-label="搜索项目"
          placeholder="搜索名称、路径或文本"
          value={query.text}
          onChange={(event) => onTextChange(event.currentTarget.value)}
        />
        <kbd>⌘F</kbd>
      </label>
      <details
        className="search-options-panel"
        open={optionsOpen}
      >
        <summary
          role="button"
          aria-expanded={optionsOpen}
          onClick={(event) => {
            event.preventDefault()
            setOptionsOpen((open) => !open)
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'Spacebar') return
            event.preventDefault()
            setOptionsOpen((open) => !open)
          }}
        >
          筛选与排序
        </summary>
        <div className="search-options-popover" hidden={!optionsOpen}>
        <label>
          <span>范围</span>
          <select
            aria-label="搜索范围"
            value={query.scopeFolderId ?? ''}
            onChange={(event) => onScopeChange(event.currentTarget.value || null)}
          >
            <option value="">整个项目</option>
            {folders.map((folder) => (
              <option key={folder.entityId} value={folder.entityId}>
                {folder.relativePath}
              </option>
            ))}
          </select>
        </label>
        <div className="search-filter-section">
          <div className="search-filter-grid">
            <fieldset>
              <legend>文件类型</legend>
              {FILE_KINDS.map(([value, label]) => (
                <CheckFilter
                  key={value}
                  label={label}
                  checked={query.filters.kinds.includes(value)}
                  onChange={(checked) =>
                    onFiltersChange({
                      ...query.filters,
                      kinds: toggleValue(query.filters.kinds, value, checked),
                    })
                  }
                />
              ))}
            </fieldset>
            <fieldset>
              <legend>审阅状态</legend>
              {REVIEW_STATES.map(([value, label]) => (
                <CheckFilter
                  key={value}
                  label={label}
                  checked={query.filters.reviewStates.includes(value)}
                  onChange={(checked) =>
                    onFiltersChange({
                      ...query.filters,
                      reviewStates: toggleValue(
                        query.filters.reviewStates,
                        value,
                        checked,
                      ),
                    })
                  }
                />
              ))}
              <CheckFilter
                label="收藏"
                checked={query.filters.favoriteOnly}
                onChange={(favoriteOnly) =>
                  onFiltersChange({ ...query.filters, favoriteOnly })
                }
              />
              <CheckFilter
                label="未标记"
                checked={query.filters.unmarkedOnly}
                onChange={(unmarkedOnly) =>
                  onFiltersChange({ ...query.filters, unmarkedOnly })
                }
              />
            </fieldset>
            <fieldset>
              <legend>图片方向</legend>
              {ORIENTATIONS.map(([value, label]) => (
                <CheckFilter
                  key={value}
                  label={label}
                  checked={query.filters.orientations.includes(value)}
                  onChange={(checked) =>
                    onFiltersChange({
                      ...query.filters,
                      orientations: toggleValue(
                        query.filters.orientations,
                        value,
                        checked,
                      ),
                    })
                  }
                />
              ))}
            </fieldset>
            <fieldset className="numeric-filters">
              <legend>尺寸与时间</legend>
              <NumberFilter
                label="最小宽度"
                value={query.filters.widthMin}
                onChange={(widthMin) => onFiltersChange({ ...query.filters, widthMin })}
              />
              <NumberFilter
                label="最大宽度"
                value={query.filters.widthMax}
                onChange={(widthMax) => onFiltersChange({ ...query.filters, widthMax })}
              />
              <NumberFilter
                label="最小高度"
                value={query.filters.heightMin}
                onChange={(heightMin) => onFiltersChange({ ...query.filters, heightMin })}
              />
              <NumberFilter
                label="最大高度"
                value={query.filters.heightMax}
                onChange={(heightMax) => onFiltersChange({ ...query.filters, heightMax })}
              />
              <NumberFilter
                label="最小文件大小"
                value={query.filters.sizeMin}
                onChange={(sizeMin) => onFiltersChange({ ...query.filters, sizeMin })}
              />
              <NumberFilter
                label="最大文件大小"
                value={query.filters.sizeMax}
                onChange={(sizeMax) => onFiltersChange({ ...query.filters, sizeMax })}
              />
              <DateFilter
                label="最早修改时间"
                value={query.filters.modifiedNsMin}
                onChange={(modifiedNsMin) =>
                  onFiltersChange({ ...query.filters, modifiedNsMin })
                }
              />
              <DateFilter
                label="最晚修改时间"
                value={query.filters.modifiedNsMax}
                onChange={(modifiedNsMax) =>
                  onFiltersChange({ ...query.filters, modifiedNsMax })
                }
              />
            </fieldset>
          </div>
        </div>
        <label>
          <span>排序</span>
          <select
            aria-label="排序方式"
            value={query.sort.key}
            onChange={(event) =>
              onSortChange({
                key: event.currentTarget.value as SearchSort['key'],
                direction: query.sort.direction,
              })
            }
          >
            <option value="relevance">相关度</option>
            <option value="natural_name">文件名</option>
            <option value="modified_time">修改时间</option>
            <option value="size">文件大小</option>
            <option value="pixel_dimensions">像素尺寸</option>
            <option value="review_state">审阅状态</option>
          </select>
        </label>
        <button
          type="button"
          aria-label={query.sort.direction === 'ascending' ? '切换为降序' : '切换为升序'}
          onClick={() =>
            onSortChange({
              ...query.sort,
              direction:
                query.sort.direction === 'ascending' ? 'descending' : 'ascending',
            })
          }
        >
          {query.sort.direction === 'ascending' ? '↑' : '↓'}
        </button>
        </div>
      </details>
      <details
        className="search-view-panel"
        open={viewOpen}
      >
        <summary
          role="button"
          aria-expanded={viewOpen}
          onClick={(event) => {
            event.preventDefault()
            setViewOpen((open) => !open)
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'Spacebar') return
            event.preventDefault()
            setViewOpen((open) => !open)
          }}
        >
          结果视图
        </summary>
        <div className="search-view-popover search-layout-toggle" hidden={!viewOpen}>
          <button
            type="button"
            aria-label="按文件夹分组"
            aria-pressed={query.layout === 'grouped'}
            onClick={() => onLayoutChange('grouped')}
          >
            分组
          </button>
          <button
            type="button"
            aria-label="展平结果"
            aria-pressed={query.layout === 'flat'}
            onClick={() => onLayoutChange('flat')}
          >
            展平
          </button>
        </div>
      </details>
      {chips.length > 0 && (
        <div className="search-filter-chips" aria-label="已启用筛选">
          {chips.map(({ chip, label }) => (
            <button
              type="button"
              key={chipKey(chip)}
              aria-label={`移除 ${label} 筛选`}
              onClick={() => onRemoveFilter(chip)}
            >
              {label} ×
            </button>
          ))}
          <button type="button" aria-label="清除全部筛选" onClick={onClearFilters}>
            清除全部
          </button>
        </div>
      )}
    </section>
  )
}

function CheckFilter({
  label,
  checked,
  onChange,
}: {
  label: string
  checked: boolean
  onChange: (checked: boolean) => void
}) {
  return (
    <label>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      {label}
    </label>
  )
}

function NumberFilter({
  label,
  value,
  onChange,
}: {
  label: string
  value: number | null
  onChange: (value: number | null) => void
}) {
  return (
    <label>
      {label}
      <input
        type="number"
        min="0"
        aria-label={label}
        value={value ?? ''}
        onChange={(event) =>
          onChange(event.currentTarget.value === '' ? null : Number(event.currentTarget.value))
        }
      />
    </label>
  )
}

function DateFilter({
  label,
  value,
  onChange,
}: {
  label: string
  value: string | null
  onChange: (value: string | null) => void
}) {
  return (
    <label>
      {label}
      <input
        type="datetime-local"
        aria-label={label}
        value={nanosecondsToLocal(value)}
        onChange={(event) => onChange(localToNanoseconds(event.currentTarget.value))}
      />
    </label>
  )
}

function toggleValue<T>(values: T[], value: T, checked: boolean): T[] {
  return checked ? [...values.filter((item) => item !== value), value] : values.filter((item) => item !== value)
}

function filterChips(filters: SearchFilters): Array<{ chip: SearchFilterChip; label: string }> {
  const chips: Array<{ chip: SearchFilterChip; label: string }> = []
  for (const value of filters.kinds) {
    chips.push({ chip: { kind: 'file_kind', value }, label: labelFor(FILE_KINDS, value) })
  }
  for (const value of filters.reviewStates) {
    chips.push({
      chip: { kind: 'review_state', value },
      label: labelFor(REVIEW_STATES, value),
    })
  }
  for (const value of filters.orientations) {
    chips.push({
      chip: { kind: 'orientation', value },
      label: labelFor(ORIENTATIONS, value),
    })
  }
  if (filters.favoriteOnly) chips.push({ chip: { kind: 'favorite' }, label: '收藏' })
  if (filters.unmarkedOnly) chips.push({ chip: { kind: 'unmarked' }, label: '未标记' })
  if (filters.widthMin !== null || filters.widthMax !== null) {
    chips.push({ chip: { kind: 'range', field: 'width' }, label: '宽度范围' })
  }
  if (filters.heightMin !== null || filters.heightMax !== null) {
    chips.push({ chip: { kind: 'range', field: 'height' }, label: '高度范围' })
  }
  if (filters.sizeMin !== null || filters.sizeMax !== null) {
    chips.push({ chip: { kind: 'range', field: 'size' }, label: '大小范围' })
  }
  if (filters.modifiedNsMin !== null || filters.modifiedNsMax !== null) {
    chips.push({ chip: { kind: 'range', field: 'modified_ns' }, label: '修改时间' })
  }
  return chips
}

function labelFor<T>(options: Array<[T, string]>, value: T): string {
  return options.find(([candidate]) => candidate === value)?.[1] ?? String(value)
}

function chipKey(chip: SearchFilterChip): string {
  if ('value' in chip) return `${chip.kind}:${chip.value}`
  if (chip.kind === 'range') return `${chip.kind}:${chip.field}`
  return chip.kind
}

function nanosecondsToLocal(value: string | null): string {
  if (value === null) return ''
  try {
    const milliseconds = Number(BigInt(value) / 1_000_000n)
    const date = new Date(milliseconds)
    const offset = date.getTimezoneOffset() * 60_000
    return new Date(milliseconds - offset).toISOString().slice(0, 16)
  } catch {
    return ''
  }
}

function localToNanoseconds(value: string): string | null {
  if (value === '') return null
  const milliseconds = Date.parse(value)
  return Number.isFinite(milliseconds) ? String(BigInt(milliseconds) * 1_000_000n) : null
}
