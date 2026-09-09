import type { KeyboardEvent, ReactNode, Ref } from 'react'
import { useEffect, useRef } from 'react'
import ViewerToolbar from '../ui/ViewerToolbar'

interface ImagePreviewChromeProps {
  ariaLabel: string
  toolbarLabel: string
  toolbarLeading?: ReactNode
  toolbarCenter: ReactNode
  toolbarActions: ReactNode
  stage: ReactNode
  sidePanel?: ReactNode
  navigation: ReactNode
  navigationRef?: Ref<HTMLElement>
  onKeyDown(event: KeyboardEvent<HTMLElement>): void
}

export default function ImagePreviewChrome({
  ariaLabel,
  toolbarLabel,
  toolbarLeading,
  toolbarCenter,
  toolbarActions,
  stage,
  sidePanel,
  navigation,
  navigationRef,
  onKeyDown,
}: ImagePreviewChromeProps) {
  const dialog = useRef<HTMLElement>(null)

  useEffect(() => {
    const previous = document.activeElement
    dialog.current?.focus()
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    }
  }, [])

  return (
    <section
      ref={dialog}
      className="preview-overlay image-preview"
      role="dialog"
      aria-label={ariaLabel}
      tabIndex={-1}
      onKeyDown={onKeyDown}
    >
      <ViewerToolbar
        label={toolbarLabel}
        leading={toolbarLeading}
        center={toolbarCenter}
        actions={toolbarActions}
      />
      <div className="image-preview-surface__content">
        {stage}
        {sidePanel}
      </div>
      <nav ref={navigationRef} className="preview-navigation-float" aria-label="图片导航">
        {navigation}
      </nav>
    </section>
  )
}
