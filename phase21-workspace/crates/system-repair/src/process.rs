//! Bounded command output; a servicing mutation keeps its lease until its own process exits.
use crate::{RepairCheck, RepairError, Result};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn run_tool(
    exe: &Path,
    args: &[&str],
    identity: (&str, &str, &str),
    accepted_codes: &[i32],
    cancel: Option<&AtomicBool>,
    mutating: bool,
    timeout: Duration,
) -> Result<RepairCheck> {
    let (id, title, log_hint) = identity;
    if !exe.is_absolute() || !exe.is_file() {
        return Err(RepairError::Command(format!(
            "required Windows executable missing: {}",
            exe.display()
        )));
    }

    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|error| RepairError::Command(error.to_string()))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_thread = match thread::Builder::new()
        .name("aether-repair-stdout".into())
        .spawn(move || read_tail(stdout, 32_768))
    {
        Ok(worker) => worker,
        Err(error) => {
            if !mutating {
                let _ = child.kill();
            }
            let _ = wait_for_child(&mut child);
            return Err(RepairError::Command(format!(
                "failed to create repair stdout reader: {error}"
            )));
        }
    };
    let stderr_thread = match thread::Builder::new()
        .name("aether-repair-stderr".into())
        .spawn(move || read_tail(stderr, 16_384))
    {
        Ok(worker) => worker,
        Err(error) => {
            if !mutating {
                let _ = child.kill();
            }
            let _ = wait_for_child(&mut child);
            let _ = stdout_thread.join();
            return Err(RepairError::Command(format!(
                "failed to create repair stderr reader: {error}"
            )));
        }
    };
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut stopped = false;

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                // Keep ownership until this exact child exits, even if polling its handle failed.
                let _ = wait_for_child(&mut child);
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(RepairError::Command(error.to_string()));
            }
        }
        timed_out |= Instant::now() >= deadline;
        stopped |= cancel.is_some_and(|flag| flag.load(Ordering::SeqCst));
        if !mutating && timed_out {
            let _ = child.kill();
            let _ = wait_for_child(&mut child);
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            return Err(RepairError::Command(format!("{title} timed out")));
        }
        if !mutating && stopped {
            let _ = child.kill();
            let _ = wait_for_child(&mut child);
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            return Err(RepairError::Cancelled);
        }
        thread::sleep(Duration::from_millis(25));
    };

    let mut notes = Vec::new();
    let mut detail = super::joined_stream(stdout_thread.join(), "stdout", &mut notes);
    let error_output = super::joined_stream(stderr_thread.join(), "stderr", &mut notes);
    if !error_output.trim().is_empty() {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&error_output);
    }
    super::trim_to_tail(&mut detail, 48_000);
    // After truncation, so a lost stream is never itself truncated away.
    for note in notes {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&note);
    }

    if mutating {
        if stopped || cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            return Err(RepairError::RepairStopped);
        }
        if timed_out || Instant::now() >= deadline {
            return Err(RepairError::RepairTimedOut);
        }
    }
    let code = status.code().unwrap_or(-1);
    if !accepted_codes.contains(&code) {
        return Err(RepairError::Command(format!(
            "{title} exited with code {code}; see {log_hint}"
        )));
    }

    Ok(RepairCheck {
        id: id.into(),
        title: title.into(),
        stage: "Completed".into(),
        result_code: format!("ExitCode{code}"),
        exit_code: code,
        detail: detail
            .chars()
            .filter(|c| *c == '\n' || *c == '\r' || *c == '\t' || !c.is_control())
            .collect(),
        log_hint: log_hint.into(),
    })
}

fn wait_for_child(child: &mut std::process::Child) -> std::process::ExitStatus {
    // A wait error is not an exit acknowledgement. Keep this owned handle/admission until exit
    // is actually observed; cancellation and elapsed deadlines cannot turn uncertainty into idle.
    loop {
        match child.wait() {
            Ok(status) => return status,
            Err(_) => thread::sleep(Duration::from_millis(25)),
        }
    }
}

fn read_tail<R: Read>(reader: Option<R>, limit: usize) -> String {
    let Some(mut reader) = reader else {
        return String::new();
    };
    let mut tail = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                tail.extend_from_slice(&chunk[..n]);
                if tail.len() > limit {
                    tail.drain(..tail.len() - limit);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return "Command output could not be read.".into(),
        }
    }
    // SFC commonly redirects UTF-16LE. Preserve code units across pipe chunks, then decode once.
    if tail.starts_with(&[0xff, 0xfe]) || (tail.len() > 2 && tail[1] == 0) {
        let start = usize::from(tail.starts_with(&[0xff, 0xfe])) * 2;
        String::from_utf16_lossy(
            &tail[start..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(&tail).into_owned()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn cancellation_and_timeout_wait_for_mutation_exit_but_never_report_success() {
        for cancelled in [false, true] {
            let flag = AtomicBool::new(cancelled);
            let started = Instant::now();
            let result = run_tool(
                Path::new("/bin/sh"),
                &["-c", "sleep 0.15"],
                ("fixture", "fixture", "fixture"),
                &[0],
                Some(&flag),
                true,
                Duration::from_millis(30),
            );
            assert!(
                started.elapsed() >= Duration::from_millis(140),
                "the servicing process was killed"
            );
            if cancelled {
                assert!(matches!(result, Err(RepairError::RepairStopped)));
            } else {
                assert!(matches!(result, Err(RepairError::RepairTimedOut)));
            }
        }
    }
    #[test]
    fn streams_are_bounded_while_reading_and_preserve_the_tail() {
        let mut input = &b"0123456789"[..];
        assert_eq!(read_tail(Some(&mut input), 4), "6789");
    }
}
