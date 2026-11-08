import '@testing-library/jest-dom/vitest'

class ResizeObserverMock {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

if (globalThis.ResizeObserver === undefined) {
  globalThis.ResizeObserver = ResizeObserverMock
}
