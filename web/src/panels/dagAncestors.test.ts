import { describe, expect, it } from 'vitest'

import { ASYNC_POLL_DAG } from '../data/fixtures/asyncPoll'
import { SERIAL_DAG } from '../data/fixtures/serial'
import { ancestorIds, upstreamVariableSuggestions } from './dagAncestors'

describe('dagAncestors', () => {
  it('lists transitive ancestors', () => {
    expect(ancestorIds(SERIAL_DAG, 'check')).toEqual(['create', 'pay'])
  })

  it('excludes siblings and the node itself', () => {
    expect(ancestorIds(ASYNC_POLL_DAG, 'wait_paid')).toEqual(['create', 'pay'])
    expect(upstreamVariableSuggestions(ASYNC_POLL_DAG, 'wait_paid')).toContain(
      '${create.output.body}',
    )
    expect(upstreamVariableSuggestions(ASYNC_POLL_DAG, 'check')).toContain('${pay.output.body}')
    expect(upstreamVariableSuggestions(ASYNC_POLL_DAG, 'check')).not.toContain(
      '${wait_paid.output.body}',
    )
  })
})
