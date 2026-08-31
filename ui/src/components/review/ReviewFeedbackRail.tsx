import type { RefObject } from 'react'
import { useEffect, useRef, useState } from 'react'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import ViewerButton, { ViewerIconButton } from '../ui/ViewerButton'
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
  const editingText = editor.status === 'idle' || editor.status === 'drawing' ? '' : editor.text
  const editingItemId =
    editor.status === 'idle' || editor.status === 'drawing' || editor.operation === 'geometry'
      ? null
      : editor.sourceItemId
  const saving = editor.status === 'saving'
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
        <div>
          <strong>评审意见</strong>
          <span>{controller.feedback.length} 条</span>
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
  const editor = controller.editor
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
      {editor.status === 'save_error' && editor.sourceItemId !== null && (
        <div className="review-feedback-rail__error" role="status" aria-live="polite">
          {editor.message}
          {editor.operation === 'geometry' && (
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
  const editor = controller.editor
  if (
    editor.status === 'idle' ||
    editor.status === 'drawing' ||
    editor.sourceItemId !== null ||
    editor.draftAnchor.kind !== 'asset'
  )
    return null
  return <InlineFeedbackEditor controller={controller} anchor={editor.draftAnchor} embedded />
}

function FeedbackList({
  controller,
  input,
  editingItemId,
  editingText,
  saving,
  readOnly,
  clearError,
  runMutation,
}: {
  controller: ImageReviewWorkbenchController
  input: RefObject<HTMLTextAreaElement | null>
  editingItemId: string | null
  editingText: string
  saving: boolean
  readOnly: boolean
  clearError(): void
  runMutation(operation: () => Promise<void>): Promise<void>
}) {
  return (
    <ol className="review-feedback-rail__list" aria-label="本图意见">
      {controller.feedback.map((feedback) => {
        const label = feedback.ordinal === null ? '整图' : String(feedback.ordinal)
        const editing = editingItemId === feedback.itemId
        return (
          <li
            key={feedback.itemId}
            aria-label={`意见 ${label}`}
            data-selected={controller.selectedItemId === feedback.itemId || undefined}
          >
            <button
              type="button"
              className="review-feedback-rail__selection"
              aria-label={`选择意见 ${label}：${feedback.text}`}
              onClick={() => controller.selectFeedback(feedback.itemId)}
            >
              <span>{feedback.ordinal === null ? '整图' : feedback.ordinal}</span>
              <strong>{feedback.text}</strong>
            </button>
            {editing ? (
              <div className="review-feedback-rail__text-editor">
                <textarea
                  ref={input}
                  aria-label={`意见 ${label} 文字`}
                  value={editingText}
                  disabled={saving}
                  onChange={(event) => controller.updateDraftText(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.nativeEvent.isComposing) return
                    if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
                      event.preventDefault()
                      if (editingText.trim().length > 0) void controller.saveDraft()
                    }
                  }}
                />
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
                {feedback.anchor.kind === 'image_rect' && (
                  <ViewerButton
                    tone="quiet"
                    aria-label={`调整意见 ${label} 区域`}
                    disabled={readOnly || controller.dirty}
                    onClick={() => {
                      controller.selectFeedback(feedback.itemId)
                      controller.setTool('rectangle')
                    }}
                  >
                    调整区域
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
                  onClick={() => void runMutation(() => controller.deleteFeedback(feedback.itemId))}
                >
                  删除
                </ViewerButton>
              </div>
            )}
          </li>
        )
      })}
    </ol>
  )
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
