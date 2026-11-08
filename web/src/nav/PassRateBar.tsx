import { navTokens, passRateFillColor } from './tokens'

export interface PassRateBarProps {
  rate: number
}

export function PassRateBar({ rate }: PassRateBarProps) {
  const clamped = Math.max(0, Math.min(1, rate))
  const percent = Math.round(clamped * 100)
  return (
    <div
      className="nav-pass-rate"
      role="meter"
      aria-valuenow={percent}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-label={`通过率 ${String(percent)}%`}
      data-testid="pass-rate-bar"
      style={{ background: navTokens.passRateTrack }}
    >
      <div
        className="nav-pass-rate__fill"
        style={{
          width: `${String(percent)}%`,
          background: passRateFillColor(clamped),
        }}
      />
    </div>
  )
}
