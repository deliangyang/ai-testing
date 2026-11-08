import { useEffect } from 'react'
import { useReactFlow } from '@xyflow/react'

export function FocusNodeEffect({ nodeId }: { nodeId?: string | null }) {
  const { fitView } = useReactFlow()
  useEffect(() => {
    if (!nodeId) {
      return
    }
    const handle = window.requestAnimationFrame(() => {
      void fitView({ nodes: [{ id: nodeId }], padding: 0.35, duration: 200 })
    })
    return () => {
      window.cancelAnimationFrame(handle)
    }
  }, [nodeId, fitView])
  return null
}
