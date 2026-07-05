use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// Runs a command, inheriting stdio, and fails if it exits non-zero.
pub(crate) fn run(program: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| spawn_error(program, err))?;
    check_status(program, args, status, None)
}

/// Like `run`, but writes `input` to the child's stdin instead of passing it
/// as a command-line argument, so secret values never appear in the process's
/// argument list or in an error message.
pub(crate) fn run_with_stdin(program: &str, args: &[&str], input: &str) -> io::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| spawn_error(program, err))?;
    child
        .stdin
        .take()
        .expect("stdin was requested via Stdio::piped()")
        .write_all(input.as_bytes())?;
    let status = child.wait().map_err(|err| spawn_error(program, err))?;
    check_status(program, args, status, None)
}

/// Same as `run`, but runs the command inside `dir`.
pub(crate) fn run_in(dir: &Path, program: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|err| spawn_error(program, err))?;
    check_status(program, args, status, None)
}

/// Runs a command and returns its trimmed stdout.
pub(crate) fn capture(program: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .output()
        .map_err(|err| spawn_error(program, err))?;
    check_status(program, args, output.status, Some(&output.stderr))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn spawn_error(program: &str, err: io::Error) -> io::Error {
    io::Error::other(format!("failed to run `{program}`: {err}"))
}

/// `stderr` is `Some` only for callers that captured output via `.output()`
/// (see `capture`). Callers that inherit stdio (`run`, `run_with_stdin`,
/// `run_in`) never have a captured buffer to pass, so they always pass `None`.
fn check_status(
    program: &str,
    args: &[&str],
    status: std::process::ExitStatus,
    stderr: Option<&[u8]>,
) -> io::Result<()> {
    if status.success() {
        return Ok(());
    }

    let mut message = format!("`{program} {}` failed with {status}", args.join(" "));
    if let Some(stderr) = stderr.map(|s| String::from_utf8_lossy(s).trim().to_string()) {
        if !stderr.is_empty() {
            message = format!("{message}: {stderr}");
        }
    }
    Err(io::Error::other(message))
}
