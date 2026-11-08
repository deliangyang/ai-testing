import { useEffect, useRef } from 'react'

export type DagEditMenuAction =
  | { kind: 'add-http-node' }
  | { kind: 'delete-node'; nodeId: string }

export interface DagEditContextMenuProps {
  x: number
  y: number
  target: 'pane' | 'node'
  nodeId?: string
  onAction: (action: DagEditMenuAction) => void
  onClose: () => void
}

export function DagEditContextMenu({
  x,
  y,
  target,
  nodeId,
  onAction,
  onClose,
}: DagEditContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (ref.current?.contains(event.target as globalThis.Node)) {
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
      data-testid="dag-edit-context-menu"
      style={{ left: x, top: y }}
    >
      {target === 'pane' ? (
        <button
          type="button"
          role="menuitem"
          className="dag-run-ctx-menu__item"
          data-testid="dag-edit-add-http"
          onClick={() => {
            onAction({ kind: 'add-http-node' })
            onClose()
          }}
        >
          添加 HTTP 节点
        </button>
      ) : null}
      {target === 'node' && nodeId ? (
        <button
          type="button"
          role="menuitem"
          className="dag-run-ctx-menu__item"
          data-testid="dag-edit-delete-node"
          onClick={() => {
            onAction({ kind: 'delete-node', nodeId })
            onClose()
          }}
        >
          删除节点
        </button>
      ) : null}
    </div>
  )
}
