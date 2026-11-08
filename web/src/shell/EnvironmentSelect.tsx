import { environmentReachable, FIXTURE_ENVIRONMENTS } from './runControl'

export interface EnvironmentSelectProps {
  value: string
  onChange: (envName: string) => void
}

export function EnvironmentSelect({ value, onChange }: EnvironmentSelectProps) {
  const reachable = environmentReachable(value)
  return (
    <label className="top-bar__env" data-testid="env-select">
      <span
        className={`top-bar__env-dot${reachable ? ' top-bar__env-dot--ok' : ' top-bar__env-dot--bad'}`}
        aria-hidden
        data-testid="env-connectivity-dot"
      />
      <span className="top-bar__env-label">环境</span>
      <select
        value={value}
        aria-label="选择环境"
        data-testid="env-select-input"
        onChange={(event) => {
          onChange(event.target.value)
        }}
      >
        {FIXTURE_ENVIRONMENTS.map((env) => (
          <option key={env.name} value={env.name}>
            {env.name}
          </option>
        ))}
      </select>
    </label>
  )
}
