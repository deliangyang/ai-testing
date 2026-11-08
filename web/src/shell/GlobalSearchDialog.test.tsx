import { fireEvent, render, screen } from '@testing-library/react'

import { FIXTURE_NAV_CATALOG } from '../nav'
import { GlobalSearchDialog } from './GlobalSearchDialog'

describe('GlobalSearchDialog', () => {
  it('filters results and picks a scenario', () => {
    const onPick = vi.fn()
    const onClose = vi.fn()
    render(
      <GlobalSearchDialog open catalog={FIXTURE_NAV_CATALOG} onClose={onClose} onPick={onPick} />,
    )
    fireEvent.change(screen.getByTestId('global-search-input'), {
      target: { value: 'serial' },
    })
    const hits = screen.getAllByRole('button', { name: /serial/i })
    fireEvent.click(hits[0])
    expect(onPick).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'scenario', refId: 'serial' }),
    )
    expect(onClose).toHaveBeenCalled()
  })
})
