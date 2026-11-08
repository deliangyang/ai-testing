import type { Node } from '../types/generated'

const TEMPLATE_PREFIX = '${'

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function walkStrings(value: unknown, out: string[]): void {
  if (typeof value === 'string') {
    if (value.includes(TEMPLATE_PREFIX)) {
      out.push(value)
    }
    return
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      walkStrings(item, out)
    }
    return
  }
  if (value !== null && typeof value === 'object') {
    for (const v of Object.values(value)) {
      walkStrings(v, out)
    }
  }
}

export function collectTemplateStrings(node: Node): string[] {
  const out: string[] = []
  walkStrings(node, out)
  return out
}

export function nodeDisplayName(node: Node): string {
  return node.name ?? node.id
}

export function nodeTypeIcon(type: Node['type']): string {
  switch (type) {
    case 'http':
      return 'HTTP'
    case 'poll':
      return 'POLL'
    case 'assert':
      return 'ASSERT'
    case 'db_query':
      return 'DB'
    case 'subworkflow':
      return 'SUB'
    case 'cleanup':
      return 'CLR'
    default:
      return '?'
  }
}

export function nodeSummary(node: Node): string {
  switch (node.type) {
    case 'http':
      return `${node.method} ${node.path}`
    case 'poll':
      return `poll ${node.request.method} ${node.request.path}`
    case 'assert':
      return `assert → ${node.target}`
    case 'db_query':
      return node.sql.trim().split(/\s+/).slice(0, 6).join(' ')
    case 'subworkflow':
      return `sub ${node.dag.id}`
    case 'cleanup':
      return `cleanup ×${String(node.requests?.length ?? 0)}`
  }
}

/**
 * Short label for a dependency edge when the downstream node references upstream output.
 */
export function edgeVariableLabel(fromId: string, toNode: Node): string | undefined {
  const re = new RegExp(`\\$\\{${escapeRegExp(fromId)}\\.output([^}]*)\\}`)
  for (const text of collectTemplateStrings(toNode)) {
    const match = re.exec(text)
    if (match) {
      const suffix = match[1]
      return suffix.length > 0 ? `output${suffix}` : 'output'
    }
  }
  return undefined
}
