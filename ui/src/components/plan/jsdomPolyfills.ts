// jsdom 缺少 React Flow 依赖的两个浏览器 API。只在本目录的测试里安装，
// 不要放进全局 setup，否则会改变其它测试观察到的 DOM 能力。

class ResizeObserverPolyfill {
  observe() {}
  unobserve() {}
  disconnect() {}
}

class DOMMatrixReadOnlyPolyfill {
  m22 = 1

  constructor(transform?: string) {
    const scale = transform?.match(/scale\(([^)]+)\)/)?.[1]
    if (scale) this.m22 = Number(scale)
  }
}

if (!('ResizeObserver' in globalThis)) {
  Object.defineProperty(globalThis, 'ResizeObserver', {
    value: ResizeObserverPolyfill,
    configurable: true,
  })
}

if (!('DOMMatrixReadOnly' in globalThis)) {
  Object.defineProperty(globalThis, 'DOMMatrixReadOnly', {
    value: DOMMatrixReadOnlyPolyfill,
    configurable: true,
  })
}

export {}
