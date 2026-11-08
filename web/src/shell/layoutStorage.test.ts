import { afterEach, describe, expect, it } from 'vitest'

import { DEFAULT_SHELL_LAYOUT, workspaceScopeKey } from './shellLayout'
import { loadShellLayout, saveShellLayout } from './layoutStorage'

describe('layoutStorage', () => {
  afterEach(() => {
    localStorage.clear()
  })

  it('round-trips layout per scope', () => {
    const key = workspaceScopeKey('scenario', 'async-poll')
    saveShellLayout(key, {
      ...DEFAULT_SHELL_LAYOUT,
      right_width_px: 280,
      drawer_collapsed: true,
    })
    const loaded = loadShellLayout(key)
    expect(loaded?.right_width_px).toBe(280)
    expect(loaded?.drawer_collapsed).toBe(true)
  })
})
