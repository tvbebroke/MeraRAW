//! Subprocess execution with a hard deadline.
//!
//! Workers are external binaries that can hang in ways we've observed first
//! hand (e.g. rawtherapee-cli deadlocking in its App Sandbox initializer
//! before `main`). `Command::output()` would block the calling thread
//! forever; this runner kills the child at the deadline instead, so a broken
//! worker degrades into an error the caller can fall back from.

use crate::Error;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Run `cmd` to completion or kill it at `timeout`. Stdout/stderr are drained
/// on background threads so a chatty child can never block on a full pipe.
pub(crate) fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<Output, Error> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| Error::Exec(e.to_string()))?;
    let out_h = drain(child.stdout.take());
    let err_h = drain(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|e| Error::Exec(e.to_string()))? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait(); // reap the zombie
                let stderr = err_h.join().unwrap_or_default();
                return Err(Error::Exec(format!(
                    "worker timed out after {timeout:?} and was killed{}",
                    stderr_tail(&stderr)
                )));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    Ok(Output {
        status,
        stdout: out_h.join().unwrap_or_default(),
        stderr: err_h.join().unwrap_or_default(),
    })
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    })
}

fn stderr_tail(stderr: &[u8]) -> String {
    if stderr.is_empty() {
        return String::new();
    }
    let s = String::from_utf8_lossy(stderr);
    let tail: String = s.chars().rev().take(400).collect::<String>().chars().rev().collect();
    format!("; stderr tail: {tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_within_deadline() {
        let mut cmd = Command::new("/bin/echo");
        cmd.arg("ok");
        let out = run_with_timeout(&mut cmd, Duration::from_secs(5)).unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
    }

    #[test]
    fn kills_at_deadline() {
        let mut cmd = Command::new("/bin/sleep");
        cmd.arg("30");
        let started = Instant::now();
        let err = run_with_timeout(&mut cmd, Duration::from_millis(300)).unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(5), "kill was not prompt");
        assert!(err.to_string().contains("timed out"), "unexpected error: {err}");
    }
}
