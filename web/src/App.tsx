import { AppShell, DagEditWorkbench } from './shell'

function App() {
  if (
    typeof window !== 'undefined' &&
    new URLSearchParams(window.location.search).get('edit') === '1'
  ) {
    return <DagEditWorkbench />
  }
  return <AppShell />
}

export default App
