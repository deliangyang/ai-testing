import { SHORTCUT_REFERENCE } from './shortcutReference'

export interface ShortcutHelpDialogProps {
  open: boolean
  onClose: () => void
}

export function ShortcutHelpDialog({ open, onClose }: ShortcutHelpDialogProps) {
  if (!open) {
    return null
  }

  return (
    <div
      className="shell-dialog-backdrop"
      data-testid="shortcut-help-backdrop"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="shell-dialog shortcut-help-dialog"
        data-testid="shortcut-help-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="快捷键"
        onClick={(event) => {
          event.stopPropagation()
        }}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            onClose()
          }
        }}
      >
        <h2 className="shortcut-help-dialog__title">快捷键</h2>
        <table className="shortcut-help-dialog__table">
          <tbody>
            {SHORTCUT_REFERENCE.map((row) => (
              <tr key={row.keys}>
                <th scope="row">{row.keys}</th>
                <td>{row.description}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <button type="button" className="shortcut-help-dialog__close" onClick={onClose}>
          关闭
        </button>
      </div>
    </div>
  )
}
