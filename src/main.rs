use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use cliclack::{confirm, input, intro, log, outro, outro_cancel, outro_note, select, spinner};

mod exec;
use exec::{capture, run, run_in, run_with_stdin};

const DEFAULT_OWNER: &str = "gawakawa";
const FLAKE_TEMPLATES_REF: &str = "github:gawakawa/flake-templates";
const SKIP_TEMPLATE: &str = "skip";
const GITHUB_APP_INSTALL_URL: &str = "https://github.com/settings/installations/127190964";

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

    let spinner = spinner();
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
        select("Flake template").item(SKIP_TEMPLATE, SKIP_TEMPLATE, "Do not apply a template");
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

    let Ok(setup_branch_rules) = confirm("Set up branch rules?")
        .initial_value(template != SKIP_TEMPLATE)
        .interact()
    else {
        outro_cancel("Cancelled")?;
        return Ok(());
    };

    let repo = format!("{owner}/{name}");
    let template = (template != SKIP_TEMPLATE).then_some(template);

    match init_repo(
        &repo,
        visibility,
        template,
        setup_secrets,
        setup_branch_rules,
    ) {
        Ok(dir) => {
            let done = format!("Done! Run: cd {}", dir.display());
            if setup_secrets {
                outro_note(
                    done,
                    format!("Add this repository to the GitHub App:\n{GITHUB_APP_INSTALL_URL}"),
                )?;
            } else {
                outro(done)?;
            }
        }
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
    setup_branch_rules: bool,
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

    if setup_branch_rules {
        set_branch_rules(repo)?;
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
    let templates: BTreeMap<String, String> = serde_json::from_str(&json)
        .map_err(|err| io::Error::other(format!("failed to parse templates output: {err}")))?;
    Ok(templates.into_iter().collect())
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

const BRANCH_RULES_JSON: &str = r#"{
  "name": "main",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] }
  },
  "bypass_actors": [
    { "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" }
  ],
  "rules": [
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": false,
        "do_not_enforce_on_create": false,
        "required_status_checks": [{ "context": "ci-success" }]
      }
    }
  ]
}"#;

fn set_branch_rules(repo: &str) -> io::Result<()> {
    log::step("Setting up branch rules")?;
    run_with_stdin(
        "gh",
        &[
            "api",
            "-X",
            "POST",
            &format!("repos/{repo}/rulesets"),
            "--input",
            "-",
        ],
        BRANCH_RULES_JSON,
    )
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
