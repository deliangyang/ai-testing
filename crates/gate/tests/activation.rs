mod common;

use common::{dag, edge, http, validate};
use gate::prepare_for_activation;
use gate::Gate;
use serde_json::json;

#[test]
fn prepare_runs_calibrate_then_save_gate() {
    let input = dag(
        vec![
            http("root"),
            http("c"),
            json!({"id": "d", "type": "http", "method": "POST", "path": "/d",
                   "body": {"order": {"id": "${c.output.body.id}"}}}),
        ],
        vec![edge("root", "c"), edge("root", "d")],
    );
    let gate = Gate::standard();
    let prep = prepare_for_activation(&gate, input);
    assert!(!prep.repairs.is_empty());
    assert!(prep.report.passed());
    assert!(validate(&prep.dag).passed());
}
