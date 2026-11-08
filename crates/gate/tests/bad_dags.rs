//! Corpus of hand-written bad DAGs (§7.2 line C): every file must be rejected, either on load
//! or by the gate, with exactly the error codes named in its `# expect:` header.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use gate::Gate;
use model::Dag;

/// Header value for documents that must fail deserialization.
const PARSE: &str = "parse";
const MIN_CORPUS: usize = 20;

fn fixtures(dir: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(dir);
    let mut files: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    files.sort();
    files
}

fn expected_codes(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("# expect:"))
        .flat_map(|codes| codes.split(','))
        .map(|code| code.trim().to_owned())
        .filter(|code| !code.is_empty())
        .collect()
}

/// `Ok` when the file is rejected as its header says, otherwise a description of the mismatch.
fn check_bad(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let expected = expected_codes(&text);
    if expected.is_empty() {
        return Err("missing '# expect:' header".into());
    }
    let dag = match Dag::from_yaml_str(&text) {
        Err(err) if expected.contains(PARSE) => {
            return if expected.len() == 1 {
                Ok(())
            } else {
                Err(format!(
                    "'{PARSE}' cannot be combined with other codes ({err})"
                ))
            };
        }
        Err(err) => return Err(format!("unexpected load failure: {err}")),
        Ok(_) if expected.contains(PARSE) => {
            return Err("expected a load failure, but the document loaded".into())
        }
        Ok(dag) => dag,
    };
    let report = Gate::standard().validate(&dag);
    if report.passed() {
        return Err("passed the gate".into());
    }
    let got: BTreeSet<String> = report.errors().map(|i| i.code.clone()).collect();
    if got != expected {
        let details: Vec<String> = report.errors().map(ToString::to_string).collect();
        return Err(format!(
            "expected {expected:?}, got {got:?}\n    {}",
            details.join("\n    ")
        ));
    }
    Ok(())
}

#[test]
fn every_bad_dag_is_rejected_with_its_expected_codes() {
    let files = fixtures("bad_dags");
    assert!(
        files.len() >= MIN_CORPUS,
        "corpus has {} files, need at least {MIN_CORPUS}",
        files.len()
    );
    let failures: Vec<String> = files
        .iter()
        .filter_map(|path| {
            check_bad(path).err().map(|why| {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                format!("{name}: {why}")
            })
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} bad DAGs not rejected as expected:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

#[test]
fn good_dags_pass_without_errors() {
    let files = fixtures("good_dags");
    assert!(!files.is_empty());
    for path in files {
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
        let dag = Dag::from_yaml_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let report = Gate::standard().validate(&dag);
        let errors: Vec<String> = report.errors().map(ToString::to_string).collect();
        assert!(
            errors.is_empty(),
            "{}:\n{}",
            path.display(),
            errors.join("\n")
        );
    }
}

#[test]
fn header_parsing() {
    assert_eq!(
        expected_codes("# expect: a.b, c.d\n# expect: e\nid: x\n"),
        ["a.b", "c.d", "e"].map(String::from).into_iter().collect()
    );
    assert!(expected_codes("id: x\n").is_empty());
}
