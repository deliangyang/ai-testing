import { mergeShellLayout, type ShellLayout } from './shellLayout'

const PREFIX = 'aitest.shell.layout.v1:'

export function loadShellLayout(scopeKey: string): ShellLayout | null {
  if (typeof localStorage === 'undefined') {
    return null
  }
  const raw = localStorage.getItem(PREFIX + scopeKey)
  if (!raw) {
    return null
  }
  try {
    return mergeShellLayout(JSON.parse(raw) as Partial<ShellLayout>)
  } catch {
    return null
  }
}

export function saveShellLayout(scopeKey: string, layout: ShellLayout): void {
  if (typeof localStorage === 'undefined') {
    return
  }
  localStorage.setItem(PREFIX + scopeKey, JSON.stringify(layout))
}
