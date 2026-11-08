import { fireEvent, render, screen } from '@testing-library/react'

import { WorkspacePresetSelect } from './WorkspacePresetSelect'

describe('WorkspacePresetSelect', () => {
  it('fires onSelect with debugger layout dimensions', () => {
    const onSelect = vi.fn()
    render(<WorkspacePresetSelect onSelect={onSelect} />)
    const select = screen.getByTestId('workspace-preset-select')
    fireEvent.change(select, { target: { value: 'debugger' } })
    expect(onSelect).toHaveBeenCalledWith('debugger')
  })

  it('lists all three seeded presets', () => {
    render(<WorkspacePresetSelect onSelect={() => {}} />)
    expect(screen.getByRole('option', { name: '编排者' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: '执行调试者' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: '看报告者' })).toBeInTheDocument()
  })
})
