//! The command line as somebody actually runs it.
//!
//! These drive the built binary rather than the library behind it, because the
//! things worth pinning here are the ones only the binary decides: what gets
//! written, what the exit status is, and whether a machine-readable answer
//! parses. A report that reads well but exits zero on a broken design is worse
//! than useless in continuous integration, and only this level of test can tell
//! the difference.

use std::{path::PathBuf, process::Command};

/// Runs the binary at a fixed width and without colour, so a report is compared
/// against what it says rather than against the terminal that captured it.
fn optimist(arguments: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_optimist"))
        .args(arguments)
        .env("COLUMNS", "100")
        .env("NO_COLOR", "1")
        .output()
        .expect("the binary runs");
    (
        output.status.success(),
        String::from_utf8(output.stdout).expect("stdout is text"),
        String::from_utf8(output.stderr).expect("stderr is text"),
    )
}

fn example(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
        .display()
        .to_string()
}

/// Writes a design that parses and is still wrong in several ways.
fn broken_design() -> tempdir::TempDir {
    let directory = tempdir::TempDir::new();
    std::fs::write(
        directory.path.join("_system.yaml"),
        "schema_version: 2\n\
         name: Broken\n\
         summary: A design that loads and does not mean what it says.\n\
         scratchpad:\n\
         - name: peak_rate\n  expression: '900'\n  unit: op/s\n\
         interventions:\n\
         - id: wishful\n  name: Wishful\n  overrides:\n  - name: no_such_quantity\n    expression: '1'\n",
    )
    .expect("the system document is written");
    std::fs::create_dir_all(directory.path.join("components")).expect("components/ is created");
    std::fs::write(
        directory.path.join("components/api.yaml"),
        "id: api\n\
         name: API\n\
         type: compute\n\
         properties:\n  parallelism: '8'\n  servce_time: '0.02'\n",
    )
    .expect("the component document is written");
    directory
}

#[test]
fn checking_a_sound_design_succeeds_and_describes_it() {
    let (ok, stdout, _) = optimist(&["check", &example("checkout")]);
    assert!(ok, "{stdout}");
    assert!(stdout.contains("Checkout"), "{stdout}");
    assert!(stdout.contains("Components"), "{stdout}");
    assert!(stdout.contains("Nothing to report"), "{stdout}");
}

#[test]
fn checking_a_broken_design_names_every_fault_and_fails() {
    let design = broken_design();
    let (ok, stdout, stderr) = optimist(&["check", &design.path.display().to_string()]);

    assert!(!ok, "a design with errors must not exit zero:\n{stdout}");
    // A property nobody declared, one nobody supplied, a component wired to
    // nothing, and an intervention rebinding a quantity that does not exist.
    assert!(stdout.contains("servce_time"), "{stdout}");
    assert!(stdout.contains("service_time"), "{stdout}");
    assert!(stdout.contains("not wired to anything"), "{stdout}");
    assert!(stdout.contains("no_such_quantity"), "{stdout}");
    assert!(stderr.contains("stop it being solved"), "{stderr}");
}

#[test]
fn checking_reports_the_same_faults_to_a_machine() {
    let design = broken_design();
    let (ok, stdout, _) = optimist(&[
        "--output",
        "json",
        "check",
        &design.path.display().to_string(),
    ]);
    assert!(!ok);

    let report: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(report["solvable"], serde_json::json!(false));
    assert_eq!(report["name"], serde_json::json!("Broken"));
    let findings = report["findings"].as_array().expect("findings are a list");
    assert!(
        findings
            .iter()
            .any(|finding| finding["severity"] == serde_json::json!("error")),
        "{stdout}"
    );
}

#[test]
fn solving_reports_a_component_and_the_traffic_reaching_it() {
    let (ok, stdout, _) = optimist(&["solve", &example("checkout"), "--component", "api"]);
    assert!(ok, "{stdout}");
    assert!(stdout.contains("utilisation"), "{stdout}");
    assert!(stdout.contains("in.requests.rate"), "{stdout}");
    // Only the component that was asked for.
    assert!(!stdout.contains("browsers"), "{stdout}");
}

#[test]
fn bottlenecks_rank_the_worst_constraint_first() {
    let (ok, stdout, _) = optimist(&["bottlenecks", &example("checkout"), "--binding"]);
    assert!(ok, "{stdout}");
    assert!(stdout.contains("runs out first"), "{stdout}");

    let (ok, json, _) = optimist(&["--output", "json", "bottlenecks", &example("checkout")]);
    assert!(ok);
    let ranked: Vec<serde_json::Value> = serde_json::from_str(&json).expect("valid JSON");
    let first = ranked.first().expect("at least one constraint");
    assert!(
        ranked
            .windows(2)
            .all(|pair| pair[0]["probability_of_binding"].as_f64()
                >= pair[1]["probability_of_binding"].as_f64()),
        "{json}"
    );
    assert!(first["probability_of_binding"].as_f64() > Some(0.0));
}

#[test]
fn comparing_weighs_several_proposals_against_one_baseline() {
    let (ok, stdout, _) = optimist(&["compare", &example("checkout"), "warm-cache", "bigger-pool"]);
    assert!(ok, "{stdout}");
    assert!(stdout.contains("warm-cache"), "{stdout}");
    assert!(stdout.contains("bigger-pool"), "{stdout}");
    assert!(stdout.contains("relieved"), "{stdout}");

    let (ok, lines, _) = optimist(&[
        "--output",
        "jsonl",
        "compare",
        &example("checkout"),
        "warm-cache",
        "bigger-pool",
    ]);
    assert!(ok);
    let named: Vec<String> = lines
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("valid JSON")["intervention"]
                .as_str()
                .expect("each movement names its intervention")
                .to_owned()
        })
        .collect();
    assert!(named.contains(&"warm-cache".to_owned()), "{lines}");
    assert!(named.contains(&"bigger-pool".to_owned()), "{lines}");
}

#[test]
fn a_mistyped_intervention_is_answered_with_the_ones_that_exist() {
    let (ok, _, stderr) = optimist(&["compare", &example("checkout"), "warm-cash"]);
    assert!(!ok);
    assert!(stderr.contains("warm-cash"), "{stderr}");
    assert!(stderr.contains("warm-cache"), "{stderr}");
}

#[test]
fn the_catalogue_describes_a_type_in_full() {
    let (ok, stdout, _) = optimist(&["catalogue", &example("checkout"), "--type", "queue"]);
    assert!(ok, "{stdout}");
    assert!(stdout.contains("service_rate"), "{stdout}");
    assert!(
        stdout.contains("CONSTRAINT") || stdout.contains("LIMIT"),
        "{stdout}"
    );

    let (ok, _, stderr) = optimist(&["catalogue", &example("checkout"), "--type", "nonesuch"]);
    assert!(!ok);
    assert!(stderr.contains("nonesuch"), "{stderr}");
}

#[test]
fn reports_stay_within_the_width_they_are_given() {
    let (_, stdout, _) = optimist(&["bottlenecks", &example("checkout")]);
    for line in stdout.lines() {
        assert!(
            line.chars().count() <= 100,
            "line longer than the terminal: {line}"
        );
    }
}

/// A throwaway directory that removes itself.
mod tempdir {
    use std::path::PathBuf;

    pub(super) struct TempDir {
        pub(super) path: PathBuf,
    }

    impl TempDir {
        pub(super) fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "optimist-cli-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("a temporary directory is created");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
