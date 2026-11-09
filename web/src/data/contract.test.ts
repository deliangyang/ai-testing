import { SCENARIOS } from './fixtures'
import {
  SERIAL_DAG_ACTIVE,
  SERIAL_DAG_VERSIONS,
  SERIAL_EXECUTIONS,
  SERIAL_RUN,
} from './fixtures/serial'
import { FixtureDataSource } from './fixture'
import { HttpDataSource } from './http'
import { NshDataSource } from './nsh'
import { copilotFixtureResponse } from '../copilot/fixtures'
import { exerciseDataSourceContract } from './contract'
import type { CopilotChatRequest } from './types'
import type { Dag } from '../types/generated'

function readDagFromInit(init?: RequestInit): Dag | undefined {
  if (init === undefined || init.body === undefined || typeof init.body !== 'string') {
    return undefined
  }
  return JSON.parse(init.body) as Dag
}

function jsonResponse(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

function installHttpMock(): void {
  const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url
    const { pathname } = new URL(url)
    if (pathname === '/api/dags' && (init === undefined || init.method === undefined)) {
      return Promise.resolve(jsonResponse(SERIAL_DAG_VERSIONS))
    }
    if (pathname === '/api/dags/pending') {
      return Promise.resolve(jsonResponse([]))
    }
    if (pathname === '/api/dags/serial/active') {
      return Promise.resolve(jsonResponse(SERIAL_DAG_ACTIVE))
    }
    if (pathname === '/api/dags/serial/versions/1') {
      return Promise.resolve(jsonResponse(SERIAL_DAG_ACTIVE))
    }
    if (pathname === '/api/runs') {
      return Promise.resolve(jsonResponse([SERIAL_RUN]))
    }
    if (pathname === `/api/runs/${SERIAL_RUN.id}`) {
      return Promise.resolve(jsonResponse(SERIAL_RUN))
    }
    if (pathname === `/api/runs/${SERIAL_RUN.id}/executions`) {
      return Promise.resolve(jsonResponse(SERIAL_EXECUTIONS))
    }
    if (pathname === '/api/dags/validate' && init?.method === 'POST') {
      const body = readDagFromInit(init)
      const pay = body?.nodes.find((n) => n.id === 'pay')
      if (pay?.type === 'http' && pay.path.includes('missing')) {
        return Promise.resolve(
          jsonResponse({
            passed: false,
            issues: [
              {
                code: 'template.unknown_node',
                severity: 'error',
                node_id: 'pay',
                message: 'missing upstream',
              },
            ],
          }),
        )
      }
      return Promise.resolve(jsonResponse({ passed: true, issues: [] }))
    }
    if (pathname === '/api/dags/repair-ai' && init?.method === 'POST') {
      const body = readDagFromInit(init)
      const pay = body?.nodes.find((n) => n.id === 'pay')
      if (pay?.type === 'http') {
        pay.path = '/orders/${create.output.body.id}/pay'
      }
      return Promise.resolve(
        jsonResponse({
          status: 'accepted',
          dag: body ?? SERIAL_DAG_ACTIVE.dag,
          gate: { passed: true, issues: [] },
          pending: {
            id: 'serial',
            version: 2,
            status: 'pending',
            created_by: 'ai',
            created_at: '2026-11-09T00:00:00.000Z',
          },
        }),
      )
    }
    if (pathname === '/api/api-specs/import/preview' && init?.method === 'POST') {
      return Promise.resolve(
        jsonResponse({
          strategy: 'merge',
          items: [],
          summary: { add: 0, update: 0, unchanged: 0, skip: 0 },
        }),
      )
    }
    if (pathname === '/api/api-specs/import/commit' && init?.method === 'POST') {
      return Promise.resolve(jsonResponse({ added: 0, updated: 0, unchanged: 0, saved_ids: [] }))
    }
    if (pathname === '/api/copilot/chat' && init !== undefined && init.method === 'POST') {
      const body = init.body
      const request =
        typeof body === 'string' ? (JSON.parse(body) as CopilotChatRequest) : undefined
      if (request === undefined) {
        return Promise.resolve(jsonResponse({ error: 'bad body' }, 400))
      }
      return Promise.resolve(jsonResponse(copilotFixtureResponse(request)))
    }
    return Promise.resolve(jsonResponse({ error: `unexpected ${pathname}` }, 404))
  })
  vi.stubGlobal('fetch', fetchMock)
}

class MockEventSource {
  static instances: MockEventSource[] = []
  url: string
  onerror: (() => void) | null = null
  private readonly handlers = new Map<string, (ev: MessageEvent<string>) => void>()

  constructor(url: string) {
    this.url = url
    MockEventSource.instances.push(this)
    queueMicrotask(() => {
      this.replay()
    })
  }

  addEventListener(type: string, listener: EventListener): void {
    this.handlers.set(type, (ev: MessageEvent<string>) => {
      listener(ev)
    })
  }

  close(): void {
    // noop
  }

  private replay(): void {
    if (!this.url.includes(SERIAL_RUN.id)) {
      this.onerror?.()
      return
    }
    const events = SCENARIOS.serial.eventsByRun[SERIAL_RUN.id] ?? []
    for (const item of events) {
      const name = item.event.type
      const handler = this.handlers.get(name)
      if (handler === undefined) {
        continue
      }
      const message = {
        data: JSON.stringify(item.event),
        lastEventId: String(item.seq),
      } as MessageEvent<string>
      handler(message)
    }
  }
}

describe('DataSource contract', () => {
  it('FixtureDataSource', async () => {
    await exerciseDataSourceContract(() => new FixtureDataSource('serial'))
  })

  it('HttpDataSource', async () => {
    installHttpMock()
    MockEventSource.instances = []
    vi.stubGlobal('EventSource', MockEventSource)
    await exerciseDataSourceContract(() => new HttpDataSource('http://127.0.0.1:8790'))
    expect(MockEventSource.instances.length).toBeGreaterThan(0)
    vi.unstubAllGlobals()
  })

  it('NshDataSource', async () => {
    installHttpMock()
    MockEventSource.instances = []
    vi.stubGlobal('EventSource', MockEventSource)
    await exerciseDataSourceContract(
      () =>
        new NshDataSource({
          ensureApi: () => Promise.resolve('http://127.0.0.1:8790'),
        }),
    )
    vi.unstubAllGlobals()
  })
})
