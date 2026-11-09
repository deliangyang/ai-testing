//! Stable issue codes. Treat as public API: tools, fixtures and the UI match on them.

pub const DUPLICATE_NODE_ID: &str = "structure.duplicate_node_id";
pub const INVALID_NODE_ID: &str = "structure.invalid_node_id";
pub const RESERVED_NODE_ID: &str = "structure.reserved_node_id";
pub const DANGLING_EDGE: &str = "structure.dangling_edge";
pub const SELF_LOOP: &str = "structure.self_loop";
pub const DUPLICATE_EDGE: &str = "structure.duplicate_edge";

pub const CYCLE: &str = "graph.cycle";

pub const SUBWORKFLOW_CYCLE: &str = "subworkflow.cycle";
pub const SUBWORKFLOW_UNKNOWN_DAG: &str = "subworkflow.unknown_dag";

pub const INVALID_TEMPLATE: &str = "reference.invalid_template";
pub const UNKNOWN_NODE: &str = "reference.unknown_node";
pub const NOT_ANCESTOR: &str = "reference.not_ancestor";
pub const UNKNOWN_FIELD: &str = "reference.unknown_field";
pub const UNVERIFIED_FIELD: &str = "reference.unverified_field";

pub const CONDITION_INVALID_PATH: &str = "condition.invalid_path";
pub const CONDITION_UNKNOWN_FIELD: &str = "condition.unknown_field";
pub const CONDITION_MISSING_EXPECTED: &str = "condition.missing_expected";
pub const CONDITION_UNEXPECTED_EXPECTED: &str = "condition.unexpected_expected";
pub const CONDITION_EXPECTED_TYPE: &str = "condition.expected_type";

pub const POLL_EMPTY_BACKOFF: &str = "poll.empty_backoff";
pub const POLL_BACKOFF_DECREASING: &str = "poll.backoff_decreasing";
pub const POLL_TIMEOUT_TOO_SHORT: &str = "poll.timeout_too_short";
pub const POLL_ZERO_MAX_ATTEMPTS: &str = "poll.zero_max_attempts";

pub const EMPTY_FIELD: &str = "completeness.empty_field";
pub const ORPHAN_NODE: &str = "completeness.orphan_node";
pub const CLEANUP_HAS_SUCCESSOR: &str = "completeness.cleanup_has_successor";
pub const UNASSERTED_HTTP: &str = "completeness.unasserted_http";

pub const REPAIR_ADD_EDGE: &str = "calibrate.add_edge";
pub const REPAIR_POLL_BACKOFF: &str = "calibrate.poll_backoff";
pub const REPAIR_POLL_TIMEOUT: &str = "calibrate.poll_timeout";
