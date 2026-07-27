import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import type { ThumbnailDensity } from '../api/types'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'
import { THUMBNAIL_HEIGHT } from './thumbnailDensity'

export interface ViewerSettingsContextValue {
  thumbnailDensity: ThumbnailDensity
  thumbnailHeight: number
  settingsError: string | null
  setThumbnailDensity: (density: ThumbnailDensity) => void
}

interface ViewerSettingsProviderProps {
  bridge: Pick<ViewerBridge, 'getViewerSettings' | 'updateThumbnailDensity'>
  children: ReactNode
}

const ViewerSettingsContext = createContext<ViewerSettingsContextValue | null>(null)

export function ViewerSettingsProvider({ bridge, children }: ViewerSettingsProviderProps) {
  const [thumbnailDensity, setThumbnailDensityState] = useState<ThumbnailDensity>('standard')
  const [settingsError, setSettingsError] = useState<string | null>(null)
  const confirmedRef = useRef<ThumbnailDensity>('standard')
  const latestSequenceRef = useRef(0)
  const writeTailRef = useRef<Promise<void>>(Promise.resolve())

  useEffect(() => {
    let active = true
    void bridge.getViewerSettings().then(
      (loaded) => {
        if (!active || latestSequenceRef.current !== 0) return
        confirmedRef.current = loaded.thumbnailDensity
        setThumbnailDensityState(loaded.thumbnailDensity)
      },
      () => undefined,
    )
    return () => {
      active = false
    }
  }, [bridge])

  const setThumbnailDensity = useCallback(
    (nextDensity: ThumbnailDensity) => {
      setSettingsError(null)
      setThumbnailDensityState(nextDensity)
      const sequence = ++latestSequenceRef.current
      writeTailRef.current = writeTailRef.current
        .catch(() => undefined)
        .then(() => bridge.updateThumbnailDensity(nextDensity))
        .then(
          (saved) => {
            confirmedRef.current = saved.thumbnailDensity
            if (sequence === latestSequenceRef.current) {
              setThumbnailDensityState(saved.thumbnailDensity)
              setSettingsError(null)
            }
          },
          (error: unknown) => {
            if (sequence === latestSequenceRef.current) {
              setThumbnailDensityState(confirmedRef.current)
              setSettingsError(safeUserMessage(error))
            }
          },
        )
    },
    [bridge],
  )

  const value = useMemo<ViewerSettingsContextValue>(
    () => ({
      thumbnailDensity,
      thumbnailHeight: THUMBNAIL_HEIGHT[thumbnailDensity],
      settingsError,
      setThumbnailDensity,
    }),
    [settingsError, setThumbnailDensity, thumbnailDensity],
  )

  return <ViewerSettingsContext.Provider value={value}>{children}</ViewerSettingsContext.Provider>
}

export function useViewerSettings(): ViewerSettingsContextValue {
  const context = useContext(ViewerSettingsContext)
  if (context === null) throw new Error('useViewerSettings requires ViewerSettingsProvider')
  return context
}
