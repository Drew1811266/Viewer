import { useState } from 'react'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import ViewerButton, { ViewerIconButton } from '../ui/ViewerButton'
import InlineFeedbackEditor from './InlineFeedbackEditor'

interface ReviewFeedbackRailProps {
  controller: ImageReviewWorkbenchController
}

export default function ReviewFeedbackRail({ controller }: ReviewFeedbackRailProps) {
  const [editingFeedbackId, setEditingFeedbackId] = useState<string | null>(null)
  const [editingText, setEditingText] = useState('')
  const [savingFeedbackId, setSavingFeedbackId] = useState<string | null>(null)
  const [operationError, setOperationError] = useState<string | null>(null)
  const readOnly = controller.readOnlyReason !== null

  async function saveFeedbackText(feedbackId: string) {
    setSavingFeedbackId(feedbackId)
    setOperationError(null)
    try {
      await controller.updateFeedbackText(feedbackId, editingText)
      setEditingFeedbackId(null)
    } catch {
      setOperationError('意见尚未保存，请重试。')
    } finally {
      setSavingFeedbackId(null)
    }
  }

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

  const assetEditorAnchor =
    controller.editor.status !== 'idle' &&
    controller.editor.status !== 'drawing' &&
    controller.editor.draftAnchor.kind === 'asset'
      ? controller.editor.draftAnchor
      : null

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
      <div className="review-feedback-rail__actions">
        <ViewerButton
          leadingIcon="plus"
          disabled={readOnly}
          onClick={() => controller.beginAnnotation({ kind: 'asset' })}
        >
          整图意见
        </ViewerButton>
        {controller.restorableFeedbackId !== null && (
          <ViewerButton
            tone="quiet"
            onClick={() => {
              const feedbackId = controller.restorableFeedbackId
              if (feedbackId !== null)
                void runMutation(() => controller.restoreDeletedFeedback(feedbackId))
            }}
          >
            撤销删除
          </ViewerButton>
        )}
      </div>
      {operationError !== null && (
        <p className="review-feedback-rail__error" role="status" aria-live="polite">
          {operationError}
        </p>
      )}
      {assetEditorAnchor !== null && (
        <InlineFeedbackEditor controller={controller} anchor={assetEditorAnchor} embedded />
      )}
      <ol className="review-feedback-rail__list" aria-label="本图意见">
        {controller.feedback.map((feedback) => {
          const label = feedback.ordinal === null ? '整图' : String(feedback.ordinal)
          const editing = editingFeedbackId === feedback.feedbackId
          return (
            <li
              key={feedback.feedbackId}
              aria-label={`意见 ${label}`}
              data-selected={controller.selectedFeedbackId === feedback.feedbackId || undefined}
            >
              <button
                type="button"
                className="review-feedback-rail__selection"
                aria-label={`选择意见 ${label}：${feedback.text}`}
                onClick={() => controller.selectFeedback(feedback.feedbackId)}
              >
                <span>{feedback.ordinal === null ? '整图' : feedback.ordinal}</span>
                <strong>{feedback.text}</strong>
              </button>
              {editing ? (
                <div className="review-feedback-rail__text-editor">
                  <textarea
                    aria-label={`意见 ${label} 文字`}
                    value={editingText}
                    disabled={savingFeedbackId === feedback.feedbackId}
                    onChange={(event) => setEditingText(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
                        event.preventDefault()
                        if (editingText.trim().length > 0)
                          void saveFeedbackText(feedback.feedbackId)
                      }
                    }}
                  />
                  <ViewerButton
                    aria-label={`保存意见 ${label} 文字`}
                    loading={savingFeedbackId === feedback.feedbackId}
                    disabled={editingText.trim().length === 0}
                    onClick={() => void saveFeedbackText(feedback.feedbackId)}
                  >
                    保存
                  </ViewerButton>
                  <ViewerButton
                    tone="quiet"
                    disabled={savingFeedbackId === feedback.feedbackId}
                    onClick={() => {
                      setEditingFeedbackId(null)
                      setOperationError(null)
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
                    disabled={readOnly}
                    onClick={() => {
                      setEditingFeedbackId(feedback.feedbackId)
                      setEditingText(feedback.text)
                      setOperationError(null)
                    }}
                  >
                    编辑文字
                  </ViewerButton>
                  {feedback.anchor.kind === 'image_rect' && (
                    <ViewerButton
                      tone="quiet"
                      aria-label={`调整意见 ${label} 区域`}
                      disabled={readOnly}
                      onClick={() => {
                        controller.selectFeedback(feedback.feedbackId)
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
                      disabled={readOnly}
                      onClick={() => {
                        controller.selectFeedback(feedback.feedbackId)
                        controller.setTool('brush')
                      }}
                    >
                      重绘
                    </ViewerButton>
                  )}
                  <ViewerButton
                    tone="quiet"
                    aria-label={`删除意见 ${label}`}
                    disabled={readOnly}
                    onClick={() =>
                      void runMutation(() => controller.deleteFeedback(feedback.feedbackId))
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
    </aside>
  )
}
