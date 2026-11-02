import type { KeyboardEvent, MouseEvent } from 'react'
import { useEffect, useMemo, useState } from 'react'
import type { BrowserFile, MatchRange, SearchHit, SearchPage, SearchQueryModel } from '../api/types'
import ReviewFeedbackCountBadge from './review/ReviewFeedbackCountBadge'
import ViewerButton from './ui/ViewerButton'
import ViewerEmptyState from './ui/ViewerEmptyState'
import ViewerIcon from './ui/ViewerIcon'
import ViewerStatusTag, { type ViewerStatusTagTone } from './ui/ViewerStatusTag'

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
  selectedEntityIds?: string[]
  onSelectionChange?: (entityIds: string[]) => void
  feedbackCountByEntityId?: ReadonlyMap<string, number>
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onPreview?: (file: BrowserFile) => void
}

interface SearchGroup {
  path: string | null
  hits: SearchHit[]
}

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
  selectedEntityIds = [],
  onSelectionChange,
  feedbackCountByEntityId,
  requestThumbnail,
  onPreview,
}: SearchResultsProps) {
  const groups = useMemo(() => groupHits(page.hits, query.layout), [page.hits, query.layout])
  const selectedIds = useMemo(() => new Set(selectedEntityIds), [selectedEntityIds])
  const visibleIds = useMemo(() => page.hits.map((hit) => hit.entityId), [page.hits])
  useEffect(() => onVisibleHits(visibleIds), [onVisibleHits, visibleIds])

  if (page.total === 0) {
    const filterCount = activeFilterCount(query)
    return (
      <section className="search-results search-empty-results" aria-label="无搜索结果">
        <header className="search-results-summary">
          <div>
            <h1>搜索结果</h1>
            <p>0 个结果 · {query.layout === 'grouped' ? '按文件夹分组' : '展平结果'}</p>
          </div>
          <ViewerButton tone="quiet" onClick={onReturnToFolder}>
            返回文件夹内容
          </ViewerButton>
        </header>
        <div className="search-empty">
          <ViewerEmptyState
            appearance="plain"
            title="没有找到结果"
            description={`关键词“${query.text || '（空）'}”，范围：${
              query.scopeFolderId === null ? '整个项目' : '当前目录及其后代'
            }，${filterCount} 个筛选条件。`}
            action={
              <>
                {filterCount > 0 && (
                  <ViewerButton tone="primary" onClick={onClearFilters}>
                    清除筛选
                  </ViewerButton>
                )}
                {query.scopeFolderId !== null && (
                  <ViewerButton tone="secondary" onClick={onSearchProject}>
                    搜索整个项目
                  </ViewerButton>
                )}
                <ViewerButton tone="quiet" onClick={onReturnToFolder}>
                  返回文件夹内容
                </ViewerButton>
              </>
            }
          />
        </div>
      </section>
    )
  }

  const showsGroups = query.layout === 'grouped'

  return (
    <section className="search-results" aria-label="搜索结果区域">
      <header className="search-results-summary">
        <div>
          <h1>搜索结果</h1>
          <p>
            {page.total} 个结果 · {showsGroups ? '按文件夹分组' : '展平结果'}
          </p>
        </div>
        <ViewerButton tone="quiet" onClick={onReturnToFolder}>
          返回文件夹内容
        </ViewerButton>
      </header>
      {searching && (
        <div className="search-indexing-banner" role="status">
          结果仍在更新 · 图片 {page.progress.imagesReady}/{page.progress.imagesTotal} · 文本{' '}
          {page.progress.textReady}/{page.progress.textTotal}
        </div>
      )}
      <div className="search-result-groups" role="listbox" aria-label="搜索结果">
        {groups.map((group, groupIndex) => (
          <section
            className="search-result-card-group"
            key={group.path ?? `result-group-${groupIndex}`}
            aria-label={group.path ?? '搜索结果'}
          >
            {showsGroups && (
              <h2 className="search-result-group-heading">
                {group.path ?? '项目根目录'}
                <span>· {group.hits.length}</span>
              </h2>
            )}
            <div className="search-result-card-grid" role="group" aria-label={group.path ?? '结果'}>
              {group.hits.map((hit) => (
                <SearchResultCard
                  key={hit.entityId}
                  hit={hit}
                  snippet={snippets[hit.entityId]}
                  selected={selectedIds.has(hit.entityId)}
                  selectedEntityIds={selectedEntityIds}
                  onSelectionChange={onSelectionChange}
                  feedbackCount={feedbackCountByEntityId?.get(hit.entityId)}
                  requestThumbnail={requestThumbnail}
                  onPreview={onPreview}
                />
              ))}
            </div>
          </section>
        ))}
      </div>
      <nav className="search-pagination" aria-label="搜索结果分页">
        <span className="search-pagination-summary">
          第 {Math.floor(offset / limit) + 1} 页 · {offset + 1}–
          {Math.min(offset + page.hits.length, page.total)} / {page.total}
        </span>
        <div className="search-pagination-actions" role="group" aria-label="分页操作">
          <ViewerButton
            tone="quiet"
            disabled={offset === 0}
            onClick={() => onPageChange(Math.max(0, offset - limit))}
          >
            上一页
          </ViewerButton>
          <ViewerButton
            tone="quiet"
            disabled={offset + page.hits.length >= page.total}
            onClick={() => onPageChange(offset + limit)}
          >
            下一页
          </ViewerButton>
        </div>
      </nav>
    </section>
  )
}

function SearchResultCard({
  hit,
  snippet,
  selected,
  selectedEntityIds,
  onSelectionChange,
  feedbackCount,
  requestThumbnail,
  onPreview,
}: {
  hit: SearchHit
  snippet: string | null | undefined
  selected: boolean
  selectedEntityIds: string[]
  onSelectionChange?: (entityIds: string[]) => void
  feedbackCount?: number
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onPreview?: (file: BrowserFile) => void
}) {
  const nameRanges =
    hit.matchedField === 'filename' || hit.matchedField === 'exact_filename' ? hit.matchRanges : []
  const pathRanges = hit.matchedField === 'path' ? hit.matchRanges : []
  const marker = searchMarker(hit)
  const file = hitFile(hit)
  return (
    <div
      className="search-result-card"
      role="option"
      tabIndex={onSelectionChange === undefined ? undefined : 0}
      aria-label={`${hit.name} ${hit.relativePath}`}
      aria-selected={selected}
      onClick={(event) =>
        selectSearchResult(event, hit.entityId, selectedEntityIds, onSelectionChange)
      }
      onDoubleClick={() => {
        if (onPreview !== undefined) onPreview(file)
      }}
      onKeyDown={(event) => {
        if (event.key !== 'Enter' && event.key !== ' ') return
        event.preventDefault()
        selectSearchResult(event, hit.entityId, selectedEntityIds, onSelectionChange)
      }}
    >
      <div className="search-result-card-cover">
        {isImageKind(hit.kind) && requestThumbnail !== undefined ? (
          <SearchCover file={file} requestThumbnail={requestThumbnail} />
        ) : (
          <span className="search-result-card-placeholder" aria-hidden="true">
            <ViewerStatusTag tone="neutral">{fileKindLabel(hit.kind)}</ViewerStatusTag>
          </span>
        )}
        {marker.label !== '未标记' && (
          <ViewerStatusTag tone={marker.tone} className="search-result-card-state">
            {marker.label}
          </ViewerStatusTag>
        )}
        {selected && (
          <span className="search-result-card-check" aria-hidden="true">
            <ViewerIcon name="check" />
          </span>
        )}
      </div>
      <div className="search-result-card-meta">
        <span className="search-result-card-title">
          <strong>
            <HighlightedText value={hit.name} ranges={nameRanges} />
          </strong>
          <ReviewFeedbackCountBadge count={feedbackCount} />
        </span>
        <span className="search-result-card-submeta">
          {fileKindLabel(hit.kind)} · {formatBytes(hit.size)}
        </span>
        {hit.matchedField === 'path' && (
          <span className="search-result-card-path">
            <HighlightedText value={hit.relativePath} ranges={pathRanges} />
          </span>
        )}
        {snippet !== undefined && snippet !== null && (
          <span className="visually-hidden" data-testid={`search-snippet-${hit.entityId}`}>
            {boundedSnippet(snippet)}
          </span>
        )}
      </div>
    </div>
  )
}

function SearchCover({
  file,
  requestThumbnail,
}: {
  file: BrowserFile
  requestThumbnail: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
}) {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    setUrl(null)
    void requestThumbnail(file, 512, 1).then(
      (next) => {
        if (active) setUrl(next)
      },
      () => {
        if (active) setUrl(null)
      },
    )
    return () => {
      active = false
    }
  }, [file, requestThumbnail])
  if (url === null) {
    return (
      <span className="search-result-card-placeholder" aria-hidden="true">
        <ViewerStatusTag tone="neutral">{fileKindLabel(file.kind)}</ViewerStatusTag>
      </span>
    )
  }
  return <img src={url} alt="" draggable={false} />
}

function selectSearchResult(
  event: Pick<MouseEvent | KeyboardEvent, 'metaKey' | 'ctrlKey'>,
  entityId: string,
  selectedEntityIds: string[],
  onSelectionChange: ((entityIds: string[]) => void) | undefined,
) {
  if (onSelectionChange === undefined) return
  if (!event.metaKey && !event.ctrlKey) {
    onSelectionChange([entityId])
    return
  }
  const selected = new Set(selectedEntityIds)
  if (selected.has(entityId)) selected.delete(entityId)
  else selected.add(entityId)
  onSelectionChange([...selected])
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

function groupHits(hits: SearchHit[], layout: SearchQueryModel['layout']): SearchGroup[] {
  if (layout === 'flat') return [{ path: null, hits }]
  const groups = new Map<string | null, SearchHit[]>()
  for (const hit of hits) {
    const current = groups.get(hit.groupRelativePath) ?? []
    current.push(hit)
    groups.set(hit.groupRelativePath, current)
  }
  return [...groups].map(([path, group]) => ({ path, hits: group }))
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

function markerTone(hit: SearchHit): ViewerStatusTagTone {
  if (hit.marker.reviewState === 'keep') return 'success'
  if (hit.marker.reviewState === 'pending') return 'warning'
  if (hit.marker.reviewState === 'reject') return 'danger'
  return 'neutral'
}

function searchMarker(hit: SearchHit): { label: string; tone: ViewerStatusTagTone } {
  return { label: markerLabel(hit), tone: markerTone(hit) }
}

function hitFile(hit: SearchHit): BrowserFile {
  return {
    entityId: hit.entityId,
    relativePath: hit.relativePath,
    name: hit.name,
    kind: hit.kind === 'directory' ? 'other' : hit.kind,
    size: hit.size,
    modifiedNs: hit.modifiedNs,
    marker: hit.marker,
    imageMetadata: hit.imageMetadata,
    imageUrl: null,
    videoMetadata: null,
  }
}

function isImageKind(kind: SearchHit['kind']): boolean {
  return kind === 'jpeg' || kind === 'png' || kind === 'image' || kind === 'unsupported_image'
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`
  if (bytes < 1_024 ** 2) return `${formatUnit(bytes / 1_024)} KiB`
  if (bytes < 1_024 ** 3) return `${formatUnit(bytes / 1_024 ** 2)} MiB`
  return `${formatUnit(bytes / 1_024 ** 3)} GiB`
}

function formatUnit(value: number): string {
  return value >= 10 ? value.toFixed(0) : value.toFixed(1).replace(/\.0$/, '')
}

function boundedSnippet(value: string): string {
  return Array.from(value).slice(0, 160).join('')
}

function fileKindLabel(kind: SearchHit['kind']): string {
  const labels: Record<SearchHit['kind'], string> = {
    jpeg: 'JPG',
    png: 'PNG',
    image: 'IMG',
    unsupported_image: 'IMG',
    video: '视频',
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
