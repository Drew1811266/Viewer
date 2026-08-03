import type { InputHTMLAttributes, ReactNode } from 'react'

export interface ViewerChoiceChipProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'children' | 'onChange' | 'type'> {
  children: ReactNode
  onCheckedChange?: (checked: boolean) => void
}

export default function ViewerChoiceChip({
  children,
  checked,
  defaultChecked,
  disabled,
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
        type="checkbox"
        checked={checked}
        defaultChecked={defaultChecked}
        disabled={disabled}
        onChange={(event) => onCheckedChange?.(event.currentTarget.checked)}
      />
      <span>{children}</span>
    </label>
  )
}
