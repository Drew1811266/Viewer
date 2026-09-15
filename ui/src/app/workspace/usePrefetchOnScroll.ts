import { useEffect, useRef } from 'react'

/** 滚动静止判定窗口：静止超过该时长才触发预取（性能方案 §3.2）。 */
export const PREFETCH_REST_MS = 120
/** 快甩速度阈值（px/ms）：超过则判定为甩动，不预取，等静止。 */
export const PREFETCH_FLICK_SPEED_PX_PER_MS = 2
/**
 * 单次静止预取的行数（2026-09-15：从 1 行扩展到 3 行）。滚动停止后沿方向
 * 连续预热 3 行（目录图片 + 前 6 张缩略图），DecodeScheduler 并发 3 吸收
 * 请求；配合 ADR 0007 内存缓存，继续滚动时行内容大概率已就绪。
 */
export const PREFETCH_ROW_AHEAD = 3

export type ScrollDirection = 'up' | 'down'

export interface PrefetchOnScrollController {
  /** 返回垂直滚动容器（由宿主解析，便于测试注入）。 */
  getContainer(): HTMLElement | null
  rowCount: number
  /** 单行估算高度（px），用于把滚动位置映射为行号。 */
  rowHeight: number
  /** 预取第 rowIndex 行；signal 在方向反转、新预取或卸载时被 abort。 */
  onPrefetchRow(rowIndex: number, signal: AbortSignal): void
}

/**
 * 滚动位置（px）到预取目标行号的映射。向下预取视口下边缘外第一行，
 * 向上对称预取上边缘外第一行；越界返回 null。
 */
export function computePrefetchTarget(
  scrollTop: number,
  clientHeight: number,
  rowHeight: number,
  rowCount: number,
  direction: ScrollDirection,
): number | null {
  if (rowCount === 0 || rowHeight <= 0 || clientHeight <= 0) return null
  if (direction === 'down') {
    const firstRowBelow = Math.floor((scrollTop + clientHeight) / rowHeight)
    return firstRowBelow >= rowCount ? null : Math.max(0, firstRowBelow)
  }
  const firstRowAbove = Math.ceil(scrollTop / rowHeight) - 2
  return firstRowAbove < 0 ? null : Math.min(rowCount - 1, firstRowAbove)
}

/** 从节点向上找最近的可垂直滚动祖先（真实滚动宿主是 `.workspace`）。 */
export function findVerticalScrollAncestor(node: HTMLElement | null): HTMLElement | null {
  let current = node?.parentElement ?? null
  while (current !== null) {
    const overflowY = window.getComputedStyle(current).overflowY
    if (overflowY === 'auto' || overflowY === 'scroll') return current
    current = current.parentElement
  }
  return null
}

export function usePrefetchOnScroll(controller: PrefetchOnScrollController): void {
  const controllerRef = useRef(controller)
  controllerRef.current = controller

  useEffect(() => {
    const container = controllerRef.current.getContainer()
    if (container === null) return

    let rafId: number | null = null
    let restTimer: ReturnType<typeof setTimeout> | null = null
    let lastScrollTop = container.scrollTop
    let lastEventTime = performance.now()
    let lastVelocity = 0
    let lastDirection: ScrollDirection | null = null
    let activeController: AbortController | null = null
    let activePrefetchTargets = new Set<number>()

    const clearRestTimer = () => {
      if (restTimer !== null) {
        clearTimeout(restTimer)
        restTimer = null
      }
    }

    const abortActive = () => {
      activeController?.abort()
      activeController = null
      activePrefetchTargets = new Set()
    }

    const prefetch = (direction: ScrollDirection) => {
      if (lastVelocity >= PREFETCH_FLICK_SPEED_PX_PER_MS) return
      const { rowCount, rowHeight } = controllerRef.current
      const base = computePrefetchTarget(
        container.scrollTop,
        container.clientHeight,
        rowHeight,
        rowCount,
        direction,
      )
      if (base === null) return
      const step = direction === 'down' ? 1 : -1
      for (let offset = 0; offset < PREFETCH_ROW_AHEAD; offset += 1) {
        const target = base + offset * step
        if (target < 0 || target >= rowCount) break
        if (activePrefetchTargets.has(target)) continue
        if (activeController === null) activeController = new AbortController()
        activePrefetchTargets.add(target)
        controllerRef.current.onPrefetchRow(target, activeController.signal)
      }
    }

    const processScroll = () => {
      const scrollTop = container.scrollTop
      const now = performance.now()
      const delta = scrollTop - lastScrollTop
      const elapsed = Math.max(1, now - lastEventTime)
      lastScrollTop = scrollTop
      lastEventTime = now
      if (delta === 0) return
      const direction: ScrollDirection = delta > 0 ? 'down' : 'up'
      lastVelocity = Math.abs(delta) / elapsed
      if (direction !== lastDirection) {
        lastDirection = direction
        abortActive()
      }
      clearRestTimer()
      restTimer = setTimeout(() => prefetch(direction), PREFETCH_REST_MS)
    }

    const handleScroll = () => {
      if (rafId !== null) return
      rafId = requestAnimationFrame(() => {
        rafId = null
        processScroll()
      })
    }

    container.addEventListener('scroll', handleScroll, { passive: true })
    return () => {
      container.removeEventListener('scroll', handleScroll)
      if (rafId !== null) cancelAnimationFrame(rafId)
      clearRestTimer()
      abortActive()
    }
  }, [])
}
