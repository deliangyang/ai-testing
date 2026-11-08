import { render, screen } from '@testing-library/react'

import App from './App'

describe('App', () => {
  it('renders the shell layout', () => {
    render(<App />)
    expect(screen.getByTestId('app-shell')).toBeInTheDocument()
    expect(screen.getByRole('banner')).toHaveTextContent('AITest')
    expect(screen.getByTestId('center-canvas')).toBeInTheDocument()
  })
})
