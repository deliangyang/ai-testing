# 失败分类与错误码

每个 `FAILED` 节点都带有 `failure_category`（7 类，来自 `develop.md` §4.5）和 `error_code`。两者都由 `crates/classifier` 中的确定性规则产出，不调用 LLM，并持久化到 `node_executions` 和 `failure_records`。自主性策略按分类决定后续动作：重试、调 LLM 修复，或转人工。

约定：

- 名称一经发布就不再修改，只能新增。分类用 snake_case，错误码用 SCREAMING_SNAKE_CASE。
- 每个错误码只属于一个分类。无法归类时使用 `unknown` / `UNKNOWN`。
- 配置问题（缺环境变量、凭据不可用、模板错误）和引擎内部问题归入 `unknown`，因为它们需要人工处理，不能自动重试，也不该交给 LLM 修复。
- 下面的表由 `classifier::catalog_markdown()` 生成，并由测试校验与代码一致。更新命令：`UPDATE_DOCS=1 cargo test -p classifier --test catalog`。

<!-- BEGIN GENERATED: cargo test -p classifier --test catalog -->
## `connection_error`

Connection refused, DNS failure, TLS failure.

| error_code | meaning |
|---|---|
| `CONN_REFUSED` | The target refused the connection. |
| `CONN_DNS` | The host name could not be resolved. |
| `CONN_TLS` | TLS handshake or certificate verification failed. |
| `CONN_RESET` | The connection was reset or closed before a response arrived. |
| `CONN_OTHER` | Any other transport failure. |

## `timeout`

A request or a poll ran out of time.

| error_code | meaning |
|---|---|
| `TIMEOUT_CONNECT` | Establishing the connection timed out. |
| `TIMEOUT_REQUEST` | A request got no complete response within its timeout. |
| `TIMEOUT_POLL` | A poll condition did not hold within `timeout_ms`. |
| `TIMEOUT_POLL_ATTEMPTS` | A poll condition did not hold within `max_attempts`. |

## `assertion_failed_status`

The HTTP status code did not match.

| error_code | meaning |
|---|---|
| `ASSERT_STATUS_MISMATCH` | The status code differs from the expected one (1xx-3xx actual). |
| `ASSERT_STATUS_CLIENT_ERROR` | An unexpected 4xx status code. |
| `ASSERT_STATUS_SERVER_ERROR` | An unexpected 5xx status code. |

## `assertion_failed_body_shape`

The response body structure or a field value did not match.

| error_code | meaning |
|---|---|
| `ASSERT_BODY_VALUE` | A field has a different value than expected. |
| `ASSERT_BODY_MISSING_FIELD` | An asserted field is absent from the response. |
| `ASSERT_BODY_TYPE` | A field has a different JSON type than expected. |

## `capture_error`

Upstream checks passed but a value could not be extracted (response shape drift).

| error_code | meaning |
|---|---|
| `CAPTURE_MISSING_PATH` | A referenced field is absent from an upstream output. |
| `CAPTURE_MISSING_NODE` | A referenced upstream node produced no output. |
| `CAPTURE_EXTRACTION` | A value could not be extracted from a response. |

## `business_rule_violation`

The system under test rejected the operation by a business rule.

| error_code | meaning |
|---|---|
| `BIZ_CONFLICT` | The target reported a conflict with its current state (e.g. 409). |
| `BIZ_UNPROCESSABLE` | The target rejected a well-formed request as invalid (e.g. 422). |
| `BIZ_STATE_TRANSITION` | The target reported an illegal state transition. |

## `unknown`

Could not be classified; handled manually.

| error_code | meaning |
|---|---|
| `CONFIG_MISSING_ENV` | A referenced environment variable is not defined. |
| `CONFIG_AUTH_UNAVAILABLE` | A referenced credential could not be resolved. |
| `CONFIG_INVALID_TEMPLATE` | A template could not be parsed at run time. |
| `ENGINE_UNSUPPORTED` | No executor handles this node type. |
| `ENGINE_INTERNAL` | An engine-side failure (store, subworkflow loading, crash). |
| `UNKNOWN` | No rule matched. |

<!-- END GENERATED -->
