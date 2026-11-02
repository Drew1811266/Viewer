import type { SpeechDictation } from '../../speech/useSpeechDictation'
import { speechElapsedText } from '../../speech/useSpeechDictation'
import ViewerButton, { ViewerIconButton } from '../ui/ViewerButton'
import ViewerIcon from '../ui/ViewerIcon'

interface SpeechMicButtonProps {
  dictation: SpeechDictation
  holdToTalk: boolean
  disabled?: boolean
  className?: string
}

export function SpeechMicButton({
  dictation,
  holdToTalk,
  disabled = false,
  className,
}: SpeechMicButtonProps) {
  const listening = dictation.phase === 'listening'
  const busy = dictation.phase === 'requesting' || dictation.phase === 'transcribing'
  return (
    <ViewerIconButton
      icon="mic"
      label={listening ? '停止语音输入' : '语音输入'}
      tone="quiet"
      active={listening}
      className={className}
      disabled={disabled || busy}
      aria-pressed={listening}
      onPointerDown={
        holdToTalk
          ? (event) => {
              event.preventDefault()
              void dictation.start()
            }
          : undefined
      }
      onPointerUp={holdToTalk ? () => void dictation.stop() : undefined}
      onPointerLeave={holdToTalk && listening ? () => void dictation.stop() : undefined}
      onClick={
        holdToTalk
          ? undefined
          : () => {
              if (listening) void dictation.stop()
              else void dictation.start()
            }
      }
    />
  )
}

export function SpeechDictationPanel({ dictation }: { dictation: SpeechDictation }) {
  if (dictation.phase === 'requesting') {
    return (
      <p className="speech-dictation__note" role="status">
        正在请求麦克风与语音识别权限…
      </p>
    )
  }
  if (dictation.phase === 'transcribing') {
    return (
      <p className="speech-dictation__note" role="status">
        正在本地转写…
      </p>
    )
  }
  if (dictation.phase === 'error' && dictation.errorKind === 'permission') {
    const speechDenied = dictation.error?.includes('语音识别') === true
    return (
      <div className="speech-dictation-error" role="alert">
        <strong className="speech-dictation-error__title">
          <ViewerIcon name="mic" />
          {speechDenied ? 'Viewer 无法使用语音识别' : 'Viewer 无法访问麦克风'}
        </strong>
        <p>
          {speechDenied
            ? '请在系统设置中允许 Viewer 使用语音识别，然后回到这里重试。音频仅在本机处理。'
            : '请在系统设置中允许 Viewer 使用麦克风，然后回到这里重试。音频仅在本机处理。'}
        </p>
        <p className="speech-dictation-error__kept">草稿已保留</p>
        <div className="speech-dictation-error__actions">
          <ViewerButton tone="primary" onClick={() => void dictation.openSettings()}>
            打开系统设置
          </ViewerButton>
          <ViewerButton tone="secondary" onClick={() => void dictation.start()}>
            重试
          </ViewerButton>
        </div>
        <p className="speech-dictation-error__keyboard">
          改用键盘输入
          <br />
          不使用麦克风也可以继续写文字意见
        </p>
      </div>
    )
  }
  if (dictation.phase === 'error' && dictation.error !== null) {
    return (
      <div className="speech-dictation-error" role="alert">
        <p>{dictation.error}</p>
        <div className="speech-dictation-error__actions">
          <ViewerButton tone="secondary" onClick={() => void dictation.start()}>
            重试
          </ViewerButton>
        </div>
      </div>
    )
  }
  if (dictation.phase !== 'listening') return null
  return (
    <div className="speech-dictation" role="status" aria-live="polite">
      <div className="speech-dictation__status">
        <span>正在聆听</span>
        <span>· {speechElapsedText(dictation.elapsedMs)}</span>
      </div>
      <div className="speech-dictation__wave" aria-hidden="true">
        {DICTATION_BARS.map((bar) => (
          <span key={bar.id} style={{ height: `${bar.height}px` }} />
        ))}
      </div>
      <p>本地转写 · 音频不上传</p>
      <ViewerButton tone="primary" onClick={() => void dictation.stop()}>
        停止并转写
      </ViewerButton>
      <button type="button" className="speech-dictation__cancel" onClick={dictation.cancel}>
        取消
      </button>
    </div>
  )
}

const DICTATION_BARS = [
  { id: 1, height: 6 },
  { id: 2, height: 12 },
  { id: 3, height: 18 },
  { id: 4, height: 10 },
  { id: 5, height: 22 },
  { id: 6, height: 14 },
  { id: 7, height: 8 },
  { id: 8, height: 16 },
  { id: 9, height: 24 },
  { id: 10, height: 12 },
  { id: 11, height: 7 },
  { id: 12, height: 15 },
  { id: 13, height: 20 },
  { id: 14, height: 9 },
  { id: 15, height: 13 },
  { id: 16, height: 19 },
  { id: 17, height: 11 },
  { id: 18, height: 6 },
]
