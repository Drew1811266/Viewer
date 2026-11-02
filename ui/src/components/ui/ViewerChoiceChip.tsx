import type { InputHTMLAttributes, ReactNode } from 'react'
import ViewerIcon from './ViewerIcon'

export interface ViewerChoiceChipProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'children' | 'onChange' | 'type'> {
  children: ReactNode
  type?: 'checkbox' | 'radio'
  onCheckedChange?: (checked: boolean) => void
}

export default function ViewerChoiceChip({
  children,
  checked,
  defaultChecked,
  disabled,
  type = 'checkbox',
  className,
  onCheckedChange,
  ...inputProps
}: ViewerChoiceChipProps) {
  return (
    <label
      className={className === undefined ? 'viewer-choice-chip' : `viewer-choice-chip ${className}`}
      data-checked={checked || undefined}
      data-disabled={disabled || undefined}
    >
      <input
        {...inputProps}
        type={type}
        checked={checked}
        defaultChecked={defaultChecked}
        disabled={disabled}
        onChange={(event) => onCheckedChange?.(event.currentTarget.checked)}
      />
      {checked && <ViewerIcon name="check" />}
      <span>{children}</span>
    </label>
  )
}
