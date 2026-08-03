import { type ButtonHTMLAttributes, forwardRef } from 'react'
import ViewerIcon, { type ViewerIconName } from './ViewerIcon'

export type ViewerButtonTone = 'primary' | 'secondary' | 'quiet' | 'danger'

export interface ViewerButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  tone?: ViewerButtonTone
  active?: boolean
  loading?: boolean
  leadingIcon?: ViewerIconName
}

const ViewerButton = forwardRef<HTMLButtonElement, ViewerButtonProps>(function ViewerButton(
  {
    tone = 'secondary',
    active,
    loading = false,
    leadingIcon,
    disabled,
    className,
    children,
    ...buttonProps
  },
  ref,
) {
  return (
    <button
      {...buttonProps}
      ref={ref}
      type={buttonProps.type ?? 'button'}
      className={className === undefined ? 'viewer-button' : `viewer-button ${className}`}
      data-tone={tone}
      data-active={active || undefined}
      aria-pressed={active === undefined ? undefined : active}
      aria-busy={loading || undefined}
      disabled={disabled || loading}
    >
      {loading ? (
        <ViewerIcon name="refresh-cw" className="viewer-button__loading-icon" />
      ) : leadingIcon === undefined ? null : (
        <ViewerIcon name={leadingIcon} />
      )}
      <span className="viewer-button__label">{children}</span>
    </button>
  )
})

export interface ViewerIconButtonProps
  extends Omit<ViewerButtonProps, 'aria-label' | 'children' | 'leadingIcon'> {
  icon: ViewerIconName
  label: string
}

export const ViewerIconButton = forwardRef<HTMLButtonElement, ViewerIconButtonProps>(
  function ViewerIconButton({ icon, label, className, ...buttonProps }, ref) {
    return (
      <ViewerButton
        {...buttonProps}
        ref={ref}
        className={
          className === undefined ? 'viewer-icon-button' : `viewer-icon-button ${className}`
        }
        aria-label={label}
        title={buttonProps.title ?? label}
      >
        <ViewerIcon name={icon} />
      </ViewerButton>
    )
  },
)

export default ViewerButton
