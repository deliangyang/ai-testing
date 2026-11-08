/**
 * NSH headless tests (`nsh test app/aitest`).
 */
import { mount, signal, Text, View } from 'nsh'
import { DataSource } from 'aitest/native'

function ContractSmoke() {
  const status = signal('pending')
  const base = DataSource.ensureApi()
  const payload = DataSource.list_dags()
  status.set(payload.startsWith('[') ? 'ok' : 'bad-json')
  return (
    <View class="flex-1 items-center justify-center bg-background" data-testid="nsh-contract">
      <Text data-testid="api-base">{base}</Text>
      <Text data-testid="contract-status">{status()}</Text>
    </View>
  )
}

mount(ContractSmoke)
