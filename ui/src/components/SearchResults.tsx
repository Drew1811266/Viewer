import { useEffect, useMemo, useState } from 'react'
import type { UIEvent } from 'react'
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
}

type ResultRow =
  | { type: 'group'; key: string; path: string }
  | { type: 'hit'; key: string; hit: SearchHit }

const ROW_HEIGHT = 76
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
}: SearchResultsProps) {
  const [scrollTop, setScrollTop] = useState(0)
  const rows = useMemo(() => resultRows(page.hits, query.layout), [page.hits, query.layout])
  const firstVisible = Math.floor(scrollTop / ROW_HEIGHT)
  const visibleCount = Math.ceil(VIEWPORT_HEIGHT / ROW_HEIGHT)
  const start = Math.max(0, firstVisible - OVERSCAN)
  const end = Math.min(rows.length, firstVisible + visibleCount + OVERSCAN)
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
          {query.scopeFolderId === null ? '整个项目' : '当前目录及其后代'}，
          {filterCount} 个筛选条件。
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
      <header className="search-results-heading">
        <div>
          <strong>{page.total} 个结果</strong>
          {!page.progress.complete && <span role="status">结果仍在更新</span>}
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
        onScroll={(event: UIEvent<HTMLDivElement>) =>
          setScrollTop(event.currentTarget.scrollTop)
        }
      >
        <div style={{ height: rows.length * ROW_HEIGHT, position: 'relative' }}>
          {visibleRows.map((row, visibleIndex) => {
            const index = start + visibleIndex
            return (
              <div
                key={row.key}
                className={`search-result-row search-result-${row.type}`}
                style={{
                  height: ROW_HEIGHT,
                  left: 0,
                  position: 'absolute',
                  right: 0,
                  top: index * ROW_HEIGHT,
                }}
              >
                {row.type === 'group' ? (
                  <h3>{row.path}</h3>
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
    hit.matchedField === 'filename' || hit.matchedField === 'exact_filename'
      ? hit.matchRanges
      : []
  const pathRanges = hit.matchedField === 'path' ? hit.matchRanges : []
  return (
    <div role="option" aria-label={`${hit.name} ${hit.relativePath}`} aria-selected="false">
      <div className="search-result-title">
        <HighlightedText value={hit.name} ranges={nameRanges} />
        <span>{markerLabel(hit)}</span>
      </div>
      <div className="search-result-path">
        <HighlightedText value={hit.relativePath} ranges={pathRanges} />
      </div>
      {hit.matchedField === 'body' && snippet !== undefined && snippet !== null && (
        <p data-testid={`search-snippet-${hit.entityId}`}>{boundedSnippet(snippet)}</p>
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
    segments.push(<mark key={`${range.start}:${range.end}`}>{characters.slice(range.start, range.end).join('')}</mark>)
    cursor = Math.max(cursor, range.end)
  }
  if (cursor < characters.length) segments.push(characters.slice(cursor).join(''))
  return segments
}

function resultRows(hits: SearchHit[], layout: SearchQueryModel['layout']): ResultRow[] {
  const rows: ResultRow[] = []
  let currentGroup: string | null | undefined
  for (const hit of hits) {
    if (layout === 'grouped' && hit.groupRelativePath !== currentGroup) {
      currentGroup = hit.groupRelativePath
      const path = currentGroup ?? '项目根目录'
      rows.push({ type: 'group', key: `group:${path}`, path })
    }
    rows.push({ type: 'hit', key: `hit:${hit.entityId}`, hit })
  }
  return rows
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
