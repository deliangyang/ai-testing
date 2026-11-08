import { describe, expect, it } from 'vitest'

import { FIXTURE_NAV_CATALOG } from '../nav'
import { buildGlobalSearchItems, filterGlobalSearchItems } from './globalSearch'

describe('globalSearch', () => {
  it('indexes scenarios and reports', () => {
    const items = buildGlobalSearchItems(FIXTURE_NAV_CATALOG)
    expect(items.some((i) => i.kind === 'scenario' && i.refId === 'serial')).toBe(true)
    expect(items.some((i) => i.kind === 'report')).toBe(true)
  })

  it('filters by title or id', () => {
    const items = buildGlobalSearchItems(FIXTURE_NAV_CATALOG)
    const hits = filterGlobalSearchItems(items, 'async')
    expect(hits.every((h) => h.refId.includes('async') || h.title.includes('async'))).toBe(true)
  })
})
