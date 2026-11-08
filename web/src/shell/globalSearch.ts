import type { NavCatalog } from '../nav/types'

export type GlobalSearchKind = 'scenario' | 'report'

export interface GlobalSearchItem {
  kind: GlobalSearchKind
  refId: string
  title: string
  subtitle: string
}

export function buildGlobalSearchItems(catalog: NavCatalog): GlobalSearchItem[] {
  const scenarios = catalog.scenarios.map((item) => ({
    kind: 'scenario' as const,
    refId: item.id,
    title: item.title,
    subtitle: '场景',
  }))
  const reports = catalog.reports.map((item) => ({
    kind: 'report' as const,
    refId: item.id,
    title: item.title,
    subtitle: '报告',
  }))
  return [...scenarios, ...reports]
}

export function filterGlobalSearchItems(
  items: GlobalSearchItem[],
  query: string,
): GlobalSearchItem[] {
  const q = query.trim().toLowerCase()
  if (!q) {
    return items
  }
  return items.filter(
    (item) =>
      item.title.toLowerCase().includes(q) ||
      item.refId.toLowerCase().includes(q) ||
      item.subtitle.toLowerCase().includes(q),
  )
}
