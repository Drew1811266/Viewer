import { useEffect, useMemo, useRef, useState } from 'react'
import type {
  FileKind,
  FolderTreeItem,
  ImageOrientation,
  ReviewState,
  SearchFilters,
  SearchQueryModel,
  SearchSort,
} from '../api/types'
import type { SearchFilterChip } from '../state/viewerReducer'
import ViewerButton, { ViewerIconButton } from './ui/ViewerButton'
import ViewerChoiceChip from './ui/ViewerChoiceChip'
import ViewerField from './ui/ViewerField'
import ViewerIcon from './ui/ViewerIcon'
import ViewerPopover from './ui/ViewerPopover'

export interface SearchToolbarProps {
  filterOpen: boolean
  onFilterOpenChange(open: boolean): void
  query: SearchQueryModel
  folders: FolderTreeItem[]
  focusRequest: number
  onTextChange: (text: string) => void
  onScopeChange: (folderId: string | null) => void
  onFiltersChange: (filters: SearchFilters) => void
  onSortChange: (sort: SearchSort) => void
  onRemoveFilter: (chip: SearchFilterChip) => void
  onClearFilters: () => void
}

const FILE_KINDS: Array<[FileKind, string]> = [
  ['jpeg', 'JPEG'],
  ['png', 'PNG'],
  ['unsupported_image', '其它图片'],
  ['video', '视频'],
  ['markdown', 'Markdown'],
  ['text', 'TXT'],
  ['other', '其它文件'],
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
  filterOpen,
  onFilterOpenChange,
  query,
  folders,
  focusRequest,
  onTextChange,
  onScopeChange,
  onFiltersChange,
  onSortChange,
  onRemoveFilter,
  onClearFilters,
}: SearchToolbarProps) {
  const searchRef = useRef<HTMLInputElement>(null)
  const filterTriggerRef = useRef<HTMLElement>(null)
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [advancedEditing, setAdvancedEditing] = useState(false)
  useEffect(() => {
    if (focusRequest > 0) searchRef.current?.focus()
  }, [focusRequest])
  const closeOptions = () => {
    onFilterOpenChange(false)
    filterTriggerRef.current?.focus()
  }
  const toggleOptions = () => onFilterOpenChange(!filterOpen)
  const chips = useMemo(() => filterChips(query.filters), [query.filters])
  const advancedRules = useMemo(() => advancedFilterRules(query.filters), [query.filters])
  useEffect(() => {
    if (!filterOpen) {
      setAdvancedOpen(false)
      setAdvancedEditing(false)
      return
    }
    if (advancedRules.length > 0) setAdvancedOpen(true)
  }, [advancedRules.length, filterOpen])
  const orientationControls = (
    <>
      <legend>方向</legend>
      {ORIENTATIONS.map(([value, label]) => (
        <CheckFilter
          key={value}
          label={label}
          checked={query.filters.orientations.includes(value)}
          onChange={(checked) =>
            onFiltersChange({
              ...query.filters,
              orientations: toggleValue(query.filters.orientations, value, checked),
            })
          }
        />
      ))}
    </>
  )
  const dimensionControls = (
    <>
      <legend>像素尺寸</legend>
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
    </>
  )
  const fileSizeControls = (
    <>
      <legend>文件大小</legend>
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
    </>
  )
  const modifiedTimeControls = (
    <>
      <legend>修改时间</legend>
      <DateFilter
        label="最早修改时间"
        value={query.filters.modifiedNsMin}
        onChange={(modifiedNsMin) => onFiltersChange({ ...query.filters, modifiedNsMin })}
      />
      <DateFilter
        label="最晚修改时间"
        value={query.filters.modifiedNsMax}
        onChange={(modifiedNsMax) => onFiltersChange({ ...query.filters, modifiedNsMax })}
      />
    </>
  )
  const scopeAndSortControls = (
    <div className="filter-scope-sort">
      <ViewerField label="范围" className="filter-scope-field">
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
      </ViewerField>
      <ViewerField label="排序" className="filter-sort-field">
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
      </ViewerField>
      <ViewerIconButton
        icon={query.sort.direction === 'ascending' ? 'arrow-up' : 'arrow-down'}
        label={query.sort.direction === 'ascending' ? '切换为降序' : '切换为升序'}
        tone="quiet"
        onClick={() =>
          onSortChange({
            ...query.sort,
            direction: query.sort.direction === 'ascending' ? 'descending' : 'ascending',
          })
        }
      />
    </div>
  )
  const fileKindAndReviewControls = (
    <>
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
                reviewStates: toggleValue(query.filters.reviewStates, value, checked),
              })
            }
          />
        ))}
        <CheckFilter
          label="收藏"
          checked={query.filters.favoriteOnly}
          onChange={(favoriteOnly) => onFiltersChange({ ...query.filters, favoriteOnly })}
        />
        <CheckFilter
          label="未标记"
          checked={query.filters.unmarkedOnly}
          onChange={(unmarkedOnly) => onFiltersChange({ ...query.filters, unmarkedOnly })}
        />
      </fieldset>
    </>
  )
  const activeFilterChipsAndClearAction = (
    <>
      {chips.length > 0 ? (
        <div className="search-filter-chips" aria-label="已启用筛选">
          {chips.map(({ chip, label }) => (
            <ViewerButton
              key={chipKey(chip)}
              aria-label={`移除 ${label} 筛选`}
              className="search-filter-chip"
              leadingIcon="x"
              tone="quiet"
              onClick={() => onRemoveFilter(chip)}
            >
              {label}
            </ViewerButton>
          ))}
        </div>
      ) : (
        <span>未启用筛选条件</span>
      )}
    </>
  )

  return (
    <section className="search-toolbar" aria-label="搜索和筛选">
      <label className="search-field">
        <span className="visually-hidden">搜索项目</span>
        <ViewerIcon name="search" />
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
      <details className="search-options-panel" open={filterOpen}>
        <summary
          ref={filterTriggerRef}
          role="button"
          aria-expanded={filterOpen}
          onClick={(event) => {
            event.preventDefault()
            toggleOptions()
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'Spacebar') return
            event.preventDefault()
            toggleOptions()
          }}
          aria-label={chips.length > 0 ? `筛选，${chips.length} 项已启用` : '筛选'}
        >
          <span>筛选</span>
          {chips.length > 0 && <span className="filter-count">{chips.length}</span>}
        </summary>
        <ViewerPopover
          className="search-options-popover"
          open={filterOpen}
          label="筛选条件"
          triggerRef={filterTriggerRef}
          onOpenChange={onFilterOpenChange}
        >
          <header className="filter-popover-header">
            <h2>筛选</h2>
            <ViewerIconButton icon="x" label="关闭筛选" tone="quiet" onClick={closeOptions} />
          </header>
          {scopeAndSortControls}
          <div className="common-filter-grid">{fileKindAndReviewControls}</div>
          <details className="advanced-filter-group" open={advancedOpen}>
            <summary
              role="button"
              aria-expanded={advancedOpen}
              onClick={(event) => {
                event.preventDefault()
                if (advancedOpen) {
                  setAdvancedOpen(false)
                  return
                }
                setAdvancedOpen(true)
                setAdvancedEditing(advancedRules.length === 0)
              }}
            >
              <span>高级条件</span>
              <ViewerIcon name="chevron-down" size={14} />
            </summary>
            {advancedEditing || advancedRules.length === 0 ? (
              <div className="advanced-filter-editor">
                <div className="advanced-filter-grid">
                  <fieldset aria-label="方向">{orientationControls}</fieldset>
                  <fieldset aria-label="像素尺寸">{dimensionControls}</fieldset>
                  <fieldset aria-label="文件大小">{fileSizeControls}</fieldset>
                  <fieldset aria-label="修改时间">{modifiedTimeControls}</fieldset>
                </div>
                {advancedRules.length > 0 && (
                  <div className="advanced-filter-editor-actions">
                    <ViewerButton tone="quiet" onClick={() => setAdvancedEditing(false)}>
                      完成高级条件编辑
                    </ViewerButton>
                  </div>
                )}
              </div>
            ) : (
              <div className="advanced-filter-rules" aria-label="已启用高级条件">
                {advancedRules.map(({ chip, label, name }) => (
                  <div className="advanced-filter-rule" key={chipKey(chip)}>
                    <span>{label}</span>
                    <ViewerButton
                      tone="quiet"
                      aria-label={`移除 ${name} 筛选`}
                      onClick={() => onRemoveFilter(chip)}
                    >
                      移除
                    </ViewerButton>
                  </div>
                ))}
                <div className="advanced-filter-editor-actions">
                  <ViewerButton tone="quiet" onClick={() => setAdvancedEditing(true)}>
                    编辑高级条件
                  </ViewerButton>
                </div>
              </div>
            )}
          </details>
          <div className="active-filter-summary">{activeFilterChipsAndClearAction}</div>
          <footer className="viewer-filter-footer">
            <ViewerButton tone="quiet" disabled={chips.length === 0} onClick={onClearFilters}>
              清除全部
            </ViewerButton>
            <ViewerButton tone="primary" onClick={() => onFilterOpenChange(false)}>
              完成
            </ViewerButton>
          </footer>
        </ViewerPopover>
      </details>
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
    <ViewerChoiceChip checked={checked} onCheckedChange={onChange}>
      {label}
    </ViewerChoiceChip>
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
    <ViewerField label={label} className="advanced-filter-field">
      <input
        type="number"
        min="0"
        aria-label={label}
        value={value ?? ''}
        onChange={(event) =>
          onChange(event.currentTarget.value === '' ? null : Number(event.currentTarget.value))
        }
      />
    </ViewerField>
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
    <ViewerField label={label} className="advanced-filter-field">
      <input
        type="datetime-local"
        aria-label={label}
        value={nanosecondsToLocal(value)}
        onChange={(event) => onChange(localToNanoseconds(event.currentTarget.value))}
      />
    </ViewerField>
  )
}

function toggleValue<T>(values: T[], value: T, checked: boolean): T[] {
  return checked
    ? [...values.filter((item) => item !== value), value]
    : values.filter((item) => item !== value)
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

function advancedFilterRules(
  filters: SearchFilters,
): Array<{ chip: SearchFilterChip; label: string; name: string }> {
  const rules: Array<{ chip: SearchFilterChip; label: string; name: string }> = []
  for (const value of filters.orientations) {
    rules.push({
      chip: { kind: 'orientation', value },
      label: `方向 · ${labelFor(ORIENTATIONS, value)}`,
      name: '方向',
    })
  }
  if (filters.widthMin !== null || filters.widthMax !== null) {
    rules.push({
      chip: { kind: 'range', field: 'width' },
      label: numericRangeLabel('像素宽度', filters.widthMin, filters.widthMax, ' px'),
      name: '像素宽度',
    })
  }
  if (filters.heightMin !== null || filters.heightMax !== null) {
    rules.push({
      chip: { kind: 'range', field: 'height' },
      label: numericRangeLabel('像素高度', filters.heightMin, filters.heightMax, ' px'),
      name: '像素高度',
    })
  }
  if (filters.sizeMin !== null || filters.sizeMax !== null) {
    rules.push({
      chip: { kind: 'range', field: 'size' },
      label: numericRangeLabel('文件大小', filters.sizeMin, filters.sizeMax, ' B'),
      name: '文件大小',
    })
  }
  if (filters.modifiedNsMin !== null || filters.modifiedNsMax !== null) {
    rules.push({
      chip: { kind: 'range', field: 'modified_ns' },
      label: dateRangeLabel(filters.modifiedNsMin, filters.modifiedNsMax),
      name: '修改时间',
    })
  }
  return rules
}

function numericRangeLabel(
  title: string,
  minimum: number | null,
  maximum: number | null,
  unit: string,
): string {
  if (minimum !== null && maximum !== null) return `${title} · ${minimum}–${maximum}${unit}`
  if (minimum !== null) return `${title} · 至少 ${minimum}${unit}`
  return `${title} · 至多 ${maximum ?? 0}${unit}`
}

function dateRangeLabel(minimum: string | null, maximum: string | null): string {
  const start = minimum === null ? null : formatRuleDate(minimum)
  const end = maximum === null ? null : formatRuleDate(maximum)
  if (start !== null && end !== null) return `修改时间 · ${start}–${end}`
  if (start !== null) return `修改时间 · 从 ${start}`
  return `修改时间 · 至 ${end ?? ''}`
}

function formatRuleDate(value: string): string {
  return nanosecondsToLocal(value).replace('T', ' ')
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
