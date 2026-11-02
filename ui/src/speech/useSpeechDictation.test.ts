import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type {
  SpeechPermissions,
  SpeechPreferences,
  SpeechStatus,
  SpeechTranscript,
} from '../api/types'
import type { SpeechServicePort } from './SpeechServiceContext'
import { appendDictationText, speechElapsedText, useSpeechDictation } from './useSpeechDictation'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((next, fail) => {
    resolve = next
    reject = fail
  })
  return { promise, resolve, reject }
}

const PREFERENCES: SpeechPreferences = {
  inputMode: 'click',
  locale: 'simplified_chinese',
  autoPunctuation: true,
}

const READY_STATUS: SpeechStatus = {
  available: true,
  speechAuthorized: true,
  microphoneAuthorized: true,
  onDevice: true,
  localeSupported: true,
  locale: 'zh-CN',
}

const TRANSCRIPT: SpeechTranscript = {
  text: '袖口再收紧一些',
  elapsedMs: 2400,
  onDevice: true,
}

function speechPort(overrides: Partial<SpeechServicePort> = {}): SpeechServicePort {
  return {
    speechStatus: vi.fn().mockResolvedValue(READY_STATUS),
    requestSpeechPermissions: vi
      .fn()
      .mockResolvedValue({ speechAuthorized: true, microphoneAuthorized: true }),
    startSpeechCapture: vi.fn().mockResolvedValue(READY_STATUS),
    stopSpeechCapture: vi.fn().mockResolvedValue(TRANSCRIPT),
    cancelSpeechCapture: vi.fn(),
    openSpeechSettings: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  }
}

function setup(port: SpeechServicePort = speechPort(), preferences = PREFERENCES) {
  const onTranscript = vi.fn()
  const onError = vi.fn()
  const hook = renderHook(() => useSpeechDictation({ port, preferences, onTranscript, onError }))
  return { hook, port, onTranscript, onError }
}

describe('speech dictation formatting', () => {
  it('formats elapsed listening time as zero-padded minutes and seconds', () => {
    expect(speechElapsedText(0)).toBe('00:00')
    expect(speechElapsedText(999)).toBe('00:00')
    expect(speechElapsedText(65_000)).toBe('01:05')
    expect(speechElapsedText(-400)).toBe('00:00')
  })

  it('appends dictation without spacing across Chinese boundaries', () => {
    expect(appendDictationText('', '  新意见  ')).toBe('新意见')
    expect(appendDictationText('已有意见', '补充一句')).toBe('已有意见补充一句')
    expect(appendDictationText('已有意见，', '继续')).toBe('已有意见，继续')
  })

  it('separates appended dictation with a space for latin text', () => {
    expect(appendDictationText('Existing note', 'more words')).toBe('Existing note more words')
    expect(appendDictationText('   ', 'Done')).toBe('Done')
    expect(appendDictationText('Existing note', '   ')).toBe('Existing note')
  })
})

describe('useSpeechDictation', () => {
  it('reports availability only for supported on-device recognition', async () => {
    const { hook } = setup()
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    const cloudOnly = setup(
      speechPort({ speechStatus: vi.fn().mockResolvedValue({ ...READY_STATUS, onDevice: false }) }),
    )
    await waitFor(() => expect(cloudOnly.hook.result.current.available).toBe(false))

    const unsupportedLocale = setup(
      speechPort({
        speechStatus: vi.fn().mockResolvedValue({ ...READY_STATUS, localeSupported: false }),
      }),
    )
    await waitFor(() => expect(unsupportedLocale.hook.result.current.available).toBe(false))
  })

  it('blocks capture with the matching permission explanation', async () => {
    const speechDenied = speechPort({
      requestSpeechPermissions: vi
        .fn()
        .mockResolvedValue({ speechAuthorized: false, microphoneAuthorized: true }),
    })
    const { hook, onError } = setup(speechDenied)
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    await act(() => hook.result.current.start())

    expect(hook.result.current.phase).toBe('error')
    expect(hook.result.current.errorKind).toBe('permission')
    expect(hook.result.current.error).toBe(
      '尚未允许语音识别，请在系统设置中允许 Viewer 使用语音识别。',
    )
    expect(onError).toHaveBeenCalledWith(
      '尚未允许语音识别，请在系统设置中允许 Viewer 使用语音识别。',
    )
    expect(speechDenied.startSpeechCapture).not.toHaveBeenCalled()

    const microphoneDenied = setup(
      speechPort({
        requestSpeechPermissions: vi
          .fn()
          .mockResolvedValue({ speechAuthorized: true, microphoneAuthorized: false }),
      }),
    )
    await waitFor(() => expect(microphoneDenied.hook.result.current.available).toBe(true))

    await act(() => microphoneDenied.hook.result.current.start())

    expect(microphoneDenied.hook.result.current.error).toBe(
      '尚未允许麦克风，请在系统设置中允许 Viewer 使用麦克风。',
    )
    expect(microphoneDenied.port.startSpeechCapture).not.toHaveBeenCalled()
  })

  it('records locally and hands a non-empty transcript back to the editor', async () => {
    const { hook, port, onTranscript } = setup()
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    await act(() => hook.result.current.start())

    expect(hook.result.current.phase).toBe('listening')
    expect(port.startSpeechCapture).toHaveBeenCalledWith({
      locale: 'simplified_chinese',
      autoPunctuation: true,
    })

    await act(() => hook.result.current.stop())

    expect(port.stopSpeechCapture).toHaveBeenCalledOnce()
    expect(onTranscript).toHaveBeenCalledWith(TRANSCRIPT)
    expect(hook.result.current.phase).toBe('idle')
    expect(hook.result.current.error).toBeNull()
  })

  it('does not hand back an empty transcript', async () => {
    const { hook, onTranscript } = setup(
      speechPort({
        stopSpeechCapture: vi.fn().mockResolvedValue({ ...TRANSCRIPT, text: '   ' }),
      }),
    )
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    await act(() => hook.result.current.start())
    await act(() => hook.result.current.stop())

    expect(onTranscript).not.toHaveBeenCalled()
    expect(hook.result.current.phase).toBe('idle')
  })

  it('surfaces recognition failures with the native message', async () => {
    const { hook, onError } = setup(
      speechPort({
        stopSpeechCapture: vi.fn().mockRejectedValue({ userMessage: '本地转写失败，请重试。' }),
      }),
    )
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    await act(() => hook.result.current.start())
    await act(() => hook.result.current.stop())

    expect(hook.result.current.phase).toBe('error')
    expect(hook.result.current.error).toBe('本地转写失败，请重试。')
    expect(onError).toHaveBeenCalledWith('本地转写失败，请重试。')
  })

  it('cancels a release that happens while permissions are still pending', async () => {
    const permissions = deferred<SpeechPermissions>()
    const port = speechPort({ requestSpeechPermissions: vi.fn(() => permissions.promise) })
    const { hook, onTranscript } = setup(port)
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    act(() => {
      void hook.result.current.start()
    })
    expect(hook.result.current.phase).toBe('requesting')

    act(() => {
      void hook.result.current.stop()
    })
    await act(async () =>
      permissions.resolve({ speechAuthorized: true, microphoneAuthorized: true }),
    )

    await waitFor(() => expect(hook.result.current.phase).toBe('idle'))
    expect(port.startSpeechCapture).not.toHaveBeenCalled()
    expect(onTranscript).not.toHaveBeenCalled()
  })

  it('transcribes a release that happens while recording starts', async () => {
    const capture = deferred<SpeechStatus>()
    const port = speechPort({ startSpeechCapture: vi.fn(() => capture.promise) })
    const { hook, onTranscript } = setup(port)
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    act(() => {
      void hook.result.current.start()
    })
    await waitFor(() => expect(port.startSpeechCapture).toHaveBeenCalledOnce())

    act(() => {
      void hook.result.current.stop()
    })
    await act(async () => capture.resolve(READY_STATUS))

    await waitFor(() => expect(onTranscript).toHaveBeenCalledWith(TRANSCRIPT))
    expect(hook.result.current.phase).toBe('idle')
  })

  it('opens the microphone privacy settings and absorbs opener failures', async () => {
    const { hook, port } = setup()
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    await act(() => hook.result.current.openSettings())

    expect(port.openSpeechSettings).toHaveBeenCalledOnce()

    const failing = setup(
      speechPort({ openSpeechSettings: vi.fn().mockRejectedValue(new Error('no opener')) }),
    )
    await waitFor(() => expect(failing.hook.result.current.available).toBe(true))
    await act(() => failing.hook.result.current.openSettings())
    expect(failing.port.openSpeechSettings).toHaveBeenCalledOnce()
  })

  it('cancels an active capture without transcribing', async () => {
    const { hook, port, onTranscript } = setup()
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    await act(() => hook.result.current.start())
    act(() => hook.result.current.cancel())

    expect(port.cancelSpeechCapture).toHaveBeenCalledOnce()
    expect(hook.result.current.phase).toBe('idle')
    expect(onTranscript).not.toHaveBeenCalled()
  })

  it('never starts recording when the editor unmounts during permission request', async () => {
    const permissions = deferred<SpeechPermissions>()
    const port = speechPort({ requestSpeechPermissions: vi.fn(() => permissions.promise) })
    const { hook } = setup(port)
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    act(() => {
      void hook.result.current.start()
    })
    hook.unmount()
    await act(async () =>
      permissions.resolve({ speechAuthorized: true, microphoneAuthorized: true }),
    )

    expect(port.startSpeechCapture).not.toHaveBeenCalled()
    expect(port.cancelSpeechCapture).toHaveBeenCalledOnce()
  })

  it('cancels the native capture when the editor unmounts', async () => {
    const { hook, port } = setup()
    await waitFor(() => expect(hook.result.current.available).toBe(true))

    await act(() => hook.result.current.start())
    hook.unmount()

    expect(port.cancelSpeechCapture).toHaveBeenCalledOnce()
  })
})
