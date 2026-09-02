import type { ReactNode } from 'react'
import { createContext, useContext, useMemo, useState } from 'react'
import { createPortal } from 'react-dom'

interface ViewerToolbarReviewSlotContextValue {
  host: HTMLDivElement | null
  setHost(host: HTMLDivElement | null): void
}

const ViewerToolbarReviewSlotContext = createContext<ViewerToolbarReviewSlotContextValue | null>(
  null,
)

export function ViewerToolbarReviewSlotProvider({ children }: { children: ReactNode }) {
  const [host, setHost] = useState<HTMLDivElement | null>(null)
  const value = useMemo(() => ({ host, setHost }), [host])
  return (
    <ViewerToolbarReviewSlotContext.Provider value={value}>
      {children}
    </ViewerToolbarReviewSlotContext.Provider>
  )
}

export function ViewerToolbarReviewSlot({ children }: { children: ReactNode }) {
  const slot = useContext(ViewerToolbarReviewSlotContext)
  return (
    <div className="workspace-review-toolbar-slot" ref={slot?.setHost}>
      {children}
    </div>
  )
}

export function ViewerToolbarReviewPortal({ children }: { children: ReactNode }) {
  const slot = useContext(ViewerToolbarReviewSlotContext)
  if (slot === null) return <>{children}</>
  if (slot.host === null) return null
  return createPortal(children, slot.host)
}
