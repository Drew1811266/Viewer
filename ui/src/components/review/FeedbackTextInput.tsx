import type { RefObject } from 'react'
import { useEffect, useRef } from 'react'
import { useFeedbackDictation } from '../../speech/useFeedbackDictation'
import { SpeechDictationPanel, SpeechMicButton } from './SpeechDictationControls'

interface FeedbackTextInputProps {
  value: string
  onChange(text: string): void
  onSave(): void
  ariaLabel: string
  id?: string
  rows?: number
  disabled?: boolean
  autoFocus?: boolean
  inputRef?: RefObject<HTMLTextAreaElement | null>
  className?: string
  micClassName?: string
  placeholder?: string
}

/**
 * The single textarea implementation used by every review feedback composer.
 * Layout-specific wrappers own positioning and action buttons; this component
 * owns typing, dictation insertion and the Command/Ctrl+Enter save gesture.
 */
export default function FeedbackTextInput({
  value,
  onChange,
  onSave,
  ariaLabel,
  id,
  rows = 3,
  disabled = false,
  autoFocus = false,
  inputRef,
  className,
  micClassName,
  placeholder,
}: FeedbackTextInputProps) {
  const fallbackRef = useRef<HTMLTextAreaElement>(null)
  const textarea = inputRef ?? fallbackRef
  const { dictation, holdToTalk } = useFeedbackDictation(value, onChange)

  useEffect(() => {
    if (autoFocus) textarea.current?.focus()
  }, [autoFocus, textarea])

  return (
    <>
      <div className={className} data-feedback-text-input>
        <textarea
          id={id}
          ref={textarea}
          aria-label={ariaLabel}
          value={value}
          rows={rows}
          disabled={disabled}
          placeholder={placeholder}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) return
            if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
              event.preventDefault()
              if (!disabled) onSave()
            }
          }}
        />
        {dictation.available && (
          <SpeechMicButton
            dictation={dictation}
            holdToTalk={holdToTalk}
            disabled={disabled}
            className={micClassName}
          />
        )}
      </div>
      <SpeechDictationPanel dictation={dictation} />
    </>
  )
}
