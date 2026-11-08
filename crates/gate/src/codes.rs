//! Stable issue codes. Treat as public API: tools, fixtures and the UI match on them.

pub const DUPLICATE_NODE_ID: &str = "structure.duplicate_node_id";
pub const INVALID_NODE_ID: &str = "structure.invalid_node_id";
pub const RESERVED_NODE_ID: &str = "structure.reserved_node_id";
pub const DANGLING_EDGE: &str = "structure.dangling_edge";
pub const SELF_LOOP: &str = "structure.self_loop";
pub const DUPLICATE_EDGE: &str = "structure.duplicate_edge";

pub const CYCLE: &str = "graph.cycle";

pub const INVALID_TEMPLATE: &str = "reference.invalid_template";
pub const UNKNOWN_NODE: &str = "reference.unknown_node";
pub const NOT_ANCESTOR: &str = "reference.not_ancestor";
pub const UNKNOWN_FIELD: &str = "reference.unknown_field";
pub const UNVERIFIED_FIELD: &str = "reference.unverified_field";
