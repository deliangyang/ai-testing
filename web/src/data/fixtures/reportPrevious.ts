import type { RunDto, StoredRunEvent } from '../types'
import { SERIAL_EVENTS, SERIAL_RUN } from './serial'

/** Baseline run for summary comparison in the serial report workspace. */
const SERIAL_PREVIOUS_RUN: RunDto = {
  ...SERIAL_RUN,
  id: 'run-fixture-serial-prev',
  started_at: '2026-11-08T13:59:00.000+08:00',
  finished_at: '2026-11-08T13:59:03.500+08:00',
}

const SERIAL_PREVIOUS_EVENTS: StoredRunEvent[] = SERIAL_EVENTS.filter((e) => e.seq <= 5)

export function reportPreviousFor(reportId: string):
  | {
      run: RunDto
      events: StoredRunEvent[]
    }
  | undefined {
  if (reportId === SERIAL_RUN.id) {
    return { run: SERIAL_PREVIOUS_RUN, events: SERIAL_PREVIOUS_EVENTS }
  }
  return undefined
}
