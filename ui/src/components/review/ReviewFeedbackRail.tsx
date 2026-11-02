import type { RefObject } from 'react'
import { useEffect, useRef, useState } from 'react'
import { reviewAnchorLabel } from '../../app/review/reviewAnchorLabel'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import ViewerButton, { ViewerIconButton } from '../ui/ViewerButton'
import ViewerIcon from '../ui/ViewerIcon'
import FeedbackTextInput from './FeedbackTextInput'
import InlineFeedbackEditor from './InlineFeedbackEditor'

interface ReviewFeedbackRailProps {
  controller: ImageReviewWorkbenchController
}

export default function ReviewFeedbackRail({ controller }: ReviewFeedbackRailProps) {
  const input = useRef<HTMLTextAreaElement>(null)
  const [operationError, setOperationError] = useState<string | null>(null)
  const readOnly = controller.readOnlyReason !== null
  const statusMessage = controller.statusMessage ?? readOnlyMessage(controller.readOnlyReason)
  const editor = controller.editor
  const phase = editor.phase
  const editingText = phase.status === 'idle' || phase.status === 'drawing' ? '' : phase.text
  const editingItemId =
    phase.status === 'idle' || phase.status === 'drawing' || phase.operation === 'geometry'
      ? null
      : phase.sourceItemId
  const saving = phase.status === 'saving'
  useEffect(() => {
    if (editingItemId !== null && !saving) input.current?.focus()
  }, [editingItemId, saving])

  async function runMutation(operation: () => Promise<void>) {
    setOperationError(null)
    try {
      await operation()
    } catch {
      setOperationError('操作尚未保存，请重试。')
    }
  }

  if (!controller.railOpen) {
    return (
      <aside className="review-feedback-rail review-feedback-rail--collapsed" aria-label="评审意见">
        <ViewerIconButton
          icon="panel-right"
          label="展开意见栏"
          tone="quiet"
          onClick={() => controller.setRailOpen(true)}
        />
      </aside>
    )
  }

  return (
    <aside className="review-feedback-rail" aria-label="评审意见">
      <header>
        <div className="review-feedback-rail__heading">
          <ViewerIcon name="message-square" />
          <strong>评审意见</strong>
          <span className="review-feedback-rail__count">{controller.feedback.length}</span>
        </div>
        <ViewerIconButton
          icon="panel-right"
          label="收起意见栏"
          tone="quiet"
          onClick={() => controller.setRailOpen(false)}
        />
      </header>
      <RailActions controller={controller} readOnly={readOnly} runMutation={runMutation} />
      <RailNotices
        controller={controller}
        operationError={operationError}
        statusMessage={statusMessage}
      />
      <AssetFeedbackEditor controller={controller} />
      <FeedbackList
        controller={controller}
        input={input}
        editingItemId={editingItemId}
        editingText={editingText}
        saving={saving}
        readOnly={readOnly}
        statusMessage={statusMessage}
        clearError={() => setOperationError(null)}
        runMutation={runMutation}
      />
    </aside>
  )
}

function RailActions({
  controller,
  readOnly,
  runMutation,
}: {
  controller: ImageReviewWorkbenchController
  readOnly: boolean
  runMutation(operation: () => Promise<void>): Promise<void>
}) {
  return (
    <div className="review-feedback-rail__actions">
      <ViewerButton
        leadingIcon="plus"
        disabled={readOnly || controller.dirty}
        onClick={() => controller.beginAnnotation({ kind: 'asset' })}
      >
        整图意见
      </ViewerButton>
      {controller.restorableItemId !== null && (
        <ViewerButton
          tone="quiet"
          onClick={() => {
            const itemId = controller.restorableItemId
            if (itemId !== null) void runMutation(() => controller.restoreDeletedFeedback(itemId))
          }}
        >
          撤销删除
        </ViewerButton>
      )}
    </div>
  )
}

function RailNotices({
  controller,
  operationError,
  statusMessage,
}: {
  controller: ImageReviewWorkbenchController
  operationError: string | null
  statusMessage: string | null
}) {
  const phase = controller.editor.phase
  return (
    <>
      {operationError !== null && (
        <p className="review-feedback-rail__error" role="status" aria-live="polite">
          {operationError}
        </p>
      )}
      {operationError === null && statusMessage !== null && (
        <p className="review-feedback-rail__status" role="status" aria-live="polite">
          {statusMessage}
        </p>
      )}
      {phase.status === 'save_error' && phase.sourceItemId !== null && (
        <div className="review-feedback-rail__error" role="status" aria-live="polite">
          {phase.message}
          {phase.operation === 'geometry' && (
            <>
              <ViewerButton aria-label="重试保存标记" onClick={() => void controller.saveDraft()}>
                重试
              </ViewerButton>
              <ViewerButton aria-label="取消标记修改" tone="quiet" onClick={controller.cancelDraft}>
                取消
              </ViewerButton>
            </>
          )}
        </div>
      )}
    </>
  )
}

function AssetFeedbackEditor({ controller }: ReviewFeedbackRailProps) {
  const phase = controller.editor.phase
  if (
    phase.status === 'idle' ||
    phase.status === 'drawing' ||
    phase.sourceItemId !== null ||
    phase.draftAnchor.kind !== 'asset'
  )
    return null
  return <InlineFeedbackEditor controller={controller} anchor={phase.draftAnchor} embedded />
}

function RailTextEditor({
  controller,
  input,
  label,
  editingText,
  saving,
  clearError,
}: {
  controller: ImageReviewWorkbenchController
  input: RefObject<HTMLTextAreaElement | null>
  label: string
  editingText: string
  saving: boolean
  clearError(): void
}) {
  return (
    <div className="review-feedback-rail__text-editor">
      <FeedbackTextInput
        value={editingText}
        onChange={controller.updateDraftText}
        onSave={() => void controller.saveDraft()}
        ariaLabel={`意见 ${label} 文字`}
        disabled={saving}
        rows={3}
        inputRef={input}
        className="review-feedback-rail__text-input"
        micClassName="review-feedback-rail__mic"
      />
      <div className="review-feedback-rail__text-actions">
        <ViewerButton
          aria-label={`保存意见 ${label} 文字`}
          loading={saving}
          disabled={editingText.trim().length === 0}
          onClick={() => void controller.saveDraft()}
        >
          保存
        </ViewerButton>
        <ViewerButton
          tone="quiet"
          disabled={saving}
          onClick={() => {
            controller.cancelDraft()
            clearError()
          }}
        >
          取消
        </ViewerButton>
      </div>
    </div>
  )
}

function FeedbackList({
  controller,
  input,
  editingItemId,
  editingText,
  saving,
  readOnly,
  statusMessage,
  clearError,
  runMutation,
}: {
  controller: ImageReviewWorkbenchController
  input: RefObject<HTMLTextAreaElement | null>
  editingItemId: string | null
  editingText: string
  saving: boolean
  readOnly: boolean
  statusMessage: string | null
  clearError(): void
  runMutation(operation: () => Promise<void>): Promise<void>
}) {
  const list = useRef<HTMLOListElement>(null)
  useEffect(() => {
    const selected = list.current?.querySelector<HTMLElement>('li[data-selected="true"]')
    // Non-browser DOM hosts (jsdom, older WebKit) may not implement
    // scrollIntoView; scrolling is a convenience, never a requirement.
    selected?.scrollIntoView?.({ block: 'nearest' })
  }, [controller.feedback.length, controller.selectedItemId])

  return (
    <>
      <ol ref={list} className="review-feedback-rail__list" aria-label="本图意见">
        {controller.feedback.map((feedback) => {
          const label = feedback.ordinal === null ? '整图' : String(feedback.ordinal)
          const editing = editingItemId === feedback.itemId
          return (
            <li
              key={feedback.itemId}
              aria-label={`意见 ${label}`}
              data-selected={controller.selectedItemId === feedback.itemId || undefined}
              data-whole-image={feedback.ordinal === null || undefined}
            >
              <button
                type="button"
                className="review-feedback-rail__selection"
                aria-label={`选择意见 ${label}：${feedback.text}`}
                onClick={() => controller.selectFeedback(feedback.itemId)}
              >
                <span className="review-feedback-rail__ordinal">
                  {feedback.ordinal === null ? '全' : feedback.ordinal}
                </span>
                <span className="review-feedback-rail__summary">
                  <strong>{feedback.text}</strong>
                  <span className="review-feedback-rail__meta">
                    <em>{reviewAnchorLabel(feedback.anchor.kind)}</em>
                    <time dateTime={new Date(feedback.createdAtMs).toISOString()}>
                      {formatFeedbackTime(feedback.createdAtMs)}
                    </time>
                  </span>
                </span>
              </button>
              {editing ? (
                <RailTextEditor
                  controller={controller}
                  input={input}
                  label={label}
                  editingText={editingText}
                  saving={saving}
                  clearError={clearError}
                />
              ) : (
                <div className="review-feedback-rail__item-actions">
                  <ViewerButton
                    tone="quiet"
                    aria-label={`编辑意见 ${label} 文字`}
                    disabled={readOnly || controller.dirty}
                    onClick={() => {
                      controller.beginFeedbackTextEdit(feedback.itemId)
                      clearError()
                    }}
                  >
                    编辑文字
                  </ViewerButton>
                  {canAdjustAnchor(feedback.anchor.kind) && (
                    <ViewerButton
                      tone="quiet"
                      aria-label={`调整意见 ${label} 标注`}
                      disabled={readOnly || controller.dirty}
                      onClick={() => {
                        controller.selectFeedback(feedback.itemId)
                        controller.setTool('browse')
                      }}
                    >
                      调整标注
                    </ViewerButton>
                  )}
                  {feedback.anchor.kind === 'image_stroke' && (
                    <ViewerButton
                      tone="quiet"
                      aria-label={`重绘意见 ${label}`}
                      disabled={readOnly || controller.dirty}
                      onClick={() => controller.beginRedraw(feedback.itemId)}
                    >
                      重绘
                    </ViewerButton>
                  )}
                  <ViewerButton
                    tone="quiet"
                    aria-label={`删除意见 ${label}`}
                    disabled={readOnly || controller.dirty}
                    onClick={() =>
                      void runMutation(() => controller.deleteFeedback(feedback.itemId))
                    }
                  >
                    删除
                  </ViewerButton>
                </div>
              )}
            </li>
          )
        })}
      </ol>
      <FeedbackNavigator controller={controller} statusMessage={statusMessage} />
    </>
  )
}

function FeedbackNavigator({
  controller,
  statusMessage,
}: {
  controller: ImageReviewWorkbenchController
  statusMessage: string | null
}) {
  const total = controller.feedback.length
  const selectedIndex = controller.feedback.findIndex(
    (feedback) => feedback.itemId === controller.selectedItemId,
  )
  const previous =
    total < 2
      ? undefined
      : selectedIndex <= 0
        ? controller.feedback.at(-1)
        : controller.feedback[selectedIndex - 1]
  const next =
    total < 2
      ? undefined
      : selectedIndex === -1 || selectedIndex === total - 1
        ? controller.feedback[0]
        : controller.feedback[selectedIndex + 1]
  const detail =
    statusMessage ??
    (controller.readOnlyReason !== null
      ? '只读 · 不可修改'
      : controller.dirty
        ? '未保存'
        : '已保存，可供外部读取')
  return (
    <footer className="review-feedback-rail__nav">
      <span className="review-feedback-rail__nav-summary">
        {total} 条意见 · {detail}
      </span>
      <div className="review-feedback-rail__nav-actions">
        <ViewerIconButton
          icon="chevron-up"
          label="上一条意见"
          tone="quiet"
          disabled={previous === undefined}
          onClick={() => {
            if (previous !== undefined) controller.selectFeedback(previous.itemId)
          }}
        />
        <ViewerIconButton
          icon="chevron-down"
          label="下一条意见"
          tone="quiet"
          disabled={next === undefined}
          onClick={() => {
            if (next !== undefined) controller.selectFeedback(next.itemId)
          }}
        />
      </div>
    </footer>
  )
}

function canAdjustAnchor(
  kind: ImageReviewWorkbenchController['feedback'][number]['anchor']['kind'],
) {
  return (
    kind === 'image_rect' ||
    kind === 'image_ellipse' ||
    kind === 'image_arrow' ||
    kind === 'image_line' ||
    kind === 'image_polygon'
  )
}

function formatFeedbackTime(createdAtMs: number) {
  const date = new Date(createdAtMs)
  if (Number.isNaN(date.getTime())) return null
  return new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit' }).format(date)
}

function readOnlyMessage(reason: ImageReviewWorkbenchController['readOnlyReason']) {
  switch (reason) {
    case null:
      return null
    case 'loading':
      return '正在准备当前素材版本…'
    case 'saving':
      return '正在保存意见，请稍候。'
    case 'source_confirmation':
      return '素材来源或版本需要确认，刷新确认后可继续评审。'
    case 'migration_required':
      return '需要先确认旧评审数据迁移。'
    case 'recovery_required':
      return '存在待恢复输入，需要先确认处理。'
    case 'outside_scope':
      return '当前素材不在旧评审范围内。'
    case 'write_unavailable':
      return '当前项目不可写。'
  }
}
