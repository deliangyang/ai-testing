export type RunMode = 'once' | 'loop' | 'from_node'

export type RunPhase = 'idle' | 'running' | 'cancelling'

export interface EnvironmentOption {
  name: string
  reachable: boolean
}

export const FIXTURE_ENVIRONMENTS: EnvironmentOption[] = [
  { name: 'local', reachable: true },
  { name: 'dev', reachable: true },
  { name: 'stg', reachable: false },
]

export interface RunControlState {
  envName: string
  mode: RunMode
  phase: RunPhase
  canRerunFailed: boolean
}

export const initialRunControlState: RunControlState = {
  envName: 'local',
  mode: 'once',
  phase: 'idle',
  canRerunFailed: false,
}

export type RunControlAction =
  | { type: 'set_env'; envName: string }
  | { type: 'set_mode'; mode: RunMode }
  | { type: 'start_run' }
  | { type: 'stop_run' }
  | { type: 'finish_run'; failed: boolean }
  | { type: 'set_can_rerun_failed'; value: boolean }

export function runControlReducer(
  state: RunControlState,
  action: RunControlAction,
): RunControlState {
  switch (action.type) {
    case 'set_env':
      return { ...state, envName: action.envName }
    case 'set_mode':
      return { ...state, mode: action.mode }
    case 'start_run':
      return { ...state, phase: 'running', canRerunFailed: false }
    case 'stop_run':
      return { ...state, phase: 'cancelling' }
    case 'finish_run':
      return {
        ...state,
        phase: 'idle',
        canRerunFailed: action.failed,
      }
    case 'set_can_rerun_failed':
      return { ...state, canRerunFailed: action.value }
    default:
      return state
  }
}

export function environmentReachable(envName: string): boolean {
  return FIXTURE_ENVIRONMENTS.find((env) => env.name === envName)?.reachable ?? false
}
