use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use cliclack::{confirm, input, intro, log, outro, outro_cancel, select};

const DEFAULT_OWNER: &str = "gawakawa";
const FLAKE_TEMPLATES_PATH: &str = "/home/iota/projects/github.com/gawakawa/flake-templates";

// (name, hint) for the template select prompt.
const TEMPLATES: &[(&str, &str)] = &[
    ("skip", "Do not apply a template"),
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
        .validate(|input: &String| {
            if input.contains('/') {
                Err("Owner must not contain slashes".to_string())
            } else {
                Ok(())
            }
        })
        .interact::<String>()
    else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    let Ok(name) = input("Repository name")
        .validate(|input: &String| {
            if input.contains('/') {
                Err("Name must not contain slashes".to_string())
            } else {
                Ok(())
            }
        })
        .interact::<String>()
    else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    let Ok(visibility) = select("Repository visibility")
        .item("public", "Public", "")
        .item("private", "Private", "")
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

    let result = init_repo(&repo, visibility, template, setup_secrets);

    match result {
        Ok(dir) => outro(format!("Done! Run: cd {}", dir.display()))?,
        Err(err) => outro_cancel(format!("Failed: {err}"))?,
    }

    Ok(())
}

fn init_repo(
    repo: &str,
    visibility: &str,
    template: &str,
    setup_secrets: bool,
) -> io::Result<PathBuf> {
    create_repo(repo, visibility)?;
    let dir = clone_repo(repo)?;

    if setup_secrets {
        set_secrets(repo)?;
    }

    if template != "skip" {
        apply_template(template, &dir)?;
    }

    Ok(dir)
}

fn create_repo(repo: &str, visibility: &str) -> io::Result<()> {
    log::step(format!("Creating repository {repo}"))?;
    run("gh", &["repo", "create", repo, &format!("--{visibility}")])?;
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

fn clone_repo(repo: &str) -> io::Result<PathBuf> {
    log::step(format!("Cloning {repo}"))?;
    run("ghq", &["get", "-p", repo])?;
    let root = capture("ghq", &["root"])?;
    Ok(PathBuf::from(root).join("github.com").join(repo))
}

fn set_secrets(repo: &str) -> io::Result<()> {
    log::step("Setting GitHub Actions secrets")?;

    let app_id = capture("pass", &["github/apps/gawakawa-bot/app-id"])?;
    run(
        "gh",
        &["secret", "set", "BOT_APP_ID", "-b", &app_id, "-R", repo],
    )?;

    let private_key = capture("pass", &["github/apps/gawakawa-bot/private-key"])?;
    run(
        "gh",
        &[
            "secret",
            "set",
            "BOT_PRIVATE_KEY",
            "-b",
            &private_key,
            "-R",
            repo,
        ],
    )?;

    let cachix_token = capture("pass", &["show", "cachix/auth-token"])?;
    run(
        "gh",
        &[
            "secret",
            "set",
            "CACHIX_AUTH_TOKEN",
            "-b",
            &cachix_token,
            "-R",
            repo,
        ],
    )
}

fn apply_template(template: &str, dir: &Path) -> io::Result<()> {
    log::step(format!("Applying template {template}"))?;

    let template_ref = format!("path:{FLAKE_TEMPLATES_PATH}#{template}");
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
    let status = Command::new(program).args(args).status()?;
    check_status(program, args, status)
}

/// Same as `run`, but runs the command inside `dir`.
fn run_in(dir: &Path, program: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program).args(args).current_dir(dir).status()?;
    check_status(program, args, status)
}

/// Runs a command and returns its trimmed stdout.
fn capture(program: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    check_status(program, args, output.status)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn check_status(program: &str, args: &[&str], status: std::process::ExitStatus) -> io::Result<()> {
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "`{program} {}` failed with {status}",
            args.join(" ")
        )))
    }
}
