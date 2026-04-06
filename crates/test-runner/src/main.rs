use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use test_runner::{
    discover_tests_recursive, run_wpt_test, testharness_preamble, testharnessreport_shim,
    to_wpt_relative, workspace_root, wrap_js_in_html, wpt_root,
};

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Status {
    Pass,
    Fail,
    NotRun,
}

#[derive(Debug, Clone)]
struct ManifestEntry {
    status: Status,
    path: String,
    alt: Option<String>,
}

fn parse_manifest(path: &std::path::Path) -> Vec<ManifestEntry> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut entries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (status, rest) = if let Some(rest) = line.strip_prefix("PASS ") {
            (Status::Pass, rest.trim())
        } else if let Some(rest) = line.strip_prefix("FAIL ") {
            (Status::Fail, rest.trim())
        } else if let Some(rest) = line.strip_prefix("NOT_RUN ") {
            (Status::NotRun, rest.trim())
        } else {
            eprintln!("warning: skipping malformed manifest line: {}", line);
            continue;
        };
        let (path, alt) = if let Some(idx) = rest.find(" alt:") {
            (rest[..idx].trim(), Some(rest[idx + 5..].trim().to_string()))
        } else {
            (rest, None)
        };
        entries.push(ManifestEntry {
            status,
            path: path.to_string(),
            alt,
        });
    }
    entries
}

fn write_manifest(path: &std::path::Path, entries: &[ManifestEntry]) {
    let mut content = String::new();
    for entry in entries {
        let status = match entry.status {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::NotRun => "NOT_RUN",
        };
        if let Some(alt) = &entry.alt {
            content.push_str(&format!("{} {} alt:{}\n", status, entry.path, alt));
        } else {
            content.push_str(&format!("{} {}\n", status, entry.path));
        }
    }
    std::fs::write(path, content).unwrap();
}

// ---------------------------------------------------------------------------
// Modes
// ---------------------------------------------------------------------------

fn run_edge(manifest_path: &std::path::Path) {
    let mut entries = parse_manifest(manifest_path);
    let total = entries.len();

    if total == 0 {
        eprintln!("manifest is empty. Run with --discover first.");
        std::process::exit(1);
    }

    loop {
        let pass_count = entries.iter().filter(|e| e.status == Status::Pass).count();

        // Find the edge: first non-PASS entry
        let edge_idx = match entries.iter().position(|e| e.status != Status::Pass) {
            Some(idx) => idx,
            None => {
                println!("[{}/{}] all tests passing", total, total);
                return;
            }
        };

        let test_path = entries[edge_idx].path.clone();
        let alt_path = entries[edge_idx].alt.clone();
        let display_idx = edge_idx + 1;

        let run_path = alt_path.as_deref().unwrap_or(&test_path);
        if alt_path.is_some() {
            eprint!("[{}/{}] {} (alt:{}) ", display_idx, total, test_path, run_path);
        } else {
            eprint!("[{}/{}] {} ", display_idx, total, test_path);
        }
        std::io::stderr().flush().unwrap();
        let result = run_single_test(run_path);

        if result {
            entries[edge_idx].status = Status::Pass;
            write_manifest(manifest_path, &entries);
            println!(
                "[{}/{}] PASS {}",
                display_idx, total, test_path
            );
            std::io::stdout().flush().unwrap();
        } else {
            if entries[edge_idx].status == Status::NotRun {
                entries[edge_idx].status = Status::Fail;
                write_manifest(manifest_path, &entries);
            }
            println!(
                "[{}/{}] FAIL {} \u{2014} high water mark: {}",
                display_idx, total, test_path, pass_count
            );
            std::io::stdout().flush().unwrap();
            std::process::exit(1);
        }
    }
}

fn run_regression(manifest_path: &std::path::Path, from: usize) {
    let mut entries = parse_manifest(manifest_path);
    let pass_entries: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.status == Status::Pass)
        .map(|(i, _)| i)
        .collect();
    let total_pass = pass_entries.len();
    let total = entries.len();

    if total_pass == 0 {
        println!("no PASS tests to check for regressions.");
        return;
    }

    // --from is 1-based test number; skip_count converts to 0-based index into pass_entries
    let skip_count = if from > 0 { from - 1 } else { 0 };
    if skip_count > 0 {
        println!("skipping to test {}...", from);
    }

    println!("checking {} PASS tests for regressions...", total_pass);

    for (check_num, &idx) in pass_entries.iter().enumerate().skip(skip_count) {
        let test_path = entries[idx].path.clone();
        let alt_path = entries[idx].alt.clone();
        let run_path = alt_path.as_deref().unwrap_or(&test_path);

        let result = run_single_test(run_path);

        if result {
            println!("[{}/{}] ok {}", check_num + 1, total_pass, test_path);
            std::io::stdout().flush().unwrap();
        } else {
            println!(
                "[{}/{}] REGRESSION {} \u{2014} flipping to FAIL",
                check_num + 1,
                total_pass,
                test_path
            );
            entries[idx].status = Status::Fail;
            write_manifest(manifest_path, &entries);

            let new_pass = entries.iter().filter(|e| e.status == Status::Pass).count();
            println!(
                "high water mark dropped: {} -> {}",
                total_pass, new_pass
            );
            std::process::exit(1);
        }
    }

    let new_hwm = entries
        .iter()
        .position(|e| e.status != Status::Pass)
        .unwrap_or(total);
    println!(
        "\nall {} PASS tests verified. high water mark: {}",
        total_pass, new_hwm
    );
}

fn run_discover(manifest_path: &std::path::Path) {
    let existing = parse_manifest(manifest_path);
    let existing_paths: std::collections::HashSet<String> =
        existing.iter().map(|e| e.path.clone()).collect();

    // Build the full set of tests that exist right now
    let mut live_tests: Vec<String> = Vec::new();

    // 1. Discover cargo tests (unit + integration)
    print!("discovering cargo tests...");
    std::io::stdout().flush().unwrap();
    let cargo_tests = discover_cargo_tests();
    println!(" found {}", cargo_tests.len());
    live_tests.extend(cargo_tests);

    // 2. Discover WPT test files
    print!("discovering WPT tests...");
    std::io::stdout().flush().unwrap();
    let wpt = wpt_root();
    let wpt_tests = discover_tests_recursive(&wpt);
    println!(" found {}", wpt_tests.len());
    for test in wpt_tests {
        let rel = to_wpt_relative(&test);
        live_tests.push(format!("wpt:{}", rel));
    }

    let live_set: std::collections::HashSet<String> = live_tests.iter().cloned().collect();

    // Count additions and removals
    let mut added = 0;
    let mut removed = 0;

    // Scan test files for "// alt_for:" comments
    let alt_map = discover_alt_mappings();
    let mut alt_count = 0;

    // Split existing entries into two groups:
    //   1. PASS — preserve their order exactly (the history)
    //   2. FAIL / NOT_RUN — will be re-sorted with new tests (the todo list)
    let mut passed: Vec<ManifestEntry> = Vec::new();
    let mut existing_todo: Vec<ManifestEntry> = Vec::new();
    for entry in &existing {
        if !live_set.contains(&entry.path) {
            removed += 1;
            println!("  removed: {}", entry.path);
            continue;
        }
        let mut entry = entry.clone();
        // Update alt mappings
        if let Some(alt) = alt_map.get(&entry.path) {
            if entry.alt.as_ref() != Some(alt) {
                entry.alt = Some(alt.clone());
                alt_count += 1;
            }
        }
        if entry.status == Status::Pass {
            passed.push(entry);
        } else {
            existing_todo.push(entry);
        }
    }

    // Collect genuinely new tests (not in existing manifest)
    let mut new_tests: Vec<ManifestEntry> = Vec::new();
    for path in &live_tests {
        if !existing_paths.contains(path) {
            added += 1;
            let alt = alt_map.get(path).cloned();
            new_tests.push(ManifestEntry {
                status: Status::NotRun,
                path: path.clone(),
                alt,
            });
        }
    }

    // Merge existing FAIL/NOT_RUN + new tests, then sort:
    //   - Our own tests (cargo:) first, then WPT non-tentative, then WPT tentative
    //   - Within each group, alphabetical
    // This ensures the ratchet always grinds our own tests before WPT.
    let mut todo: Vec<ManifestEntry> = existing_todo;
    todo.extend(new_tests);
    todo.sort_by(|a, b| {
        fn sort_key(path: &str) -> (u8, &str) {
            if path.starts_with("cargo:") {
                (0, path)
            } else if path.contains(".tentative.") {
                (2, path)
            } else {
                (1, path)
            }
        }
        sort_key(&a.path).cmp(&sort_key(&b.path))
    });

    // Final manifest: passed (stable order) then todo (our tests first)
    let mut new_entries = passed;
    new_entries.extend(todo);
    if alt_count > 0 {
        println!("  updated {} alt mappings", alt_count);
    }

    write_manifest(manifest_path, &new_entries);
    println!(
        "added: {}, removed: {}, total: {}",
        added,
        removed,
        new_entries.len()
    );
}

// ---------------------------------------------------------------------------
// Cargo test discovery
// ---------------------------------------------------------------------------

/// Discover all cargo test functions in braille-engine.
/// Returns entries like "cargo:lib::module::test_name" and "cargo:integration_file::test_name".
fn discover_cargo_tests() -> Vec<String> {
    let mut tests = Vec::new();
    let root = workspace_root();

    // Unit tests (--lib)
    let output = Command::new("cargo")
        .args(["test", "-p", "braille-engine", "--lib", "--", "--list"])
        .current_dir(&root)
        .output()
        .expect("failed to run cargo test --list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(name) = line.strip_suffix(": test") {
            tests.push(format!("cargo:lib::{}", name));
        }
    }

    // Integration tests — discover test files then list their functions
    let test_dir = root.join("crates/engine/tests");
    if test_dir.exists() {
        let mut files: Vec<_> = std::fs::read_dir(&test_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_str().unwrap_or("").to_string();
                name.ends_with(".rs")
                    && !name.starts_with("html5lib") // custom harness, not standard #[test]
            })
            .collect();
        files.sort_by_key(|e| e.file_name());

        for entry in files {
            let file_name = entry.file_name();
            let test_name = file_name
                .to_str()
                .unwrap()
                .strip_suffix(".rs")
                .unwrap();

            let output = Command::new("cargo")
                .args([
                    "test",
                    "-p",
                    "braille-engine",
                    "--test",
                    test_name,
                    "--",
                    "--list",
                ])
                .current_dir(&root)
                .output();

            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Some(fn_name) = line.strip_suffix(": test") {
                        tests.push(format!("cargo:{}::{}", test_name, fn_name));
                    }
                }
            }
        }
    }

    tests
}

// ---------------------------------------------------------------------------
// Alt-for discovery
// ---------------------------------------------------------------------------

/// Scan integration test files for `// alt_for: <test_id>` comments.
/// Returns a map from the target test_id to the cargo test that replaces it.
fn discover_alt_mappings() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let root = workspace_root();
    let test_dir = root.join("crates/engine/tests");
    if !test_dir.exists() {
        return map;
    }

    for entry in std::fs::read_dir(&test_dir).unwrap().filter_map(|e| e.ok()) {
        let name = entry.file_name().to_str().unwrap_or("").to_string();
        if !name.ends_with(".rs") || name.starts_with("html5lib") {
            continue;
        }
        let test_file = name.strip_suffix(".rs").unwrap().to_string();
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Look for patterns like:
        //   // alt_for: wpt:WebCryptoAPI/derive_bits_keys/pbkdf2.https.any.js
        //   #[test]
        //   fn some_test_name() {
        let lines: Vec<&str> = content.lines().collect();
        for i in 0..lines.len() {
            let line = lines[i].trim();
            if let Some(target) = line.strip_prefix("// alt_for: ") {
                let target = target.trim().to_string();
                // Find the next #[test] fn name
                for fline in lines.iter().skip(i + 1) {
                    let fline = fline.trim();
                    if let Some(rest) = fline.strip_prefix("fn ") {
                        if let Some(fn_name) = rest.split('(').next() {
                            let cargo_id = format!("cargo:{}::{}", test_file, fn_name.trim());
                            println!("  alt: {} -> {}", target, cargo_id);
                            map.insert(target, cargo_id);
                        }
                        break;
                    }
                }
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Single test execution
// ---------------------------------------------------------------------------

fn run_single_test(test_id: &str) -> bool {
    if let Some(cargo_path) = test_id.strip_prefix("cargo:") {
        run_cargo_test(cargo_path)
    } else if let Some(wpt_path) = test_id.strip_prefix("wpt:") {
        run_wpt_test_single(wpt_path)
    } else {
        // Legacy: bare path treated as WPT
        run_wpt_test_single(test_id)
    }
}

fn run_cargo_test(cargo_path: &str) -> bool {
    let root = workspace_root();

    // Parse "lib::module::test_name" or "test_file::test_name"
    let (test_target, test_name) = if let Some(rest) = cargo_path.strip_prefix("lib::") {
        (vec!["--lib"], rest)
    } else {
        // "test_file::test_name" — split on first "::"
        let parts: Vec<&str> = cargo_path.splitn(2, "::").collect();
        if parts.len() == 2 {
            (vec!["--test", parts[0]], parts[1])
        } else {
            eprintln!("  malformed cargo test path: {}", cargo_path);
            return false;
        }
    };

    let mut args = vec!["test", "-p", "braille-engine"];
    args.extend(test_target.iter().copied());
    args.extend(["--", "--exact", test_name]);

    let output = Command::new("cargo")
        .args(&args)
        .current_dir(&root)
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                true
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Show relevant failure lines
                for line in stdout.lines().chain(stderr.lines()) {
                    if line.contains("FAILED")
                        || line.contains("panicked")
                        || line.contains("assertion")
                        || line.starts_with("thread")
                    {
                        eprintln!("  {}", line);
                    }
                }
                false
            }
        }
        Err(e) => {
            eprintln!("  failed to run cargo test: {}", e);
            false
        }
    }
}

fn run_wpt_test_single(rel_path: &str) -> bool {
    let wpt = wpt_root();
    let preamble = testharness_preamble();
    let report_shim = testharnessreport_shim();

    let abs_path = wpt.join(rel_path);
    let is_js = rel_path.ends_with(".any.js") || rel_path.ends_with(".window.js");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if is_js {
            let html = wrap_js_in_html(&abs_path);
            let tmp_path = std::env::temp_dir().join("wpt_runner_tmp.html");
            std::fs::write(&tmp_path, &html).unwrap();
            run_wpt_test(&tmp_path, &preamble, &report_shim)
        } else {
            run_wpt_test(&abs_path, &preamble, &report_shim)
        }
    }));

    match result {
        Ok(wpt_result) => {
            let total = wpt_result.passed + wpt_result.failed;
            if total > 1 {
                eprintln!();
            }
            if wpt_result.failed == 0 {
                true
            } else {
                for detail in &wpt_result.details {
                    eprintln!("    {}", detail);
                }
                eprintln!(
                    "  ({}/{} subtests passed)",
                    wpt_result.passed,
                    total
                );
                false
            }
        }
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "test panicked".to_string()
            };
            eprintln!("  PANIC: {}", msg);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    // Run in a thread with 32MB stack for deep QuickJS recursion
    let child = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(main_inner)
        .expect("failed to spawn thread");
    child.join().unwrap();
}

fn main_inner() {
    let args: Vec<String> = std::env::args().collect();

    let default_manifest = workspace_root().join("tests/manifest.txt");
    let mut manifest_path = default_manifest;
    let mut mode = "edge";
    let mut from: usize = 0;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--manifest" => {
                i += 1;
                manifest_path = PathBuf::from(&args[i]);
            }
            "--regression" => {
                mode = "regression";
            }
            "--discover" => {
                mode = "discover";
            }
            "--from" => {
                i += 1;
                from = args[i].parse().expect("--from requires a number");
            }
            "--help" | "-h" => {
                println!("Usage: wpt-runner [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --manifest <path>  Path to manifest file (default: tests/manifest.txt)");
                println!("  --discover         Discover cargo + WPT tests, append new as NOT_RUN");
                println!("  --regression       Re-run PASS tests, stop on first failure");
                println!("  --from <N>         Skip to test number N (1-based, works with --regression)");
                println!("  (default)          Run the edge test (first non-PASS)");
                return;
            }
            other => {
                eprintln!("unknown argument: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    match mode {
        "discover" => run_discover(&manifest_path),
        "regression" => run_regression(&manifest_path, from),
        _ => run_edge(&manifest_path),
    }
}
