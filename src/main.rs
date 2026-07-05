use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cliclack::{confirm, input, intro, log, outro, outro_cancel, select};

const DEFAULT_OWNER: &str = "gawakawa";
const FLAKE_TEMPLATES_REPO: &str = "gawakawa/flake-templates";
const SKIP_TEMPLATE: &str = "skip";

// (name, hint) for the template select prompt.
const TEMPLATES: &[(&str, &str)] = &[
    (SKIP_TEMPLATE, "Do not apply a template"),
    ("crane", "Rust template, using crane"),
    ("crane-workspace", "Rust workspace template, using crane"),
    ("deno", "Deno template"),
    ("flake-parts", "Modular flake with flake-parts"),
    ("go", "Go template"),
    ("haskell", "Haskell template, using haskell.nix and hix"),
    ("idris2", "Idris2 template"),
    ("lean", "Lean theorem prover template, using elan"),
    ("pack", "Idris2 template, using pack"),
    ("pnpm", "Node.js template, using pnpm"),
    ("purs-nix", "PureScript template, using purs-nix"),
    ("python", "Python template, using uv"),
    ("rust-overlay", "Rust template, using rust-overlay"),
    ("rustup", "Rust template, using rustup"),
    ("terraform", "Terraform template"),
    ("uv2nix", "Python template, using uv2nix"),
];

fn main() -> io::Result<()> {
    intro("init-env")?;

    let Ok(owner) = input("GitHub owner")
        .default_input(DEFAULT_OWNER)
        .validate(|input: &String| no_slashes(input, "Owner"))
        .interact::<String>()
    else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    let Ok(name) = input("Repository name")
        .validate(|input: &String| no_slashes(input, "Name"))
        .interact::<String>()
    else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    let Ok(visibility) = select("Repository visibility")
        .item("--public", "Public", "")
        .item("--private", "Private", "")
        .interact()
    else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    let mut template_prompt = select("Flake template");
    for (name, hint) in TEMPLATES {
        template_prompt = template_prompt.item(*name, *name, *hint);
    }
    let Ok(template) = template_prompt.interact() else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    let Ok(setup_secrets) = confirm("Set up GitHub Actions secrets?")
        .initial_value(false)
        .interact()
    else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    let repo = format!("{owner}/{name}");
    let template = (template != SKIP_TEMPLATE).then_some(template);

    match init_repo(&repo, visibility, template, setup_secrets) {
        Ok(dir) => outro(format!("Done! Run: cd {}", dir.display()))?,
        Err(err) => outro_cancel(format!("Failed: {err}"))?,
    }

    Ok(())
}

fn no_slashes(value: &str, field: &str) -> Result<(), String> {
    if value.contains('/') {
        Err(format!("{field} must not contain slashes"))
    } else {
        Ok(())
    }
}

fn init_repo(
    repo: &str,
    visibility: &str,
    template: Option<&str>,
    setup_secrets: bool,
) -> io::Result<PathBuf> {
    let dir = ghq_path(repo)?;
    if dir.exists() {
        return Err(io::Error::other(format!(
            "{} already exists; remove it before retrying",
            dir.display()
        )));
    }

    create_repo(repo, visibility)?;
    clone_repo(repo)?;

    if setup_secrets {
        set_secrets(repo)?;
    }

    if let Some(template) = template {
        apply_template(template, &dir)?;
    }

    Ok(dir)
}

fn create_repo(repo: &str, visibility: &str) -> io::Result<()> {
    log::step(format!("Creating repository {repo}"))?;
    run("gh", &["repo", "create", repo, visibility])?;
    run(
        "gh",
        &[
            "repo",
            "edit",
            repo,
            "--enable-auto-merge",
            "--delete-branch-on-merge",
            "--allow-update-branch",
        ],
    )
}

fn ghq_root() -> io::Result<PathBuf> {
    Ok(PathBuf::from(capture("ghq", &["root"])?))
}

/// Resolves `owner/repo` to its ghq clone path: `<ghq root>/github.com/owner/repo`.
fn ghq_path(repo: &str) -> io::Result<PathBuf> {
    Ok(ghq_root()?.join("github.com").join(repo))
}

fn clone_repo(repo: &str) -> io::Result<()> {
    log::step(format!("Cloning {repo}"))?;
    run("ghq", &["get", "-p", repo])
}

const SECRETS: &[(&str, &str)] = &[
    ("BOT_APP_ID", "github/apps/gawakawa-bot/app-id"),
    ("BOT_PRIVATE_KEY", "github/apps/gawakawa-bot/private-key"),
    ("CACHIX_AUTH_TOKEN", "cachix/auth-token"),
];

fn set_secrets(repo: &str) -> io::Result<()> {
    log::step("Setting GitHub Actions secrets")?;

    for (name, pass_path) in SECRETS {
        let value = capture("pass", &["show", pass_path])?;
        run_with_stdin("gh", &["secret", "set", name, "-R", repo], &value)?;
    }

    Ok(())
}

fn apply_template(template: &str, dir: &Path) -> io::Result<()> {
    log::step(format!("Applying template {template}"))?;

    let templates_path = ghq_path(FLAKE_TEMPLATES_REPO)?;
    let template_ref = format!("path:{}#{template}", templates_path.display());
    run_in(dir, "nix", &["flake", "init", "-t", &template_ref])?;
    run_in(dir, "git", &["add", "-A"])?;
    run_in(
        dir,
        "git",
        &[
            "commit",
            "-m",
            &format!(":tada: Initialize from {template} template"),
        ],
    )?;
    run_in(dir, "git", &["push", "-u", "origin", "HEAD"])
}

/// Runs a command, inheriting stdio, and fails if it exits non-zero.
fn run(program: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| spawn_error(program, err))?;
    check_status(program, args, status, None)
}

/// Like `run`, but writes `input` to the child's stdin instead of passing it
/// as a command-line argument, so secret values never appear in the process's
/// argument list or in an error message.
fn run_with_stdin(program: &str, args: &[&str], input: &str) -> io::Result<()> {
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
fn run_in(dir: &Path, program: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|err| spawn_error(program, err))?;
    check_status(program, args, status, None)
}

/// Runs a command and returns its trimmed stdout.
fn capture(program: &str, args: &[&str]) -> io::Result<String> {
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
