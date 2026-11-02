import { useCallback, useEffect, useRef, useState } from 'react'
import type { SpeechPreferences, SpeechStatus, SpeechTranscript } from '../api/types'
import { safeUserMessage } from '../api/viewer'
import type { SpeechServicePort } from './SpeechServiceContext'

export type SpeechDictationPhase = 'idle' | 'requesting' | 'listening' | 'transcribing' | 'error'

export type SpeechDictationErrorKind = 'permission' | 'unavailable' | 'capture'

export interface SpeechDictation {
  available: boolean
  phase: SpeechDictationPhase
  elapsedMs: number
  error: string | null
  errorKind: SpeechDictationErrorKind | null
  start(): Promise<void>
  stop(): Promise<void>
  cancel(): void
  openSettings(): Promise<void>
}

export function useSpeechDictation(options: {
  port: SpeechServicePort | null
  preferences: SpeechPreferences
  onTranscript(transcript: SpeechTranscript): void
  onError(message: string): void
}): SpeechDictation {
  const { port, preferences, onTranscript, onError } = options
  const [status, setStatus] = useState<SpeechStatus | null>(null)
  const [phase, setPhase] = useState<SpeechDictationPhase>('idle')
  const [elapsedMs, setElapsedMs] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [errorKind, setErrorKind] = useState<SpeechDictationErrorKind | null>(null)
  const phaseRef = useRef<SpeechDictationPhase>('idle')
  const startedAtRef = useRef(0)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const stopRequestedRef = useRef(false)
  const stopRef = useRef<() => void>(() => undefined)
  const mountedRef = useRef(true)
  const transcriptRef = useRef(onTranscript)
  const errorRef = useRef(onError)
  transcriptRef.current = onTranscript
  errorRef.current = onError

  const updatePhase = useCallback((next: SpeechDictationPhase) => {
    phaseRef.current = next
    setPhase(next)
  }, [])

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current)
      timerRef.current = null
    }
  }, [])

  useEffect(() => {
    if (port === null) {
      setStatus(null)
      return
    }
    let active = true
    void port
      .speechStatus({
        locale: preferences.locale,
        autoPunctuation: preferences.autoPunctuation,
      })
      .then(
        (next) => {
          if (active) setStatus(next)
        },
        () => {
          if (active) setStatus(null)
        },
      )
    return () => {
      active = false
    }
  }, [port, preferences.autoPunctuation, preferences.locale])

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
      clearTimer()
      port?.cancelSpeechCapture()
    }
  }, [clearTimer, port])

  const start = useCallback(async () => {
    if (port === null) return
    if (phaseRef.current !== 'idle' && phaseRef.current !== 'error') return
    stopRequestedRef.current = false
    setError(null)
    setErrorKind(null)
    updatePhase('requesting')
    try {
      const permissions = await port.requestSpeechPermissions()
      if (!mountedRef.current) {
        stopRequestedRef.current = false
        return
      }
      if (!permissions.speechAuthorized || !permissions.microphoneAuthorized) {
        stopRequestedRef.current = false
        const message = !permissions.speechAuthorized
          ? '尚未允许语音识别，请在系统设置中允许 Viewer 使用语音识别。'
          : '尚未允许麦克风，请在系统设置中允许 Viewer 使用麦克风。'
        setError(message)
        setErrorKind('permission')
        errorRef.current(message)
        updatePhase('error')
        return
      }
      if (stopRequestedRef.current) {
        stopRequestedRef.current = false
        updatePhase('idle')
        return
      }
      const started = await port.startSpeechCapture({
        locale: preferences.locale,
        autoPunctuation: preferences.autoPunctuation,
      })
      if (!mountedRef.current) {
        port.cancelSpeechCapture()
        stopRequestedRef.current = false
        return
      }
      setStatus(started)
      if (!started.available) {
        stopRequestedRef.current = false
        const message = '当前无法使用本地语音识别，请稍后重试。'
        setError(message)
        setErrorKind('unavailable')
        errorRef.current(message)
        updatePhase('error')
        return
      }
      startedAtRef.current = performance.now()
      setElapsedMs(0)
      clearTimer()
      timerRef.current = setInterval(() => {
        setElapsedMs(Math.max(0, Math.round(performance.now() - startedAtRef.current)))
      }, 250)
      updatePhase('listening')
      if (stopRequestedRef.current) {
        stopRequestedRef.current = false
        stopRef.current()
      }
    } catch (cause) {
      stopRequestedRef.current = false
      if (!mountedRef.current) return
      const message = speechErrorMessage(cause)
      setError(message)
      setErrorKind('capture')
      errorRef.current(message)
      updatePhase('error')
    }
  }, [clearTimer, port, preferences.autoPunctuation, preferences.locale, updatePhase])

  const stop = useCallback(async () => {
    if (port === null) return
    if (phaseRef.current === 'requesting') {
      stopRequestedRef.current = true
      return
    }
    if (phaseRef.current !== 'listening') return
    clearTimer()
    updatePhase('transcribing')
    try {
      const transcript = await port.stopSpeechCapture()
      if (!mountedRef.current) return
      if (transcript.text.trim().length > 0) transcriptRef.current(transcript)
      setError(null)
      setErrorKind(null)
      updatePhase('idle')
    } catch (cause) {
      if (!mountedRef.current) return
      const message = speechErrorMessage(cause)
      setError(message)
      setErrorKind('capture')
      errorRef.current(message)
      updatePhase('error')
    }
  }, [clearTimer, port, updatePhase])
  stopRef.current = () => {
    void stop()
  }

  const cancel = useCallback(() => {
    stopRequestedRef.current = false
    clearTimer()
    port?.cancelSpeechCapture()
    setElapsedMs(0)
    setError(null)
    setErrorKind(null)
    updatePhase('idle')
  }, [clearTimer, port, updatePhase])

  const openSettings = useCallback(async () => {
    await port?.openSpeechSettings().catch(() => undefined)
  }, [port])

  return {
    available: Boolean(status?.available && status.localeSupported && status.onDevice),
    phase,
    elapsedMs,
    error,
    errorKind,
    start,
    stop,
    cancel,
    openSettings,
  }
}

export function speechElapsedText(elapsedMs: number): string {
  const totalSeconds = Math.floor(Math.max(0, elapsedMs) / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

export function appendDictationText(current: string, transcript: string): string {
  const spoken = transcript.trim()
  if (spoken.length === 0) return current
  if (current.trim().length === 0) return spoken
  if (/[\u3000-\u9fff\uff00-\uffef]$/.test(current) || /^[\u3000-\u9fff\uff00-\uffef]/.test(spoken))
    return `${current}${spoken}`
  const spacer = /[\s，。！？、；：,.!?;:]$/.test(current) ? '' : ' '
  return `${current}${spacer}${spoken}`
}

function speechErrorMessage(cause: unknown): string {
  const message = safeUserMessage(cause)
  return message.length > 0 ? message : '语音转写失败，请重试。'
}
