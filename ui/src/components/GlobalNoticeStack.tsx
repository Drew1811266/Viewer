import { useEffect, useState } from 'react'
import ViewerButton, { ViewerIconButton } from './ui/ViewerButton'

export type GlobalNoticeTone = 'info' | 'warning' | 'danger'

export interface GlobalNotice {
  id: string
  title: string
  message: string
  tone: GlobalNoticeTone
  action?: {
    label: string
    onAction(): void
  }
}

export interface GlobalNoticeStackProps {
  notices: GlobalNotice[]
  belowReadOnly?: boolean
}

export default function GlobalNoticeStack({
  notices,
  belowReadOnly = false,
}: GlobalNoticeStackProps) {
  const [dismissed, setDismissed] = useState<Set<string>>(() => new Set())

  useEffect(() => {
    const activeIds = new Set(notices.map((notice) => notice.id))
    setDismissed((current) => {
      const next = new Set([...current].filter((id) => activeIds.has(id)))
      return next.size === current.size ? current : next
    })
  }, [notices])

  const visible = notices.filter((notice) => !dismissed.has(notice.id))
  if (visible.length === 0) return null

  return (
    <section
      className={
        belowReadOnly
          ? 'global-notice-stack global-notice-stack--below-read-only'
          : 'global-notice-stack'
      }
      aria-label="全局通知"
    >
      {visible.map((notice) => (
        <article
          className="global-notice"
          data-tone={notice.tone}
          key={notice.id}
          role={notice.tone === 'danger' ? 'alert' : 'status'}
        >
          <div>
            <strong>{notice.title}</strong>
            <p>{notice.message}</p>
          </div>
          {notice.action && (
            <ViewerButton onClick={notice.action.onAction}>{notice.action.label}</ViewerButton>
          )}
          <ViewerIconButton
            icon="x"
            label={`关闭 ${notice.title}`}
            tone="quiet"
            onClick={() => setDismissed((current) => new Set([...current, notice.id]))}
          />
        </article>
      ))}
    </section>
  )
}
