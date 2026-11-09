import { FixtureDataSource } from '../data/fixture'
import { MARKDOWN_IMPORT_PREVIEW } from './fixtures'
import { ApiImportPanel } from './ApiImportPanel'

const dataSource = new FixtureDataSource('serial')

export function ApiImportWorkbench() {
  return (
    <main data-testid="api-import-workbench">
      <h1 className="visually-hidden">接口导入</h1>
      <ApiImportPanel dataSource={dataSource} initialPreview={MARKDOWN_IMPORT_PREVIEW} />
    </main>
  )
}
