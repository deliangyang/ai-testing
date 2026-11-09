import { ApiImportWorkbench } from './import'
import { AppShell, DagEditWorkbench } from './shell'

function App() {
  if (typeof window !== 'undefined') {
    const params = new URLSearchParams(window.location.search)
    if (params.get('import') === '1') {
      return <ApiImportWorkbench />
    }
    if (params.get('edit') === '1') {
      return <DagEditWorkbench />
    }
  }
  return <AppShell />
}

export default App
