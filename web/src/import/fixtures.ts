import type { ApiImportPreviewResponse } from '../data/types'

/** Markdown 导入预览样例：一条高置信、一条低置信（行号来自设计 §6.1）。 */
export const MARKDOWN_IMPORT_PREVIEW: ApiImportPreviewResponse = {
  strategy: 'merge',
  items: [
    {
      spec: {
        id: 'post.orders',
        name: '创建订单',
        method: 'POST',
        path: '/orders',
        params: [],
      },
      confidence: 0.92,
      source_lines: [3, 4],
      merge_action: 'add',
    },
    {
      spec: {
        id: 'get.orders.id',
        name: '查询订单',
        method: 'GET',
        path: '/orders/{id}',
        params: [],
      },
      confidence: 0.72,
      source_lines: [8, 11],
      merge_action: 'add',
    },
  ],
  summary: { add: 2, update: 0, unchanged: 0, skip: 0 },
}
