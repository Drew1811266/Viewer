import { cloneElement, type ReactElement, type ReactNode, useId } from 'react'

interface ViewerFieldControlProps {
  id?: string
  'aria-describedby'?: string
  'aria-invalid'?: boolean
}

export interface ViewerFieldProps {
  label: ReactNode
  hint?: ReactNode
  error?: ReactNode
  inline?: boolean
  className?: string
  children: ReactElement<ViewerFieldControlProps>
}

export default function ViewerField({
  label,
  hint,
  error,
  inline = false,
  className,
  children,
}: ViewerFieldProps) {
  const generatedId = useId()
  const controlId = children.props.id ?? `viewer-field-${generatedId}`
  const hintId = hint === undefined ? undefined : `${controlId}-hint`
  const errorId = error === undefined ? undefined : `${controlId}-error`
  const describedBy = [children.props['aria-describedby'], hintId, errorId]
    .filter((value) => value !== undefined)
    .join(' ')
  const control = cloneElement(children, {
    id: controlId,
    'aria-describedby': describedBy || undefined,
    'aria-invalid': error === undefined ? children.props['aria-invalid'] : true,
  })

  return (
    <div
      className={className === undefined ? 'viewer-field' : `viewer-field ${className}`}
      data-inline={inline || undefined}
      data-invalid={error === undefined ? undefined : true}
    >
      <label className="viewer-field__label" htmlFor={controlId}>
        {label}
      </label>
      <span className="viewer-field__control">{control}</span>
      {hint === undefined ? null : (
        <span className="viewer-field__hint" id={hintId}>
          {hint}
        </span>
      )}
      {error === undefined ? null : (
        <span className="viewer-field__error" id={errorId} role="alert">
          {error}
        </span>
      )}
    </div>
  )
}
