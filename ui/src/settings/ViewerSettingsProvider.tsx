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
import type {
  MagnifierArea,
  MagnifierMagnification,
  MagnifierPreferences,
  MagnifierShape,
  ThumbnailDensity,
  ViewerSettings,
  ViewerSettingsUpdate,
} from '../api/types'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'
import { THUMBNAIL_HEIGHT } from './thumbnailDensity'
import { DEFAULT_VIEWER_SETTINGS_UPDATE } from './viewerSettings'

export interface ViewerSettingsContextValue {
  thumbnailDensity: ThumbnailDensity
  thumbnailHeight: number
  magnifier: MagnifierPreferences
  settingsError: string | null
  setThumbnailDensity: (density: ThumbnailDensity) => void
  setMagnifierShape: (shape: MagnifierShape) => void
  setMagnifierMagnification: (magnification: MagnifierMagnification) => void
  setMagnifierArea: (area: MagnifierArea) => void
}

interface ViewerSettingsProviderProps {
  bridge: Pick<ViewerBridge, 'getViewerSettings' | 'updateViewerSettings'>
  children: ReactNode
}

const ViewerSettingsContext = createContext<ViewerSettingsContextValue | null>(null)

function withoutSchema(settings: ViewerSettings): ViewerSettingsUpdate {
  return {
    thumbnailDensity: settings.thumbnailDensity,
    magnifier: { ...settings.magnifier },
  }
}

function initialSettings(): ViewerSettingsUpdate {
  return {
    thumbnailDensity: DEFAULT_VIEWER_SETTINGS_UPDATE.thumbnailDensity,
    magnifier: { ...DEFAULT_VIEWER_SETTINGS_UPDATE.magnifier },
  }
}

export function ViewerSettingsProvider({ bridge, children }: ViewerSettingsProviderProps) {
  const [settings, setSettings] = useState<ViewerSettingsUpdate>(initialSettings)
  const [settingsError, setSettingsError] = useState<string | null>(null)
  const optimisticRef = useRef<ViewerSettingsUpdate>(initialSettings())
  const confirmedRef = useRef<ViewerSettingsUpdate>(initialSettings())
  const latestSequenceRef = useRef(0)
  const writeTailRef = useRef<Promise<void>>(Promise.resolve())

  useEffect(() => {
    let active = true
    void bridge.getViewerSettings().then(
      (loaded) => {
        if (!active || latestSequenceRef.current !== 0) return
        const next = withoutSchema(loaded)
        confirmedRef.current = next
        optimisticRef.current = next
        setSettings(next)
      },
      () => undefined,
    )
    return () => {
      active = false
    }
  }, [bridge])

  const enqueue = useCallback(
    (next: ViewerSettingsUpdate) => {
      setSettingsError(null)
      const sequence = ++latestSequenceRef.current
      writeTailRef.current = writeTailRef.current
        .catch(() => undefined)
        .then(() => bridge.updateViewerSettings(next))
        .then(
          (saved) => {
            const confirmed = withoutSchema(saved)
            confirmedRef.current = confirmed
            if (sequence === latestSequenceRef.current) {
              optimisticRef.current = confirmed
              setSettings(confirmed)
              setSettingsError(null)
            }
          },
          (error: unknown) => {
            if (sequence !== latestSequenceRef.current) return
            optimisticRef.current = confirmedRef.current
            setSettings(confirmedRef.current)
            setSettingsError(safeUserMessage(error))
          },
        )
    },
    [bridge],
  )

  const updateOptimistically = useCallback(
    (recipe: (current: ViewerSettingsUpdate) => ViewerSettingsUpdate) => {
      const next = recipe(optimisticRef.current)
      optimisticRef.current = next
      setSettings(next)
      enqueue(next)
    },
    [enqueue],
  )

  const setThumbnailDensity = useCallback(
    (thumbnailDensity: ThumbnailDensity) => {
      updateOptimistically((current) => ({ ...current, thumbnailDensity }))
    },
    [updateOptimistically],
  )

  const setMagnifierShape = useCallback(
    (shape: MagnifierShape) => {
      updateOptimistically((current) => ({
        ...current,
        magnifier: { ...current.magnifier, shape },
      }))
    },
    [updateOptimistically],
  )

  const setMagnifierMagnification = useCallback(
    (magnification: MagnifierMagnification) => {
      updateOptimistically((current) => ({
        ...current,
        magnifier: { ...current.magnifier, magnification },
      }))
    },
    [updateOptimistically],
  )

  const setMagnifierArea = useCallback(
    (area: MagnifierArea) => {
      updateOptimistically((current) => ({
        ...current,
        magnifier: { ...current.magnifier, area },
      }))
    },
    [updateOptimistically],
  )

  const value = useMemo<ViewerSettingsContextValue>(
    () => ({
      thumbnailDensity: settings.thumbnailDensity,
      thumbnailHeight: THUMBNAIL_HEIGHT[settings.thumbnailDensity],
      magnifier: settings.magnifier,
      settingsError,
      setThumbnailDensity,
      setMagnifierShape,
      setMagnifierMagnification,
      setMagnifierArea,
    }),
    [
      setMagnifierArea,
      setMagnifierMagnification,
      setMagnifierShape,
      setThumbnailDensity,
      settings,
      settingsError,
    ],
  )

  return <ViewerSettingsContext.Provider value={value}>{children}</ViewerSettingsContext.Provider>
}

export function useViewerSettings(): ViewerSettingsContextValue {
  const context = useContext(ViewerSettingsContext)
  if (context === null) throw new Error('useViewerSettings requires ViewerSettingsProvider')
  return context
}
