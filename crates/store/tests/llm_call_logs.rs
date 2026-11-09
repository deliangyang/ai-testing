#![allow(clippy::unwrap_used)]

use common::clock::{FixedClock, Timestamp};
use common::id::SeqIdGen;
use model::Dag;
use store::{
    DagRepository, Db, LlmCallLogRepository, LlmCallOutcome, NewLlmCallLog, NewRun, RunMode,
    RunRepository, StoreError,
};

struct Fixture {
    db: Db,
    ids: SeqIdGen,
    run_id: String,
}

impl Fixture {
    fn new() -> Self {
        let clock = FixedClock::new(Timestamp::from_unix_millis(0));
        let db = Db::open_in_memory(&clock).unwrap();
        let dag = Dag::from_json_str(
            r#"{"id": "d", "nodes": [{"id": "a", "type": "http", "method": "GET", "path": "/"}]}"#,
        )
        .unwrap();
        DagRepository::new(db.conn(), &clock)
            .save(&dag, "t")
            .unwrap();
        let run_ids = SeqIdGen::new("run");
        let run_id = RunRepository::new(db.conn(), &clock, &run_ids)
            .create(&NewRun {
                dag_id: "d".into(),
                dag_version: 1,
                env_name: "dev".into(),
                mode: RunMode::Manual,
            })
            .unwrap()
            .id;
        Self {
            db,
            ids: SeqIdGen::new("llm"),
            run_id,
        }
    }

    fn repo(&self) -> LlmCallLogRepository<'_> {
        LlmCallLogRepository::new(self.db.conn(), &self.ids)
    }
}

#[test]
fn insert_assigns_id_and_round_trips() {
    let f = Fixture::new();
    let row = f
        .repo()
        .insert(&NewLlmCallLog {
            run_id: Some(f.run_id.clone()),
            node_id: Some("a".into()),
            purpose: "dag_repair".into(),
            tokens_in: 10,
            tokens_out: 5,
            cost: None,
            latency_ms: 120,
            outcome: LlmCallOutcome::Success,
        })
        .unwrap();
    assert_eq!(row.id, "llm-000001");
    assert_eq!(row.cost, None);
    let listed = f.repo().list_for_run(&f.run_id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], row);
}

#[test]
fn list_by_purpose_respects_limit_and_order() {
    let f = Fixture::new();
    let repo = f.repo();
    for i in 0..3 {
        repo.insert(&NewLlmCallLog {
            run_id: None,
            node_id: None,
            purpose: "import_markdown".into(),
            tokens_in: i,
            tokens_out: 1,
            cost: Some(0.001 * f64::from(i)),
            latency_ms: 50,
            outcome: LlmCallOutcome::Success,
        })
        .unwrap();
    }
    repo.insert(&NewLlmCallLog {
        run_id: None,
        node_id: None,
        purpose: "other".into(),
        tokens_in: 0,
        tokens_out: 0,
        cost: None,
        latency_ms: 1,
        outcome: LlmCallOutcome::Error,
    })
    .unwrap();
    let latest = repo.list_by_purpose("import_markdown", 2).unwrap();
    assert_eq!(latest.len(), 2);
    assert_eq!(latest[0].tokens_in, 2);
    assert_eq!(latest[1].tokens_in, 1);
}

#[test]
fn empty_purpose_rejected() {
    let f = Fixture::new();
    let err = f
        .repo()
        .insert(&NewLlmCallLog {
            run_id: None,
            node_id: None,
            purpose: "   ".into(),
            tokens_in: 0,
            tokens_out: 0,
            cost: None,
            latency_ms: 0,
            outcome: LlmCallOutcome::Error,
        })
        .unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)));
}
