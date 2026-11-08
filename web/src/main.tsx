import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import { installInjectedApiBase } from './host/apiBase'
import App from './App.tsx'

installInjectedApiBase()

const root = document.getElementById('root')
if (!root) {
  throw new Error('missing #root element')
}
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
