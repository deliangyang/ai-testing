export interface TemplateSegment {
  kind: 'text' | 'var'
  text: string
}

const VAR_PATTERN = /\$\{[^}]+\}/g

export function splitTemplateSegments(value: string): TemplateSegment[] {
  if (!value) {
    return []
  }
  const segments: TemplateSegment[] = []
  let last = 0
  for (const match of value.matchAll(VAR_PATTERN)) {
    const index = match.index
    if (index > last) {
      segments.push({ kind: 'text', text: value.slice(last, index) })
    }
    segments.push({ kind: 'var', text: match[0] })
    last = index + match[0].length
  }
  if (last < value.length) {
    segments.push({ kind: 'text', text: value.slice(last) })
  }
  return segments
}
