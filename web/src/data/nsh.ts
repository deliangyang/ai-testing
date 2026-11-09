import type { DataSource } from './datasource'
import { HttpDataSource } from './http'

/**
 * Minimal TS mirror of `aitest/native` `DataSource` module used by the NSH shell.
 * In production the NSH toolchain supplies the real import; tests inject `client`.
 */
export interface NativeDataSourceClient {
  ensureApi(): Promise<string>
}

/**
 * Desktop `DataSource`: native module starts the embedded HTTP API, then reuses
 * [`HttpDataSource`] so all `web/` components share one contract (incl. SSE).
 */
export class NshDataSource implements DataSource {
  private readonly inner: Promise<HttpDataSource>

  constructor(client: NativeDataSourceClient) {
    this.inner = client.ensureApi().then((base) => new HttpDataSource(base))
  }

  private async ds(): Promise<HttpDataSource> {
    return this.inner
  }

  listDags(...args: Parameters<DataSource['listDags']>) {
    return this.ds().then((d) => d.listDags(...args))
  }

  listPendingAiDags(...args: Parameters<DataSource['listPendingAiDags']>) {
    return this.ds().then((d) => d.listPendingAiDags(...args))
  }

  getActiveDag(...args: Parameters<DataSource['getActiveDag']>) {
    return this.ds().then((d) => d.getActiveDag(...args))
  }

  getDagVersion(...args: Parameters<DataSource['getDagVersion']>) {
    return this.ds().then((d) => d.getDagVersion(...args))
  }

  listRuns(...args: Parameters<DataSource['listRuns']>) {
    return this.ds().then((d) => d.listRuns(...args))
  }

  getRun(...args: Parameters<DataSource['getRun']>) {
    return this.ds().then((d) => d.getRun(...args))
  }

  listExecutions(...args: Parameters<DataSource['listExecutions']>) {
    return this.ds().then((d) => d.listExecutions(...args))
  }

  subscribeRunEvents(
    runId: string,
    onEvent: Parameters<DataSource['subscribeRunEvents']>[1],
    options?: Parameters<DataSource['subscribeRunEvents']>[2],
  ): () => void {
    const ac = new AbortController()
    const merged = { ...options, signal: options?.signal ?? ac.signal }
    void this.ds().then((d) => d.subscribeRunEvents(runId, onEvent, merged))
    return () => {
      ac.abort()
    }
  }

  validateDag(...args: Parameters<DataSource['validateDag']>) {
    return this.ds().then((d) => d.validateDag(...args))
  }

  repairDagWithAi(...args: Parameters<DataSource['repairDagWithAi']>) {
    return this.ds().then((d) => d.repairDagWithAi(...args))
  }

  saveDag(...args: Parameters<DataSource['saveDag']>) {
    return this.ds().then((d) => d.saveDag(...args))
  }

  activateDag(...args: Parameters<DataSource['activateDag']>) {
    return this.ds().then((d) => d.activateDag(...args))
  }

  startRun(...args: Parameters<DataSource['startRun']>) {
    return this.ds().then((d) => d.startRun(...args))
  }

  cancelRun(...args: Parameters<DataSource['cancelRun']>) {
    return this.ds().then((d) => d.cancelRun(...args))
  }

  previewApiImport(...args: Parameters<DataSource['previewApiImport']>) {
    return this.ds().then((d) => d.previewApiImport(...args))
  }

  commitApiImport(...args: Parameters<DataSource['commitApiImport']>) {
    return this.ds().then((d) => d.commitApiImport(...args))
  }
}
