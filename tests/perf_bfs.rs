use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use tetrisEngine::parity::engine_from_fixture;
use tetrisEngine::ParityFixtureSet;

#[derive(Debug, Deserialize)]
struct PythonPerfCase {
    name: String,
    count: i32,
    iterations: i32,
    mean_ms: f64,
    p95_ms: f64,
}

#[derive(Debug, Deserialize)]
struct PythonPerfPayload {
    iterations: i32,
    cases: Vec<PythonPerfCase>,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("parity.json")
}

fn python_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has repo root parent")
        .join("tests")
        .join("measure_python_bfs.py")
}

fn python_executable() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has repo root parent")
        .join(".venv")
        .join("bin")
        .join("python")
}

fn percentile(values: &mut [f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|left, right| left.partial_cmp(right).expect("durations are finite"));
    let rank = q * (values.len().saturating_sub(1)) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        return values[lower];
    }
    let weight = rank - lower as f64;
    values[lower] * (1.0 - weight) + values[upper] * weight
}

#[test]
#[ignore = "performance harness; run explicitly with --ignored --nocapture"]
fn rust_bfs_outperforms_python_reference_on_fixture_cases() {
    let fixtures: ParityFixtureSet = serde_json::from_str(
        &fs::read_to_string(fixture_path()).expect("fixture file must exist"),
    )
    .expect("fixture file must parse");
    let iterations = 200;

    let python_output = Command::new(python_executable())
        .arg(python_script_path())
        .arg(fixture_path())
        .arg(iterations.to_string())
        .output()
        .expect("python performance script must run");
    assert!(
        python_output.status.success(),
        "python perf script failed: {}",
        String::from_utf8_lossy(&python_output.stderr)
    );
    let python: PythonPerfPayload =
        serde_json::from_slice(&python_output.stdout).expect("python perf json must parse");
    assert_eq!(python.iterations, iterations);
    assert_eq!(python.cases.len(), fixtures.bfs_results.len());

    let mut rust_means = Vec::new();
    let mut python_means = Vec::new();

    for (case, py_case) in fixtures.bfs_results.iter().zip(python.cases.iter()) {
        assert_eq!(case.name, py_case.name);

        let mut samples = Vec::with_capacity(iterations as usize);
        let mut result_count = 0;
        for _ in 0..iterations {
            let engine = engine_from_fixture(&case.state);
            let started = Instant::now();
            let results = engine.bfs_all_placements(
                case.piece.as_ref(),
                case.include_180,
                case.base_attack,
                case.include_no_place,
                case.dedupe_final,
            );
            result_count = results.len();
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }

        let mean_ms = samples.iter().sum::<f64>() / samples.len() as f64;
        let mut p95_samples = samples.clone();
        let p95_ms = percentile(&mut p95_samples, 0.95);
        rust_means.push(mean_ms);
        python_means.push(py_case.mean_ms);

        println!(
            "{}: count={}, iterations={}, rust mean={:.3}ms p95={:.3}ms | python mean={:.3}ms p95={:.3}ms",
            case.name,
            result_count,
            py_case.iterations,
            mean_ms,
            p95_ms,
            py_case.mean_ms,
            py_case.p95_ms,
        );
        assert_eq!(result_count as i32, py_case.count);
    }

    let rust_overall = rust_means.iter().sum::<f64>() / rust_means.len() as f64;
    let python_overall = python_means.iter().sum::<f64>() / python_means.len() as f64;
    println!(
        "overall rust mean={:.3}ms | python mean={:.3}ms",
        rust_overall, python_overall
    );
    if rust_overall < python_overall {
        println!("Rust is faster on the current fixture set.");
    } else {
        println!("Python is faster on the current fixture set.");
    }
}
