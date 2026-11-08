/**
 * NSH headless test (run with `nsh test app/aitest` once the mixed project is initialized).
 *
 * Until `nsh init` merges Cargo/nsh.json, this file documents the intended contract:
 * open the shell entry and assert the host mounted.
 */
import { mount, signal, Text, View } from 'nsh'

function SpikeHost() {
  const label = signal('aitest spike')
  return (
    <View class="flex-1 items-center justify-center bg-background">
      <Text data-testid="spike-label" class="text-foreground">
        {label()}
      </Text>
    </View>
  )
}

mount(SpikeHost)
