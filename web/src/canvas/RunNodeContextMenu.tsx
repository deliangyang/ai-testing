import { useEffect, useRef } from 'react'

export interface RunNodeContextMenuProps {
  nodeId: string
  x: number
  y: number
  onRerun: (nodeId: string) => void
  onClose: () => void
}

export function RunNodeContextMenu({ nodeId, x, y, onRerun, onClose }: RunNodeContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (ref.current?.contains(event.target as Node)) {
        return
      }
      onClose()
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose()
      }
    }
    window.addEventListener('pointerdown', onPointerDown, true)
    window.addEventListener('keydown', onKeyDown)
    return () => {
      window.removeEventListener('pointerdown', onPointerDown, true)
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [onClose])

  return (
    <div
      ref={ref}
      className="dag-run-ctx-menu"
      role="menu"
      data-testid="run-node-context-menu"
      style={{ left: x, top: y }}
    >
      <button
        type="button"
        role="menuitem"
        className="dag-run-ctx-menu__item"
        data-testid="run-node-ctx-rerun"
        onClick={() => {
          onRerun(nodeId)
          onClose()
        }}
      >
        从此节点重跑
      </button>
    </div>
  )
}
