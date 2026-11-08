import type { RunMode } from './runControl'

export function isEditableShortcutTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false
  }
  const tag = target.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') {
    return true
  }
  return target.isContentEditable
}

export function modifierHeld(event: KeyboardEvent): boolean {
  return event.ctrlKey || event.metaKey
}

export function runModeForDigitKey(key: string): RunMode | null {
  if (key === '1') {
    return 'once'
  }
  if (key === '2') {
    return 'loop'
  }
  if (key === '3') {
    return 'from_node'
  }
  return null
}

export function isShortcutHelpKey(event: KeyboardEvent): boolean {
  if (isEditableShortcutTarget(event.target)) {
    return false
  }
  return event.key === '?' && !event.ctrlKey && !event.metaKey && !event.altKey
}

export function isGlobalSearchKey(event: KeyboardEvent): boolean {
  if (isEditableShortcutTarget(event.target)) {
    return false
  }
  return modifierHeld(event) && event.key.toLowerCase() === 'k'
}

export function isRunShortcut(event: KeyboardEvent): boolean {
  if (isEditableShortcutTarget(event.target)) {
    return false
  }
  return modifierHeld(event) && event.key === 'Enter' && !event.shiftKey
}

export function isRerunFailedShortcut(event: KeyboardEvent): boolean {
  if (isEditableShortcutTarget(event.target)) {
    return false
  }
  return modifierHeld(event) && event.key === 'Enter' && event.shiftKey
}

export function isFitCanvasKey(event: KeyboardEvent): boolean {
  if (isEditableShortcutTarget(event.target)) {
    return false
  }
  return event.key === 'f' || event.key === 'F'
}
