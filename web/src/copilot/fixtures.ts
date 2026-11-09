import type { CopilotChatRequest, CopilotChatResponse } from '../data/types'

/** 与 E2E / 服务端回放共用的触发话术。 */
export const COPILOT_FIXTURE_TRIGGER = '补健康检查'

export function copilotFixtureResponse(request: CopilotChatRequest): CopilotChatResponse {
  const trimmed = request.message.trim()
  if (trimmed !== COPILOT_FIXTURE_TRIGGER) {
    return {
      message: {
        role: 'assistant',
        content: '请说明要在当前 DAG 上做什么；试用话术「补健康检查」可查看示例工件绑定。',
      },
    }
  }
  const focus = request.context.selected_node_id ?? 'pay'
  return {
    message: {
      role: 'assistant',
      content: `可在 **create** 后插入 **health** 探活节点；当前上下文节点 **${focus}** 可定位；并为 **check** 追加 paid 断言。`,
      node_refs: ['create', 'health', focus, 'check'],
      actions: [
        {
          type: 'insert_canvas',
          node: { id: 'health', type: 'http', method: 'GET', path: '/health' },
          edges: [{ from: 'create', to: 'health' }],
        },
        { type: 'focus_node', node_id: 'pay' },
        {
          type: 'adopt_assertion',
          node_id: 'check',
          assertions: [
            {
              path: '$.body.paid',
              op: 'eq',
              expected: true as unknown as { [k: string]: unknown },
            },
          ],
        },
      ],
    },
  }
}
