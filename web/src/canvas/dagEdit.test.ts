import { describe, expect, it } from 'vitest'

import { SERIAL_DAG } from '../data/fixtures/serial'
import { addEdge, addHttpNode, edgeCount, removeNode, removeEdge as dropEdge } from './dagEdit'

describe('dagEdit', () => {
  it('adds a sorted http node with a unique id', () => {
    const next = addHttpNode(SERIAL_DAG)
    expect(next.nodes.map((n) => n.id)).toEqual(['check', 'create', 'node_1', 'pay'])
    expect(next.nodes.find((n) => n.id === 'node_1')?.type).toBe('http')
  })

  it('removes a node and incident edges', () => {
    const next = removeNode(SERIAL_DAG, 'pay')
    expect(next.nodes.map((n) => n.id)).toEqual(['check', 'create'])
    expect(edgeCount(next)).toBe(0)
  })

  it('adds and removes edges without duplicates', () => {
    const linked = addEdge(SERIAL_DAG, 'create', 'check')
    expect(edgeCount(linked)).toBe(3)
    const same = addEdge(linked, 'create', 'check')
    expect(edgeCount(same)).toBe(3)
    const trimmed = dropEdge(same, 'create', 'check')
    expect(edgeCount(trimmed)).toBe(2)
  })
})
