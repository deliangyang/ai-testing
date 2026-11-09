import { FixtureDataSource } from '../data/fixture'
import { SERIAL_AI_PENDING_DETAIL } from './fixtures'
import { PendingAiReviewPanel } from './PendingAiReviewPanel'

const dataSource = new FixtureDataSource('serial')

export function PendingAiWorkbench() {
  return (
    <main>
      <PendingAiReviewPanel dataSource={dataSource} initialPending={SERIAL_AI_PENDING_DETAIL} />
    </main>
  )
}
