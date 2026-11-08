import { fireEvent, render, screen } from '@testing-library/react'

import { ShortcutHelpDialog } from './ShortcutHelpDialog'
import { SHORTCUT_REFERENCE } from './shortcutReference'

describe('ShortcutHelpDialog', () => {
  it('lists develop shortcuts and closes', () => {
    const onClose = vi.fn()
    render(<ShortcutHelpDialog open onClose={onClose} />)
    expect(screen.getByText(SHORTCUT_REFERENCE[0].description)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))
    expect(onClose).toHaveBeenCalled()
  })
})
