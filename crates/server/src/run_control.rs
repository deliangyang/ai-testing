//! Active runs and background execution.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use common::TokioSleeper;
use engine::subworkflow::StoreDagSource;
use engine::{
    event_channel, CancelToken, Environment, HttpExecutor, NodeRouter, RuleClassifier, RunRecorder,
    Scheduler, StoreRecorder, SubworkflowExecutor, SystemEnv,
};
use model::Dag;
use store::{NewRun, RunMode};
use tracing::warn;

use crate::event_hub::forward_engine_events;
use crate::state::AppState;

pub const CREATED_BY: &str = "api";

#[derive(Default)]
pub struct RunRegistry {
    tokens: RwLock<HashMap<String, CancelToken>>,
}

impl RunRegistry {
    pub fn register(&self, run_id: &str, token: CancelToken) {
        let mut guard = self.tokens.write().unwrap_or_else(|e| e.into_inner());
        guard.insert(run_id.to_owned(), token);
    }

    pub fn cancel(&self, run_id: &str) -> bool {
        let token = self
            .tokens
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(run_id)
            .cloned();
        token.is_some_and(|t| {
            t.cancel();
            true
        })
    }

    pub fn remove(&self, run_id: &str) {
        let mut guard = self.tokens.write().unwrap_or_else(|e| e.into_inner());
        guard.remove(run_id);
    }
}

pub struct StartRunRequest {
    pub dag_id: String,
    pub env: String,
    pub parallel: usize,
}

pub fn spawn_run(state: AppState, req: StartRunRequest) -> Result<String, String> {
    let env =
        Environment::load(&state.env_root, &req.env, &SystemEnv).map_err(|e| e.to_string())?;
    let parallel = req.parallel.max(1);
    HttpExecutor::new(Arc::clone(&state.clock)).map_err(|e| e.to_string())?;
    let classifier = Arc::new(RuleClassifier::new().map_err(|e| e.to_string())?);
    let recorder = Arc::new(StoreRecorder::new(
        Arc::clone(&state.db),
        Arc::clone(&state.clock),
        Arc::clone(&state.ids),
        classifier,
    ));

    let (dag, dag_version) = {
        let db = state
            .db
            .lock()
            .map_err(|_| "store lock poisoned".to_owned())?;
        let repo = store::DagRepository::new(db.conn(), state.clock.as_ref());
        let record = repo
            .active(&req.dag_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("DAG '{}' has no active version", req.dag_id))?;
        env.check_dag(&record.dag).map_err(|e| e.to_string())?;
        (record.dag, record.info.version)
    };
    let run = recorder
        .begin_run(&NewRun {
            dag_id: req.dag_id.clone(),
            dag_version,
            env_name: env.name().to_owned(),
            mode: RunMode::Manual,
        })
        .map_err(|e| e.to_string())?;
    let run_id = run.id;

    let cancel = CancelToken::new();
    state.runs.register(&run_id, cancel.clone());
    let returned = run_id.clone();
    let thread_run_id = run_id.clone();
    let state_bg = state.clone();
    let env = Arc::new(env);
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                warn!(%thread_run_id, error = %e, "cannot start run runtime");
                state_bg.runs.remove(&thread_run_id);
                return;
            }
        };
        runtime.block_on(async {
            if let Err(message) = execute_run(
                state_bg.clone(),
                thread_run_id.clone(),
                dag,
                cancel,
                env,
                parallel,
                recorder,
            )
            .await
            {
                warn!(%thread_run_id, %message, "run failed");
            }
            state_bg.runs.remove(&thread_run_id);
        });
    });
    Ok(returned)
}

async fn execute_run(
    state: AppState,
    run_id: String,
    dag: Dag,
    cancel: CancelToken,
    env: Arc<Environment>,
    parallel: usize,
    recorder: Arc<StoreRecorder>,
) -> Result<(), String> {
    let (sink, rx) = event_channel();
    forward_engine_events(Arc::clone(&state.hub), run_id.clone(), rx);

    let http = HttpExecutor::new(Arc::clone(&state.clock)).map_err(|e| e.to_string())?;
    let router = NodeRouter::new(
        Arc::new(http),
        Arc::clone(&state.clock),
        Arc::new(TokioSleeper),
    )
    .with_subworkflows(
        SubworkflowExecutor::new(Arc::new(StoreDagSource::new(
            Arc::clone(&state.db),
            Arc::clone(&state.clock),
        )))
        .with_max_parallel(parallel),
    );

    let sink_recorder: Arc<dyn RunRecorder> = recorder.clone();
    let outcome = Scheduler::new(Arc::new(router))
        .with_shared_environment(env)
        .with_max_parallel(parallel)
        .with_events(Some(sink))
        .with_recorder(Some(sink_recorder))
        .with_cancel(cancel)
        .run(&run_id, &dag)
        .await
        .map_err(|e| e.to_string())?;

    recorder
        .finish_run(&run_id, &outcome)
        .map_err(|e| e.to_string())?;
    Ok(())
}
