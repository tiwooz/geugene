//! Git and GitHub for a generated project: its status, and the tasks that commit, publish
//! (`gh repo create --push`) and push it. Publishing is outward-facing, so it is always confirmed.

use crate::command::{Log, Task, Then};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct GitState {
    /// The project these facts are about.
    pub project: Option<PathBuf>,
    pub git: Option<PathBuf>,
    pub gh: Option<PathBuf>,
    pub repo: bool,
    pub branch: Option<String>,
    /// Changed or untracked files (`git status --porcelain` lines).
    pub changes: usize,
    pub commits: bool,
    pub remote: Option<String>,
    /// The GitHub account `gh` is signed in to.
    pub gh_user: Option<String>,
}

fn output(program: &Path, dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(program).current_dir(dir).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

pub fn scan(project: Option<&Path>) -> GitState {
    let git = crate::toolchain::find_program("git");
    let gh = crate::toolchain::find_program("gh");
    let mut state = GitState { project: project.map(Path::to_path_buf), git: git.clone(), gh: gh.clone(), ..GitState::default() };
    if let Some(gh) = &gh {
        // `gh auth status` prints "Logged in to github.com account NAME (…)" to stderr.
        if let Ok(out) = Command::new(gh).args(["auth", "status", "--hostname", "github.com"]).output() {
            let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            state.gh_user = text.lines().find_map(|line| line.split(" account ").nth(1)).and_then(|rest| rest.split_whitespace().next()).map(str::to_owned);
        }
    }
    let (Some(git), Some(dir)) = (git, project.filter(|p| p.is_dir())) else { return state };
    state.repo = dir.join(".git").exists();
    if state.repo {
        state.branch = output(&git, dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
        state.commits = output(&git, dir, &["rev-parse", "--verify", "HEAD"]).is_some();
        state.changes = output(&git, dir, &["status", "--porcelain"]).map_or(0, |s| s.lines().filter(|l| !l.trim().is_empty()).count());
        state.remote = output(&git, dir, &["remote", "get-url", "origin"]);
    }
    state
}

pub fn valid_message(message: &str) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("Write a short commit message first.".into());
    }
    if message.len() > 2000 || message.contains('\0') {
        return Err("Keep the commit message under 2000 characters.".into());
    }
    Ok(message.to_owned())
}

/// GitHub repository names: letters, digits, `-`, `_` and `.`, at most 100 characters.
pub fn valid_repo_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    let ok = !name.is_empty()
        && name.len() <= 100
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && name != "."
        && name != "..";
    if ok { Ok(name.to_owned()) } else { Err("A GitHub repository name uses letters, digits, -, _ and . only.".into()) }
}

fn task(title: &str, explanation: &str, program: &Path, args: Vec<String>, dir: &Path, then: Then) -> Task {
    Task { title: title.into(), explanation: explanation.into(), program: program.to_path_buf(), args, cwd: Some(dir.to_path_buf()), log: Log::Main, then }
}

/// `git init` (when needed), `git add -A`, then `git commit -m <message>` as one chained task.
pub fn commit(state: &GitState, message: &str) -> Result<Task, String> {
    let git = state.git.as_deref().ok_or("git is not installed.")?;
    let dir = state.project.as_deref().ok_or("Generate a project first.")?;
    let message = valid_message(message)?;
    let commit = task("Commit", "", git, vec!["commit".into(), "-m".into(), message], dir, Then::Nothing);
    let add = task("Stage all changes", "", git, vec!["add".into(), "-A".into()], dir, Then::Task(Box::new(commit)));
    let first = if state.repo { add } else { task("Start a git repository", "", git, vec!["init".into(), "-q".into()], dir, Then::Task(Box::new(add))) };
    Ok(Task {
        title: "Commit the project".into(),
        explanation: format!(
            "Records the current state of every file in {} as a commit in its local git history. Nothing leaves your computer.",
            dir.display()
        ),
        ..first
    })
}

/// Create a GitHub repository from the project and push it (`gh repo create … --source . --push`).
pub fn publish(state: &GitState, name: &str, private: bool) -> Result<Task, String> {
    let gh = state.gh.as_deref().ok_or("The GitHub CLI (gh) is not installed.")?;
    let dir = state.project.as_deref().ok_or("Generate a project first.")?;
    if state.gh_user.is_none() {
        return Err("Sign in to GitHub first (gh auth login).".into());
    }
    if !state.commits {
        return Err("Commit the project first; GitHub needs at least one commit.".into());
    }
    let name = valid_repo_name(name)?;
    let visibility = if private { "--private" } else { "--public" };
    Ok(task(
        "Publish to GitHub",
        &format!(
            "Creates the {} repository {name} on your GitHub account ({}) and uploads the project's commits. {}",
            if private { "private" } else { "public" },
            state.gh_user.as_deref().unwrap_or("?"),
            if private { "Only you (and people you invite) can see it." } else { "Anyone on the internet can see it." }
        ),
        gh,
        vec!["repo".into(), "create".into(), name, visibility.into(), "--source".into(), ".".into(), "--remote".into(), "origin".into(), "--push".into()],
        dir,
        Then::Nothing,
    ))
}

/// `git push -u origin HEAD` for a project that already has a remote.
pub fn push(state: &GitState) -> Result<Task, String> {
    let git = state.git.as_deref().ok_or("git is not installed.")?;
    let dir = state.project.as_deref().ok_or("Generate a project first.")?;
    let remote = state.remote.as_deref().ok_or("This project has no remote yet; publish it first.")?;
    Ok(task(
        "Push to GitHub",
        &format!("Uploads the project's new commits to {remote}."),
        git,
        vec!["push".into(), "-u".into(), "origin".into(), "HEAD".into()],
        dir,
        Then::Nothing,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_messages_are_checked() {
        assert!(valid_message("  ").is_err());
        assert_eq!(valid_message(" first ").as_deref(), Ok("first"));
        assert!(valid_repo_name("my-photocore").is_ok());
        for bad in ["", "a b", "../x", "x;rm", ".."] {
            assert!(valid_repo_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn publishing_needs_sign_in_and_a_commit() {
        let state = GitState { gh: Some("/usr/bin/gh".into()), project: Some("/tmp/x".into()), commits: true, ..GitState::default() };
        assert!(publish(&state, "x", true).is_err(), "not signed in");
        let signed = GitState { gh_user: Some("someone".into()), ..state.clone() };
        assert!(publish(&GitState { commits: false, ..signed.clone() }, "x", true).is_err(), "no commit yet");
        let task = publish(&signed, "x", true).unwrap_or_else(|_| Task {
            title: String::new(),
            explanation: String::new(),
            program: PathBuf::new(),
            args: Vec::new(),
            cwd: None,
            log: Log::Main,
            then: Then::Nothing,
        });
        assert!(task.args.contains(&"--private".to_owned()) && task.explanation.contains("private"));
    }

    #[test]
    fn commit_chains_init_add_and_commit() -> Result<(), String> {
        let state = GitState { git: Some("/usr/bin/git".into()), project: Some("/tmp/x".into()), repo: false, ..GitState::default() };
        let first = commit(&state, "hello")?;
        let Then::Task(add) = &first.then else { return Err("init should be followed by add".into()) };
        let Then::Task(last) = &add.then else { return Err("add should be followed by commit".into()) };
        if first.args != ["init", "-q"] || last.args != ["commit", "-m", "hello"] {
            return Err(format!("unexpected chain: {first:?}"));
        }
        Ok(())
    }
}
