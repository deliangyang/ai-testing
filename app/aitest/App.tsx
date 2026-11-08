/**
 * NSH mixed shell: Rust `DataSource` starts the embedded API; WebView loads `web/dist`.
 */
import { mount, View, Text } from 'nsh'
import { DataSource, WebUiHost } from 'aitest/native'

function Host() {
  const base = DataSource.ensureApi()
  const shellUrl = DataSource.web_shell_url()
  return (
    <View class="flex-1 bg-background" data-testid="aitest-host">
      <View class="px-3 py-2">
        <Text data-testid="api-base" class="text-muted text-xs">
          {base}
        </Text>
      </View>
      <WebUiHost src={shellUrl} />
    </View>
  )
}

mount(Host)
