//! Activation path: deterministic calibration then full save-gate validation (§6.6).

use model::Dag;

use crate::calibrate::{calibrate, RepairRecord};
use crate::{Gate, Report};

/// Outcome of calibrate → validate before a version may become `active`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivationPrepareResult {
    pub dag: Dag,
    pub repairs: Vec<RepairRecord>,
    pub report: Report,
}

/// Apply [`calibrate`] then run the same [`Gate`] used at save/activate time.
#[must_use]
pub fn prepare_for_activation(gate: &Gate, dag: Dag) -> ActivationPrepareResult {
    let calibrated = calibrate(dag);
    let report = gate.validate(&calibrated.dag);
    ActivationPrepareResult {
        dag: calibrated.dag,
        repairs: calibrated.repairs,
        report,
    }
}
