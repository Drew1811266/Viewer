import { useEffect } from 'react'
import type { ReviewState } from '../api/types'
import { organizationShortcutIsOwned } from './organizationShortcutOwnership'

export default function useReviewShortcuts({
  disabled,
  onSetReview,
  onToggleFavorite,
}: {
  disabled: boolean
  onSetReview: (review: ReviewState | null) => void
  onToggleFavorite: () => void
}) {
  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (
        organizationShortcutIsOwned(event, disabled) ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        event.shiftKey
      ) {
        return
      }
      const review =
        event.key === '1'
          ? 'keep'
          : event.key === '2'
            ? 'pending'
            : event.key === '3'
              ? 'reject'
              : event.key === '0'
                ? null
                : undefined
      if (review !== undefined) {
        event.preventDefault()
        onSetReview(review)
      } else if (event.key.toLowerCase() === 'f') {
        event.preventDefault()
        onToggleFavorite()
      }
    }
    window.addEventListener('keydown', shortcut)
    return () => window.removeEventListener('keydown', shortcut)
  }, [disabled, onSetReview, onToggleFavorite])
}
