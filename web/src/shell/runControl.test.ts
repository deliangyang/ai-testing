import { initialRunControlState, runControlReducer } from './runControl'

describe('runControlReducer', () => {
  it('transitions through run and stop', () => {
    let state = runControlReducer(initialRunControlState, { type: 'start_run' })
    expect(state.phase).toBe('running')
    state = runControlReducer(state, { type: 'stop_run' })
    expect(state.phase).toBe('cancelling')
    state = runControlReducer(state, { type: 'finish_run', failed: false })
    expect(state.phase).toBe('idle')
  })
})
