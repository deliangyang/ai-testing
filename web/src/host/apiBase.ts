/** Resolve API base for `HttpDataSource` when embedded in NSH WebView or dev. */
export function readInjectedApiBase(): string | undefined {
  if (typeof window === 'undefined') {
    return undefined
  }
  const injected = (window as Window & { __AITEST_API_BASE__?: string }).__AITEST_API_BASE__
  if (injected) {
    return injected
  }
  const fromQuery = new URLSearchParams(window.location.search).get('api_base')
  return fromQuery ?? undefined
}

export function installInjectedApiBase(): void {
  const base = readInjectedApiBase()
  if (base) {
    ;(window as Window & { __AITEST_API_BASE__?: string }).__AITEST_API_BASE__ = base
  }
}
