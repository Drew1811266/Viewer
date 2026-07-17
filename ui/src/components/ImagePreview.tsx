import { useEffect, useRef, useState } from 'react'
import type { KeyboardEvent, PointerEvent } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
} from '../api/types'

interface ImagePreviewProps {
  file: BrowserFile
  files: BrowserFile[]
  requestImage: (
    file: BrowserFile,
    representation: ImageRepresentationRequest,
  ) => Promise<ImageRepresentation>
  onNavigate: (file: BrowserFile) => void
  onClose: () => void
  onDimensions?: (entityId: string, width: number, height: number) => void
}

type PreviewMode = 'fit' | 'original' | 'free'

export default function ImagePreview({
  file,
  files,
  requestImage,
  onNavigate,
  onClose,
  onDimensions,
}: ImagePreviewProps) {
  const fitCache = useRef(new Map<string, ImageRepresentation>())
  const dialog = useRef<HTMLElement>(null)
  const pendingFit = useRef(new Map<string, Promise<ImageRepresentation>>())
  const allowedWindow = useRef(new Set<string>())
  const dragStart = useRef<{ x: number; y: number; offsetX: number; offsetY: number } | null>(
    null,
  )
  const [, refresh] = useState(0)
  const [original, setOriginal] = useState<ImageRepresentation | null>(null)
  const [mode, setMode] = useState<PreviewMode>('fit')
  const [zoom, setZoom] = useState(1)
  const [rotation, setRotation] = useState(0)
  const [offset, setOffset] = useState({ x: 0, y: 0 })
  const [error, setError] = useState<string | null>(null)
  const currentIndex = files.findIndex((candidate) => candidate.entityId === file.entityId)

  useEffect(() => {
    const previous = document.activeElement
    dialog.current?.focus()
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    }
  }, [])

  useEffect(() => {
    setMode('fit')
    setZoom(1)
    setRotation(0)
    setOffset({ x: 0, y: 0 })
    setOriginal(null)
    setError(null)
  }, [file.entityId])

  useEffect(() => {
    const windowFiles = files.slice(Math.max(0, currentIndex - 1), currentIndex + 2)
    const allowed = new Set(windowFiles.map((candidate) => candidate.entityId))
    allowedWindow.current = allowed
    for (const entityId of fitCache.current.keys()) {
      if (!allowed.has(entityId)) fitCache.current.delete(entityId)
    }
    for (const candidate of windowFiles) {
      if (
        fitCache.current.has(candidate.entityId) ||
        pendingFit.current.has(candidate.entityId)
      ) {
        continue
      }
      const request = requestImage(candidate, {
        kind: 'fit_preview',
        maxWidth: 2_400,
        maxHeight: 2_400,
        scaleMilli: 1_000,
      })
      pendingFit.current.set(candidate.entityId, request)
      void request.then(
        (representation) => {
          pendingFit.current.delete(candidate.entityId)
          if (!allowedWindow.current.has(candidate.entityId)) return
          fitCache.current.set(candidate.entityId, representation)
          onDimensions?.(
            candidate.entityId,
            representation.width,
            representation.height,
          )
          refresh((value) => value + 1)
        },
        () => {
          pendingFit.current.delete(candidate.entityId)
          if (candidate.entityId === file.entityId) setError('无法预览该图片。')
        },
      )
    }
  }, [currentIndex, file.entityId, files, onDimensions, requestImage])

  useEffect(() => {
    if (mode !== 'original' || original !== null) return
    let current = true
    void requestImage(file, { kind: 'original100_percent' }).then(
      (representation) => {
        if (!current) return
        setOriginal(representation)
        onDimensions?.(file.entityId, representation.width, representation.height)
      },
      (caught: unknown) => {
        if (!current) return
        if (commandCode(caught) === 'image_budget_exceeded') {
          setMode('fit')
          setError('原图超出安全预览限制，已返回适应窗口模式。')
        } else {
          setMode('fit')
          setError('无法加载原图，已返回适应窗口模式。')
        }
      },
    )
    return () => {
      current = false
    }
  }, [file, mode, onDimensions, original, requestImage])

  const representation = mode === 'original' ? original : fitCache.current.get(file.entityId)
  const scale = mode === 'free' ? zoom : 1
  const translated = offset.x === 0 && offset.y === 0 ? '' : `translate(${offset.x}px, ${offset.y}px) `

  function navigate(delta: number) {
    const next = files[currentIndex + delta]
    if (next) onNavigate(next)
  }

  function keyboard(event: KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
    }
    if (event.key === 'ArrowLeft') {
      event.preventDefault()
      navigate(-1)
    }
    if (event.key === 'ArrowRight') {
      event.preventDefault()
      navigate(1)
    }
  }

  function zoomBy(factor: number) {
    setMode('free')
    setZoom((value) => Math.max(0.1, Math.min(8, value * factor)))
  }

  function pointerDown(event: PointerEvent<HTMLDivElement>) {
    const bounds = panBounds(
      representation,
      event.currentTarget,
      mode,
      zoom,
      rotation,
    )
    if (bounds.x === 0 && bounds.y === 0) return
    dragStart.current = {
      x: event.clientX,
      y: event.clientY,
      offsetX: offset.x,
      offsetY: offset.y,
    }
    event.currentTarget.setPointerCapture?.(event.pointerId)
  }

  function pointerMove(event: PointerEvent<HTMLDivElement>) {
    const start = dragStart.current
    if (start === null) return
    const bounds = panBounds(
      representation,
      event.currentTarget,
      mode,
      zoom,
      rotation,
    )
    setOffset({
      x: clamp(start.offsetX + event.clientX - start.x, -bounds.x, bounds.x),
      y: clamp(start.offsetY + event.clientY - start.y, -bounds.y, bounds.y),
    })
  }

  return (
    <section
      ref={dialog}
      className="preview-overlay image-preview"
      role="dialog"
      aria-label="图片预览"
      tabIndex={-1}
      onKeyDown={keyboard}
    >
      <header className="preview-toolbar">
        <strong>{file.name}</strong>
        <button type="button" onClick={() => setMode('fit')}>
          适应窗口
        </button>
        <button type="button" aria-label="按 100% 显示" onClick={() => setMode('original')}>
          100%
        </button>
        <button type="button" aria-label="缩小" onClick={() => zoomBy(0.8)}>
          −
        </button>
        <span>{Math.round(scale * 100)}%</span>
        <button type="button" aria-label="放大" onClick={() => zoomBy(1.25)}>
          +
        </button>
        <button
          type="button"
          aria-label="顺时针旋转"
          onClick={() => setRotation((value) => (value + 90) % 360)}
        >
          ↻
        </button>
        <button type="button" aria-label="关闭预览" onClick={onClose}>
          ×
        </button>
      </header>
      {error && <p role="alert">{error}</p>}
      <div
        className="image-preview-stage"
        onPointerDown={pointerDown}
        onPointerMove={pointerMove}
        onPointerUp={() => {
          dragStart.current = null
        }}
      >
        {representation ? (
          <img
            src={representation.url}
            alt={file.name}
            data-mode={mode}
            style={{ transform: `${translated}rotate(${rotation}deg) scale(${scale})` }}
          />
        ) : (
          <p>正在加载图片…</p>
        )}
      </div>
      <footer>
        <button type="button" disabled={currentIndex <= 0} onClick={() => navigate(-1)}>
          上一张
        </button>
        <span>
          {currentIndex + 1} / {files.length}
        </span>
        <button
          type="button"
          disabled={currentIndex < 0 || currentIndex >= files.length - 1}
          onClick={() => navigate(1)}
        >
          下一张
        </button>
      </footer>
    </section>
  )
}

function commandCode(error: unknown): string | null {
  if (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    typeof error.code === 'string'
  ) {
    return error.code
  }
  return null
}

function panBounds(
  representation: ImageRepresentation | undefined | null,
  stage: HTMLDivElement,
  mode: PreviewMode,
  zoom: number,
  rotation: number,
): { x: number; y: number } {
  if (representation === null || representation === undefined || mode === 'fit') {
    return { x: 0, y: 0 }
  }
  const stageWidth = stage.clientWidth
  const stageHeight = stage.clientHeight
  if (stageWidth <= 0 || stageHeight <= 0) return { x: 0, y: 0 }
  const containScale =
    mode === 'free'
      ? Math.min(
          1,
          (stageWidth * 0.9) / representation.width,
          (stageHeight * 0.9) / representation.height,
        )
      : 1
  let width = representation.width * containScale * (mode === 'free' ? zoom : 1)
  let height = representation.height * containScale * (mode === 'free' ? zoom : 1)
  if (rotation % 180 !== 0) [width, height] = [height, width]
  return {
    x: Math.max(0, (width - stageWidth) / 2),
    y: Math.max(0, (height - stageHeight) / 2),
  }
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}
