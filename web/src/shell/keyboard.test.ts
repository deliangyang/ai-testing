import { describe, expect, it } from 'vitest'

import { isGlobalSearchKey, isRunShortcut, isShortcutHelpKey, runModeForDigitKey } from './keyboard'

function keyEvent(init: KeyboardEventInit & { target?: EventTarget | null }): KeyboardEvent {
  const event = new KeyboardEvent('keydown', init)
  Object.defineProperty(event, 'target', { value: init.target ?? document.body })
  return event
}

describe('keyboard shortcuts', () => {
  it('detects Ctrl/Cmd+K', () => {
    expect(isGlobalSearchKey(keyEvent({ key: 'k', ctrlKey: true }))).toBe(true)
    expect(isGlobalSearchKey(keyEvent({ key: 'k', metaKey: true }))).toBe(true)
    expect(isGlobalSearchKey(keyEvent({ key: 'k' }))).toBe(false)
  })

  it('ignores shortcuts while typing in inputs', () => {
    const input = document.createElement('input')
    expect(isGlobalSearchKey(keyEvent({ key: 'k', ctrlKey: true, target: input }))).toBe(false)
  })

  it('detects help key', () => {
    expect(isShortcutHelpKey(keyEvent({ key: '?' }))).toBe(true)
    expect(isShortcutHelpKey(keyEvent({ key: '/', shiftKey: true }))).toBe(false)
  })

  it('maps digit keys to run modes', () => {
    expect(runModeForDigitKey('1')).toBe('once')
    expect(runModeForDigitKey('2')).toBe('loop')
    expect(runModeForDigitKey('3')).toBe('from_node')
  })

  it('detects run shortcut', () => {
    expect(isRunShortcut(keyEvent({ key: 'Enter', ctrlKey: true }))).toBe(true)
    expect(isRunShortcut(keyEvent({ key: 'Enter', ctrlKey: true, shiftKey: true }))).toBe(false)
  })
})
