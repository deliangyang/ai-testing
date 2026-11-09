import { useCallback, useState } from 'react'

import type { DataSource } from '../data/datasource'
import type { CopilotAction, CopilotChatMessage } from '../data/types'
import type { Dag } from '../types/generated'
import { COPILOT_FIXTURE_TRIGGER } from './fixtures'
import { applyCopilotAction } from './applyActions'
import './copilot.css'

export interface CopilotPanelProps {
  dag: Dag
  selectedNodeId: string | null
  dataSource: DataSource
  onDagChange: (dag: Dag) => void
  onFocusNode: (nodeId: string) => void
  onSelectNode: (nodeId: string) => void
}

function actionLabel(action: CopilotAction): string {
  switch (action.type) {
    case 'insert_canvas':
      return `插入画布：HTTP 节点「${action.node.id}」`
    case 'focus_node':
      return `定位节点「${action.node_id}」`
    case 'adopt_assertion':
      return `采纳断言：节点「${action.node_id}」+${String(action.assertions.length)} 条`
    default:
      return '未知动作'
  }
}

export function CopilotPanel({
  dag,
  selectedNodeId,
  dataSource,
  onDagChange,
  onFocusNode,
  onSelectNode,
}: CopilotPanelProps) {
  const [messages, setMessages] = useState<CopilotChatMessage[]>([])
  const [draft, setDraft] = useState(COPILOT_FIXTURE_TRIGGER)
  const [sending, setSending] = useState(false)
  const [applied, setApplied] = useState<Set<string>>(new Set())

  const send = useCallback(async () => {
    const text = draft.trim()
    if (!text || sending) {
      return
    }
    const userMsg: CopilotChatMessage = { role: 'user', content: text }
    setMessages((prev) => [...prev, userMsg])
    setDraft('')
    setSending(true)
    try {
      const response = await dataSource.copilotChat({
        context: {
          dag_id: dag.id,
          selected_node_id: selectedNodeId,
        },
        message: text,
        history: [...messages, userMsg],
      })
      setMessages((prev) => [...prev, response.message])
    } finally {
      setSending(false)
    }
  }, [dag.id, dataSource, draft, messages, selectedNodeId, sending])

  const runAction = useCallback(
    (messageIndex: number, actionIndex: number, action: CopilotAction) => {
      const key = `${String(messageIndex)}:${String(actionIndex)}`
      if (applied.has(key)) {
        return
      }
      const result = applyCopilotAction(dag, action)
      onDagChange(result.dag)
      if (result.focusNodeId) {
        onFocusNode(result.focusNodeId)
      }
      if (result.selectedNodeId) {
        onSelectNode(result.selectedNodeId)
      }
      setApplied((prev) => new Set(prev).add(key))
    },
    [applied, dag, onDagChange, onFocusNode, onSelectNode],
  )

  return (
    <div className="copilot-panel" data-testid="copilot-panel">
      <div className="copilot-panel__context" data-testid="copilot-context-strip">
        上下文：<strong data-testid="copilot-context-dag">{dag.id}</strong>
        {' · '}
        选中节点：
        <strong data-testid="copilot-context-node">{selectedNodeId ?? '（无）'}</strong>
      </div>

      <div className="copilot-panel__thread" data-testid="copilot-thread">
        {messages.length === 0 ? (
          <p className="copilot-bubble copilot-bubble--assistant">
            输入消息开始对话；E2E 使用「{COPILOT_FIXTURE_TRIGGER}」。
          </p>
        ) : null}
        {messages.map((msg, mi) => (
          <CopilotMessageBubble
            key={`${msg.role}-${String(mi)}-${msg.content.slice(0, 24)}`}
            msg={msg}
            mi={mi}
            applied={applied}
            onNodeRef={onFocusNode}
            onAction={runAction}
          />
        ))}
      </div>

      <form
        className="copilot-panel__composer"
        onSubmit={(e) => {
          e.preventDefault()
          void send()
        }}
      >
        <textarea
          data-testid="copilot-input"
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value)
          }}
          placeholder="向 AI 副驾提问…"
          disabled={sending}
        />
        <button type="submit" data-testid="copilot-send" disabled={sending || !draft.trim()}>
          {sending ? '发送中…' : '发送'}
        </button>
      </form>
    </div>
  )
}

function CopilotMessageBubble(props: {
  msg: CopilotChatMessage
  mi: number
  applied: Set<string>
  onNodeRef: (nodeId: string) => void
  onAction: (messageIndex: number, actionIndex: number, action: CopilotAction) => void
}) {
  const { msg, mi, applied, onNodeRef, onAction } = props
  const bubbleClass =
    msg.role === 'user'
      ? 'copilot-bubble copilot-bubble--user'
      : 'copilot-bubble copilot-bubble--assistant'
  return (
    <div className={bubbleClass} data-testid={`copilot-msg-${msg.role}-${String(mi)}`}>
      <div>{msg.content}</div>
      {msg.node_refs?.length ? (
        <div className="copilot-bubble__refs">
          {msg.node_refs.map((nodeId) => (
            <button
              key={nodeId}
              type="button"
              className="copilot-node-ref"
              data-testid={`copilot-node-ref-${nodeId}`}
              onClick={() => {
                onNodeRef(nodeId)
              }}
            >
              {nodeId}
            </button>
          ))}
        </div>
      ) : null}
      {msg.actions?.length ? (
        <div className="copilot-bubble__actions">
          {msg.actions.map((action, ai) => {
            const key = `${String(mi)}:${String(ai)}`
            const done = applied.has(key)
            return (
              <button
                key={key}
                type="button"
                className="copilot-action-btn"
                data-testid={`copilot-action-${action.type}-${String(ai)}`}
                disabled={done}
                onClick={() => {
                  onAction(mi, ai, action)
                }}
              >
                {done ? '已应用 · ' : ''}
                {actionLabel(action)}
              </button>
            )
          })}
        </div>
      ) : null}
    </div>
  )
}
