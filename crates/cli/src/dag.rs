//! `mytool dag ...`

use std::path::Path;

use gate::{Gate, Report};
use model::to_canonical_json;
use serde_json::json;

use crate::args::OutputFormat;
use crate::exit;
use crate::input::{load_dag, Loaded};

/// Exit 0 when the gate passes (warnings allowed), 3 when it reports errors or the file is not
/// a DAG.
pub fn validate(file: &Path, format: OutputFormat) -> anyhow::Result<u8> {
    let report = match load_dag(file)? {
        Loaded::Dag(dag) => Gate::standard().validate(&dag),
        Loaded::Invalid(report) => report,
    };
    print_report(file, &report, format)?;
    Ok(if report.passed() {
        exit::SUCCESS
    } else {
        exit::INVALID
    })
}

fn print_report(file: &Path, report: &Report, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            let doc = json!({
                "file": file.display().to_string(),
                "passed": report.passed(),
                "issues": report.issues,
            });
            println!("{}", to_canonical_json(&doc)?);
        }
        OutputFormat::Text => {
            for issue in &report.issues {
                println!("{issue}");
            }
            let errors = report.errors().count();
            let warnings = report.warnings().count();
            let verdict = if report.passed() {
                "passed"
            } else {
                "rejected"
            };
            println!(
                "{}: {verdict} ({errors} error{}, {warnings} warning{})",
                file.display(),
                plural(errors),
                plural(warnings)
            );
        }
    }
    Ok(())
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
