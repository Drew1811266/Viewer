import type { UIEvent } from 'react'
import { useEffect, useMemo, useState } from 'react'
import type { MatchRange, SearchHit, SearchPage, SearchQueryModel } from '../api/types'

interface SearchResultsProps {
  page: SearchPage
  query: SearchQueryModel
  snippets: Record<string, string | null>
  offset: number
  limit: number
  onPageChange: (offset: number) => void
  onVisibleHits: (entityIds: string[]) => void
  onClearFilters: () => void
  onSearchProject: () => void
  onReturnToFolder: () => void
  searching: boolean
}

type ResultRow =
  | { type: 'group'; key: string; path: string; count: number; height: number }
  | { type: 'hit'; key: string; hit: SearchHit; height: number }

const RESULT_ROW_HEIGHT = 48
const GROUP_ROW_HEIGHT = 28
const VIEWPORT_HEIGHT = 520
const OVERSCAN = 5

export default function SearchResults({
  page,
  query,
  snippets,
  offset,
  limit,
  onPageChange,
  onVisibleHits,
  onClearFilters,
  onSearchProject,
  onReturnToFolder,
  searching,
}: SearchResultsProps) {
  const [scrollTop, setScrollTop] = useState(0)
  const rows = useMemo(() => resultRows(page.hits, query.layout), [page.hits, query.layout])
  const rowOffsets = useMemo(() => rowOffsetsFor(rows), [rows])
  const totalRowsHeight = rows.reduce((height, row) => height + row.height, 0)
  const firstVisible =
    rows.length === 0
      ? -1
      : rowOffsets.findIndex((offset, index) => {
          const row = rows[index]
          return row !== undefined && offset + row.height > scrollTop
        })
  const firstVisibleIndex = firstVisible === -1 ? Math.max(0, rows.length - 1) : firstVisible
  const lastVisible = rowOffsets.findIndex((offset) => offset >= scrollTop + VIEWPORT_HEIGHT)
  const lastVisibleIndex = lastVisible === -1 ? rows.length : lastVisible
  const start = Math.max(0, firstVisibleIndex - OVERSCAN)
  const end = Math.min(rows.length, lastVisibleIndex + OVERSCAN)
  const visibleRows = useMemo(() => rows.slice(start, end), [end, rows, start])
  const visibleIds = useMemo(
    () => visibleRows.flatMap((row) => (row.type === 'hit' ? [row.hit.entityId] : [])),
    [visibleRows],
  )
  useEffect(() => onVisibleHits(visibleIds), [onVisibleHits, visibleIds])

  if (page.total === 0) {
    const filterCount = activeFilterCount(query)
    return (
      <section className="search-empty" aria-label="无搜索结果">
        <h2>没有找到结果</h2>
        <p>
          关键词“{query.text || '（空）'}”，范围：
          {query.scopeFolderId === null ? '整个项目' : '当前目录及其后代'}，{filterCount}{' '}
          个筛选条件。
        </p>
        <div>
          {filterCount > 0 && (
            <button type="button" onClick={onClearFilters}>
              清除筛选
            </button>
          )}
          {query.scopeFolderId !== null && (
            <button type="button" onClick={onSearchProject}>
              搜索整个项目
            </button>
          )}
          <button type="button" onClick={onReturnToFolder}>
            返回文件夹内容
          </button>
        </div>
      </section>
    )
  }

  return (
    <section className="search-results" aria-label="搜索结果区域">
      <header className="search-results-summary">
        <div>
          <strong>{page.total} 个结果</strong>
          {searching && <span role="status">结果仍在更新</span>}
        </div>
        <button type="button" onClick={onReturnToFolder}>
          返回文件夹内容
        </button>
      </header>
      <div
        role="listbox"
        aria-label="搜索结果"
        className="search-result-list"
        style={{ height: VIEWPORT_HEIGHT, overflowY: 'auto', position: 'relative' }}
        onScroll={(event: UIEvent<HTMLDivElement>) => setScrollTop(event.currentTarget.scrollTop)}
      >
        <div style={{ height: totalRowsHeight, position: 'relative' }}>
          {visibleRows.map((row, visibleIndex) => {
            const index = start + visibleIndex
            return (
              <div
                key={row.key}
                className={`search-result-row search-result-${row.type}`}
                style={{
                  height: row.height,
                  left: 0,
                  position: 'absolute',
                  right: 0,
                  top: rowOffsets[index],
                }}
              >
                {row.type === 'group' ? (
                  <div className="search-result-group" role="group" aria-label={row.path}>
                    <strong>{row.path}</strong>
                    <span>{row.count} 项</span>
                  </div>
                ) : (
                  <ResultItem hit={row.hit} snippet={snippets[row.hit.entityId]} />
                )}
              </div>
            )
          })}
        </div>
      </div>
      <nav className="search-pagination" aria-label="搜索结果分页">
        <button
          type="button"
          disabled={offset === 0}
          onClick={() => onPageChange(Math.max(0, offset - limit))}
        >
          上一页
        </button>
        <span>
          {offset + 1}–{Math.min(offset + page.hits.length, page.total)} / {page.total}
        </span>
        <button
          type="button"
          disabled={offset + page.hits.length >= page.total}
          onClick={() => onPageChange(offset + limit)}
        >
          下一页
        </button>
      </nav>
    </section>
  )
}

function ResultItem({ hit, snippet }: { hit: SearchHit; snippet: string | null | undefined }) {
  const nameRanges =
    hit.matchedField === 'filename' || hit.matchedField === 'exact_filename' ? hit.matchRanges : []
  const pathRanges = hit.matchedField === 'path' ? hit.matchRanges : []
  return (
    <div
      role="option"
      tabIndex={undefined}
      aria-label={`${hit.name} ${hit.relativePath}`}
      aria-selected="false"
    >
      <div className="search-result-title">
        <span className="search-result-file-kind" aria-hidden="true">
          {fileKindLabel(hit.kind)}
        </span>
        <strong>
          <HighlightedText value={hit.name} ranges={nameRanges} />
        </strong>
        <span>{markerLabel(hit)}</span>
      </div>
      <div className="search-result-context">
        <span className="search-result-context">{matchContextLabel(hit)}</span>
        <span className="search-result-path">
          <HighlightedText value={hit.relativePath} ranges={pathRanges} />
        </span>
      </div>
      {hit.matchedField === 'body' && snippet !== undefined && snippet !== null && (
        <p className="search-result-context" data-testid={`search-snippet-${hit.entityId}`}>
          {boundedSnippet(snippet)}
        </p>
      )}
    </div>
  )
}

function HighlightedText({ value, ranges }: { value: string; ranges: MatchRange[] }) {
  const characters = Array.from(value)
  const safeRanges = ranges
    .map((range) => ({
      start: Math.max(0, Math.min(characters.length, range.start)),
      end: Math.max(0, Math.min(characters.length, range.end)),
    }))
    .filter((range) => range.end > range.start)
    .sort((left, right) => left.start - right.start)
  if (safeRanges.length === 0) return value
  const segments = []
  let cursor = 0
  for (const range of safeRanges) {
    if (range.start > cursor) segments.push(characters.slice(cursor, range.start).join(''))
    segments.push(
      <mark key={`${range.start}:${range.end}`}>
        {characters.slice(range.start, range.end).join('')}
      </mark>,
    )
    cursor = Math.max(cursor, range.end)
  }
  if (cursor < characters.length) segments.push(characters.slice(cursor).join(''))
  return segments
}

function resultRows(hits: SearchHit[], layout: SearchQueryModel['layout']): ResultRow[] {
  const rows: ResultRow[] = []
  let currentGroup: string | null | undefined
  for (let index = 0; index < hits.length; index += 1) {
    const hit = hits[index]
    if (hit === undefined) continue
    if (layout === 'grouped' && hit.groupRelativePath !== currentGroup) {
      currentGroup = hit.groupRelativePath
      const path = currentGroup ?? '项目根目录'
      let count = 1
      while (hits[index + count]?.groupRelativePath === currentGroup) count += 1
      rows.push({ type: 'group', key: `group:${path}`, path, count, height: GROUP_ROW_HEIGHT })
    }
    rows.push({ type: 'hit', key: `hit:${hit.entityId}`, hit, height: RESULT_ROW_HEIGHT })
  }
  return rows
}

function rowOffsetsFor(rows: ResultRow[]): number[] {
  let offset = 0
  return rows.map((row) => {
    const current = offset
    offset += row.height
    return current
  })
}

function markerLabel(hit: SearchHit): string {
  const review =
    hit.marker.reviewState === 'keep'
      ? '保留'
      : hit.marker.reviewState === 'pending'
        ? '待定'
        : hit.marker.reviewState === 'reject'
          ? '淘汰'
          : '未标记'
  return hit.marker.favorite ? `${review} · 收藏` : review
}

function boundedSnippet(value: string): string {
  return Array.from(value).slice(0, 160).join('')
}

function matchContextLabel(hit: SearchHit): string {
  if (hit.matchedField === 'body') return '文件内容匹配'
  if (hit.matchedField === 'path') return '路径匹配'
  return '文件名与路径匹配'
}

function fileKindLabel(kind: SearchHit['kind']): string {
  const labels: Record<SearchHit['kind'], string> = {
    jpeg: 'JPG',
    png: 'PNG',
    unsupported_image: 'IMG',
    markdown: 'MD',
    text: 'TXT',
    other: '文件',
    directory: '文件夹',
  }
  return labels[kind]
}

function activeFilterCount(query: SearchQueryModel): number {
  const filters = query.filters
  return (
    filters.kinds.length +
    filters.reviewStates.length +
    filters.orientations.length +
    Number(filters.favoriteOnly) +
    Number(filters.unmarkedOnly) +
    Number(filters.widthMin !== null || filters.widthMax !== null) +
    Number(filters.heightMin !== null || filters.heightMax !== null) +
    Number(filters.sizeMin !== null || filters.sizeMax !== null) +
    Number(filters.modifiedNsMin !== null || filters.modifiedNsMax !== null)
  )
}
