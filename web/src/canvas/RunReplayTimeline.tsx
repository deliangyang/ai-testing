export interface RunReplayTimelineProps {
  maxSeq: number
  value: number
  onChange: (seq: number) => void
}

export function RunReplayTimeline({ maxSeq, value, onChange }: RunReplayTimelineProps) {
  if (maxSeq <= 0) {
    return null
  }
  return (
    <div className="run-replay-timeline" data-testid="run-replay-timeline">
      <label className="run-replay-timeline__label" htmlFor="run-replay-seq">
        时间轴
      </label>
      <input
        id="run-replay-seq"
        type="range"
        min={0}
        max={maxSeq}
        value={value}
        data-testid="run-replay-seq"
        onChange={(e) => {
          onChange(Number(e.target.value))
        }}
      />
      <span className="run-replay-timeline__value" data-testid="run-replay-seq-label">
        {value}/{maxSeq}
      </span>
    </div>
  )
}
