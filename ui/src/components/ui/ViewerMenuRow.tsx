import type { ButtonHTMLAttributes, ReactNode } from 'react'
import ViewerIcon, { type ViewerIconName } from './ViewerIcon'

export type ViewerMenuRowTone = 'neutral' | 'danger'

export interface ViewerMenuRowProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children' | 'onClick'> {
  children: ReactNode
  current?: boolean
  disabledReason?: ReactNode
  shortcut?: string
  tone?: ViewerMenuRowTone
  icon?: ViewerIconName
  onSelect?(): void
}

export default function ViewerMenuRow({
  children,
  current = false,
  disabledReason,
  shortcut,
  tone = 'neutral',
  icon,
  onSelect,
  disabled,
  className,
  ...buttonProps
}: ViewerMenuRowProps) {
  const unavailable = disabled || disabledReason !== undefined

  return (
    <button
      {...buttonProps}
      type={buttonProps.type ?? 'button'}
      className={className === undefined ? 'viewer-menu-row' : `viewer-menu-row ${className}`}
      data-tone={tone}
      data-current={current || undefined}
      aria-pressed={current || undefined}
      disabled={unavailable}
      onClick={onSelect}
    >
      {icon === undefined ? null : <ViewerIcon name={icon} />}
      <span className="viewer-menu-row__content">
        <span className="viewer-menu-row__label">{children}</span>
        {disabledReason === undefined ? null : (
          <span className="viewer-menu-row__reason">{disabledReason}</span>
        )}
      </span>
      {current && <span className="viewer-menu-row__current">当前</span>}
      {shortcut === undefined ? null : <kbd>{shortcut}</kbd>}
    </button>
  )
}
