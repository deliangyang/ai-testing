import { ApiImportWorkbench } from './import'
import { PendingAiWorkbench } from './pending'
import { AppShell, DagEditWorkbench } from './shell'

function App() {
  if (typeof window !== 'undefined') {
    const params = new URLSearchParams(window.location.search)
    if (params.get('import') === '1') {
      return <ApiImportWorkbench />
    }
    if (params.get('pending') === '1') {
      return <PendingAiWorkbench />
    }
    if (params.get('edit') === '1') {
      return <DagEditWorkbench />
    }
  }
  return <AppShell />
}

export default App
