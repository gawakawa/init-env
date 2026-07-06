use std::io;
use std::path::{Path, PathBuf};

use cliclack::{confirm, input, intro, log, outro, outro_cancel, select};

mod exec;
use exec::{capture, run, run_in, run_with_stdin};

const DEFAULT_OWNER: &str = "gawakawa";
const FLAKE_TEMPLATES_REF: &str = "github:gawakawa/flake-templates";
const SKIP_TEMPLATE: &str = "skip";

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

    let spinner = cliclack::spinner();
    spinner.start("Fetching templates");
    let templates = match fetch_templates() {
        Ok(templates) => {
            spinner.stop("Fetched templates");
            templates
        }
        Err(err) => {
            spinner.error("Failed to fetch templates");
            outro_cancel(format!("Failed: {err}"))?;
            return Ok(());
        }
    };

    let mut template_prompt =
        select("Flake template").item(SKIP_TEMPLATE, "skip", "Do not apply a template");
    for (name, hint) in &templates {
        template_prompt = template_prompt.item(name.as_str(), name.as_str(), hint.as_str());
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

/// Fetches the (name, description) list of flake templates from
/// `FLAKE_TEMPLATES_REF`, sorted alphabetically by name.
fn fetch_templates() -> io::Result<Vec<(String, String)>> {
    let json = capture(
        "nix",
        &[
            "eval",
            "--json",
            &format!("{FLAKE_TEMPLATES_REF}#templates"),
            "--apply",
            r#"builtins.mapAttrs (_: t: t.description or "")"#,
        ],
    )?;
    let value: serde_json::Value = serde_json::from_str(&json).map_err(io::Error::other)?;
    let obj = value
        .as_object()
        .ok_or_else(|| io::Error::other("unexpected templates output"))?;
    Ok(obj
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect())
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

    let template_ref = format!("{FLAKE_TEMPLATES_REF}#{template}");
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
