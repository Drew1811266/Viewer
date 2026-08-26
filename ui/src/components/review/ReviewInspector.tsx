import { useEffect, useRef } from 'react'
import { eligibleReviewTargetIds } from '../../app/review/reviewModel'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import ViewerButton, { ViewerIconButton } from '../ui/ViewerButton'

interface ReviewInspectorProps {
  review: ReviewSessionCoordinator
  selectedEntityIds: string[]
  readOnly: boolean
  onClose(): void
}

export default function ReviewInspector({
  review,
  selectedEntityIds,
  readOnly,
  onClose,
}: ReviewInspectorProps) {
  const eligibleTargets = eligibleReviewTargetIds(selectedEntityIds, review.snapshot.members)
  const ineligibleCount = new Set(selectedEntityIds).size - eligibleTargets.length
  const editor = review.editor
  const selectionIdentity = selectedEntityIds.join('\u0000')
  const previousSelectionIdentity = useRef(selectionIdentity)

  useEffect(() => {
    if (
      previousSelectionIdentity.current !== selectionIdentity &&
      !readOnly &&
      editor.mode === 'create' &&
      editor.text.trim().length === 0
    ) {
      review.beginCreate()
    }
    previousSelectionIdentity.current = selectionIdentity
  }, [editor.mode, editor.text, readOnly, review.beginCreate, selectionIdentity])

  const submit = () => {
    if (editor.mode === 'edit') void review.saveEdit()
    else void review.submitFeedback()
  }

  return (
    <aside className="review-inspector" aria-label="评审意见面板">
      <header className="review-inspector__header">
        <div>
          <h2>评审意见</h2>
          <p>{readOnly ? '已完成记录' : '只记录需要返工的素材'}</p>
        </div>
        <ViewerIconButton
          icon="x"
          label="关闭评审意见面板"
          tone="quiet"
          onClick={() => review.requestDiscard('inspector_close', onClose)}
        />
      </header>

      {!readOnly && (
        <section className="review-editor" aria-labelledby="review-editor-title">
          <h3 id="review-editor-title">{editor.mode === 'edit' ? '编辑意见' : '添加返工意见'}</h3>
          <p>{editor.targetEntityIds.length} 个已选目标可添加意见</p>
          {ineligibleCount > 0 && <p>{ineligibleCount} 个所选项目不属于本轮素材</p>}
          <label htmlFor="review-feedback-text">返工意见</label>
          <textarea
            id="review-feedback-text"
            value={editor.text}
            disabled={editor.saveState === 'saving'}
            onChange={(event) => review.setEditorText(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && event.metaKey) {
                event.preventDefault()
                submit()
              }
            }}
            rows={5}
          />
          <div className="review-editor__actions">
            <span>⌘ Enter 提交</span>
            <ViewerButton
              tone="primary"
              loading={editor.saveState === 'saving'}
              disabled={editor.text.trim().length === 0 || editor.targetEntityIds.length === 0}
              onClick={submit}
            >
              {editor.mode === 'edit' ? '保存修改' : '添加意见'}
            </ViewerButton>
          </div>
          {editor.error !== null && (
            <p className="review-editor__error" role="status" aria-live="polite">
              {editor.error}
            </p>
          )}
        </section>
      )}

      <section className="review-feedback-list" aria-labelledby="review-saved-feedback">
        <h3 id="review-saved-feedback">已保存意见</h3>
        {review.snapshot.feedback.length === 0 ? (
          <p>还没有返工意见。</p>
        ) : (
          <ul>
            {review.snapshot.feedback.map((feedback) => (
              <li key={feedback.feedbackId}>
                <p>{feedback.text}</p>
                <small>{feedback.targetCount} 个目标</small>
                {!readOnly && (
                  <div>
                    <ViewerButton
                      tone="quiet"
                      aria-label={`编辑意见：${feedback.text}`}
                      onClick={() => review.beginEdit(feedback.feedbackId)}
                    >
                      编辑
                    </ViewerButton>
                    <ViewerButton
                      tone="quiet"
                      aria-label={`删除意见：${feedback.text}`}
                      onClick={() => void review.deleteFeedback(feedback.feedbackId)}
                    >
                      删除
                    </ViewerButton>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </aside>
  )
}
