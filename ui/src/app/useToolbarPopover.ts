import { useCallback, useState } from 'react'

export type ToolbarPopover = 'filter' | 'view' | 'more' | null
type OpenToolbarPopover = Exclude<ToolbarPopover, null>

export interface ToolbarPopoverState {
  openPopover: ToolbarPopover
  setPopoverOpen(name: OpenToolbarPopover, open: boolean): void
  togglePopover(name: OpenToolbarPopover): void
  closePopover(): void
}

export function useToolbarPopover(): ToolbarPopoverState {
  const [openPopover, setOpenPopover] = useState<ToolbarPopover>(null)
  const setPopoverOpen = useCallback((name: OpenToolbarPopover, open: boolean) => {
    setOpenPopover((current) => (open ? name : current === name ? null : current))
  }, [])
  const togglePopover = useCallback((name: OpenToolbarPopover) => {
    setOpenPopover((current) => (current === name ? null : name))
  }, [])
  const closePopover = useCallback(() => setOpenPopover(null), [])

  return { openPopover, setPopoverOpen, togglePopover, closePopover }
}
