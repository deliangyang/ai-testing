import { useEffect } from 'react'

import {
  isFitCanvasKey,
  isGlobalSearchKey,
  isRerunFailedShortcut,
  isRunShortcut,
  isShortcutHelpKey,
  modifierHeld,
  runModeForDigitKey,
} from './keyboard'
import type { RunMode } from './runControl'

export function useAppShellShortcuts(options: {
  openSearch: () => void
  openHelp: () => void
  onRun: () => void
  onRerunFailed: () => void
  onModeChange: (mode: RunMode) => void
  onFitCanvas: () => void
  canRerunFailed: boolean
  runBlocked: boolean
  anyDialogOpen: boolean
}): void {
  const {
    openSearch,
    openHelp,
    onRun,
    onRerunFailed,
    onModeChange,
    onFitCanvas,
    canRerunFailed,
    runBlocked,
    anyDialogOpen,
  } = options

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (anyDialogOpen) {
        return
      }

      if (isGlobalSearchKey(event)) {
        event.preventDefault()
        openSearch()
        return
      }

      if (isShortcutHelpKey(event)) {
        event.preventDefault()
        openHelp()
        return
      }

      if (isRunShortcut(event)) {
        if (!runBlocked) {
          event.preventDefault()
          onRun()
        }
        return
      }

      if (isRerunFailedShortcut(event)) {
        if (canRerunFailed && !runBlocked) {
          event.preventDefault()
          onRerunFailed()
        }
        return
      }

      if (isFitCanvasKey(event)) {
        event.preventDefault()
        onFitCanvas()
        return
      }

      if (!modifierHeld(event) && !event.altKey) {
        const mode = runModeForDigitKey(event.key)
        if (mode && !runBlocked) {
          event.preventDefault()
          onModeChange(mode)
        }
      }
    }

    window.addEventListener('keydown', onKeyDown)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [
    anyDialogOpen,
    canRerunFailed,
    onFitCanvas,
    onModeChange,
    onRerunFailed,
    onRun,
    openHelp,
    openSearch,
    runBlocked,
  ])
}
