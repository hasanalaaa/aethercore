//! P85-01: what SFC proved is read from the part of the CBS log written during ITS run, not from a
//! tail that holds older runs. A clean verdict needs the run's own completion marker and a zero exit
//! code; anything that cannot be attributed to the run is Unknown, never healthy and never corrupt.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use aethercore_system_repair::cbs::{CbsBaseline, CbsEvidence, Window, classify, window};

const CLEAN_RUN: &str = "[SR] Beginning Verify and Repair transaction\n[SR] Verify complete\n[SR] Repairing 0 components\n[SR] Repair complete\n";
const CORRUPT_RUN: &str = "[SR] Beginning Verify and Repair transaction\n[SR] Cannot repair member file [l:10]'x.dll' of Microsoft-Windows-Foo\n[SR] Verify complete\n";
const PROGRESS_ONLY: &str = "[SR] Beginning Verify and Repair transaction\n[SR] Verifying 100 components\n[SR] Verifying 40 components\n";

fn temp(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "aethercore-cbs-{label}-{}-{}",
        std::process::id(),
        uuid_like()
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    dir.join("CBS.log")
}

fn uuid_like() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos()
}

fn append(path: &Path, text: &str) {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open");
    file.write_all(text.as_bytes()).expect("write");
}

fn run_window(path: &Path, baseline: Option<&CbsBaseline>, exit_code: i32) -> CbsEvidence {
    match window(path, baseline) {
        Window::Text(text) => classify(&text, exit_code),
        Window::Unknown => CbsEvidence::Unknown,
    }
}

#[test]
fn an_old_corrupt_log_does_not_condemn_a_fresh_clean_run() {
    let path = temp("old-corrupt");
    append(&path, CORRUPT_RUN);
    let baseline = CbsBaseline::capture(&path);
    append(&path, CLEAN_RUN);
    assert_eq!(
        run_window(&path, baseline.as_ref(), 0),
        CbsEvidence::NoViolation,
        "the old corruption is not this run's"
    );
}

#[test]
fn an_old_clean_log_does_not_vouch_for_a_run_that_found_corruption() {
    let path = temp("old-clean");
    append(&path, CLEAN_RUN);
    let baseline = CbsBaseline::capture(&path);
    append(&path, CORRUPT_RUN);
    assert_eq!(
        run_window(&path, baseline.as_ref(), 0),
        CbsEvidence::ViolationUnresolved
    );
}

#[test]
fn progress_lines_alone_and_a_run_that_did_not_complete_are_unknown() {
    let path = temp("progress");
    append(&path, CLEAN_RUN);
    let baseline = CbsBaseline::capture(&path);
    append(&path, PROGRESS_ONLY);
    assert_eq!(
        run_window(&path, baseline.as_ref(), 0),
        CbsEvidence::Unknown,
        "[SR] progress is not a verdict"
    );
    let quiet = temp("quiet");
    append(&quiet, CLEAN_RUN);
    let before = CbsBaseline::capture(&quiet);
    assert_eq!(
        run_window(&quiet, before.as_ref(), 0),
        CbsEvidence::Unknown,
        "a run that wrote nothing proved nothing, whatever the old tail says"
    );
}

#[test]
fn a_nonzero_exit_is_never_a_clean_verdict() {
    let path = temp("exit");
    let baseline = CbsBaseline::capture(&path);
    append(&path, CLEAN_RUN);
    assert_eq!(
        run_window(&path, baseline.as_ref(), 0),
        CbsEvidence::NoViolation,
        "no log before the run: the whole file is its window"
    );
    assert_eq!(
        run_window(&path, baseline.as_ref(), 2),
        CbsEvidence::Unknown
    );
    assert_eq!(
        run_window(&path, baseline.as_ref(), 1),
        CbsEvidence::Unknown
    );
}

#[test]
fn repair_activity_in_the_window_is_reported_as_such() {
    let path = temp("repaired");
    let baseline = CbsBaseline::capture(&path);
    append(
        &path,
        "[SR] Beginning Verify and Repair transaction\n[SR] Repairing corrupted file [l:10]'y.dll'\n[SR] Repair complete\n",
    );
    assert_eq!(
        run_window(&path, baseline.as_ref(), 0),
        CbsEvidence::ViolationRepaired
    );
}

#[test]
fn a_log_that_was_rotated_truncated_missing_or_too_large_is_unknown() {
    // Truncated or replaced by a shorter file: the start of the run is gone.
    let path = temp("rotated");
    append(&path, &CLEAN_RUN.repeat(20));
    let baseline = CbsBaseline::capture(&path);
    fs::write(&path, CLEAN_RUN).expect("replace with a shorter log");
    assert_eq!(
        run_window(&path, baseline.as_ref(), 0),
        CbsEvidence::Unknown,
        "a shorter log is a rotation, not a clean run"
    );
    // Gone after the run.
    let gone = temp("gone");
    append(&gone, CLEAN_RUN);
    let before = CbsBaseline::capture(&gone);
    fs::remove_file(&gone).expect("remove");
    assert_eq!(run_window(&gone, before.as_ref(), 0), CbsEvidence::Unknown);
    // Never existed.
    assert_eq!(run_window(&temp("never"), None, 0), CbsEvidence::Unknown);
    // More than the bound was written during the run: the start is not in the window.
    let big = temp("big");
    let start = CbsBaseline::capture(&big);
    append(&big, &"[SR] Verifying 1 components\n".repeat(200_000));
    append(&big, CLEAN_RUN);
    assert_eq!(
        run_window(&big, start.as_ref(), 0),
        CbsEvidence::Unknown,
        "a window past the bound cannot be attributed"
    );
}

#[test]
fn another_log_with_the_same_length_but_a_new_identity_is_a_rotation() {
    let path = temp("replaced");
    append(&path, CLEAN_RUN);
    let baseline = CbsBaseline::capture(&path);
    std::thread::sleep(Duration::from_millis(1100)); // creation times have second granularity on some file systems
    fs::remove_file(&path).expect("remove");
    append(&path, &format!("{CLEAN_RUN}{CLEAN_RUN}"));
    assert_eq!(
        run_window(&path, baseline.as_ref(), 0),
        CbsEvidence::Unknown,
        "a file created after the baseline is not the file the run began in"
    );
}

#[test]
fn an_incomplete_transaction_after_a_complete_one_is_not_clean() {
    assert_eq!(
        classify(&format!("{CLEAN_RUN}{PROGRESS_ONLY}"), 0),
        CbsEvidence::Unknown
    );
}

#[test]
fn unreadable_baseline_is_not_treated_as_a_new_log() {
    let path = temp("unreadable-baseline");
    fs::create_dir(&path).expect("log path is a directory");
    let baseline = CbsBaseline::capture(&path);
    fs::remove_dir(&path).expect("remove directory");
    append(&path, CLEAN_RUN);
    assert_eq!(
        run_window(&path, baseline.as_ref(), 0),
        CbsEvidence::Unknown
    );
}
