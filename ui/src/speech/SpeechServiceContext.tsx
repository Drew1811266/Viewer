import { createContext, type ReactNode, useContext } from 'react'
import type { SpeechPreferences } from '../api/types'
import type { ViewerBridge } from '../api/viewer'

export type { SpeechPreferences } from '../api/types'

export type SpeechServicePort = Pick<
  ViewerBridge,
  | 'speechStatus'
  | 'requestSpeechPermissions'
  | 'startSpeechCapture'
  | 'stopSpeechCapture'
  | 'cancelSpeechCapture'
  | 'openSpeechSettings'
>

export interface SpeechServiceContextValue {
  port: SpeechServicePort
  preferences: SpeechPreferences
}

const SpeechServiceContext = createContext<SpeechServiceContextValue | null>(null)

export function SpeechServiceProvider({
  port,
  preferences,
  children,
}: SpeechServiceContextValue & { children: ReactNode }) {
  return (
    <SpeechServiceContext.Provider value={{ port, preferences }}>
      {children}
    </SpeechServiceContext.Provider>
  )
}

export function useSpeechService(): SpeechServiceContextValue | null {
  return useContext(SpeechServiceContext)
}
