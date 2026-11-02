import { useRef } from 'react'
import type { SpeechPreferences } from '../api/types'
import { useSpeechService } from './SpeechServiceContext'
import { appendDictationText, useSpeechDictation } from './useSpeechDictation'

const FALLBACK_PREFERENCES: SpeechPreferences = {
  inputMode: 'click',
  locale: 'simplified_chinese',
  autoPunctuation: true,
}

export function useFeedbackDictation(text: string, updateText: (text: string) => void) {
  const speechService = useSpeechService()
  const currentTextRef = useRef(text)
  currentTextRef.current = text
  const dictation = useSpeechDictation({
    port: speechService?.port ?? null,
    preferences: speechService?.preferences ?? FALLBACK_PREFERENCES,
    onTranscript: (transcript) => {
      updateText(appendDictationText(currentTextRef.current, transcript.text))
    },
    onError: () => undefined,
  })
  return {
    dictation,
    holdToTalk: speechService?.preferences.inputMode === 'hold',
  }
}
