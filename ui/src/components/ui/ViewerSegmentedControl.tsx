import type { HTMLAttributes, ReactNode } from 'react'

export interface ViewerSegmentedControlProps extends Omit<HTMLAttributes<HTMLDivElement>, 'role'> {
  label: string
  children: ReactNode
}

export default function ViewerSegmentedControl({
  label,
  className,
  children,
  ...containerProps
}: ViewerSegmentedControlProps) {
  return (
    <div
      {...containerProps}
      className={
        className === undefined
          ? 'viewer-segmented-control'
          : `viewer-segmented-control ${className}`
      }
      role="toolbar"
      aria-label={label}
    >
      {children}
    </div>
  )
}
