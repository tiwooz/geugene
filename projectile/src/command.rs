//! External commands ProjectIle runs for you (rustup, the docs download, git, gh, cargo build):
//! each is a [`Task`] that is shown in full, confirmed when it changes something outside the app,
//! run on a worker thread, and streamed line by line into a log.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where a task's output goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Log {
    /// The Rust tab's rustup output.
    Rust,
    /// The Create tab's output log.
    Main,
}

/// What happens after a task succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Then {
    Nothing,
    /// Start this program (detached), e.g. the editor that was just built.
    Launch(PathBuf),
    /// Run another task (for example `git add` then `git commit`).
    Task(Box<Task>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub title: String,
    pub explanation: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub log: Log,
    pub then: Then,
}

impl Task {
    /// The command as a user would type it.
    pub fn command_text(&self) -> String {
        // Chained tasks are already part of `command_text_without_cd`.
        let next = match &self.then {
            Then::Launch(program) => format!(" && {}", program.display()),
            Then::Task(_) | Then::Nothing => String::new(),
        };
        let own = self.command_text_without_cd();
        match &self.cwd {
            Some(dir) => format!("cd \"{}\" && {own}{next}", dir.display()),
            None => format!("{own}{next}"),
        }
    }

    fn command_text_without_cd(&self) -> String {
        if self.args.first().is_some_and(|a| a == "-c") && self.program.file_name().is_some_and(|n| n == "sh") {
            return self.args.get(1).cloned().unwrap_or_default();
        }
        let name = self.program.file_name().map_or_else(|| self.program.display().to_string(), |n| n.to_string_lossy().into_owned());
        let args: Vec<String> = self.args.iter().map(|a| if a.contains(' ') || a.is_empty() { format!("\"{a}\"") } else { a.clone() }).collect();
        let own = format!("{name} {}", args.join(" "));
        match &self.then {
            Then::Task(next) => format!("{own} && {}", next.command_text_without_cd()),
            _ => own,
        }
    }
}

/// Run `task`, streaming each output line to `line`. Blocking: call from a worker thread.
pub fn run(task: &Task, line: &mut dyn FnMut(String)) -> Result<(), String> {
    line(format!("$ {}", task.command_text()));
    let mut command = Command::new(&task.program);
    command.args(&task.args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = &task.cwd {
        command.current_dir(dir);
    }
    let mut child = command.spawn().map_err(|error| format!("could not start {}: {error}", task.program.display()))?;
    let stdout = child.stdout.take();
    let out_thread = std::thread::Builder::new()
        .name("projectile-task-stdout".into())
        .spawn(move || stdout.map(|out| BufReader::new(out).lines().map_while(Result::ok).collect::<Vec<_>>()).unwrap_or_default())
        .map_err(|error| format!("could not read output: {error}"))?;
    if let Some(stderr) = child.stderr.take() {
        for text in BufReader::new(stderr).split(b'\n').map_while(Result::ok) {
            // Progress bars redraw with \r; keep only the last state of the line.
            let text = String::from_utf8_lossy(&text);
            let last = text.rsplit('\r').find(|part| !part.trim().is_empty()).unwrap_or_default().trim().to_owned();
            if !last.is_empty() {
                line(last);
            }
        }
    }
    for text in out_thread.join().unwrap_or_default() {
        line(text);
    }
    let status = child.wait().map_err(|error| format!("could not wait for {}: {error}", task.program.display()))?;
    if !status.success() {
        return Err(format!("{} failed ({status}).", task.title));
    }
    match &task.then {
        Then::Launch(program) => {
            launch(program, task.cwd.as_deref())?;
            line(format!("Started {}.", program.display()));
        }
        Then::Task(next) => run(next, line)?,
        Then::Nothing => {}
    }
    Ok(())
}

/// Start a program without waiting for it; a helper thread reaps it when it exits.
pub fn launch(program: &Path, cwd: Option<&Path>) -> Result<(), String> {
    let mut command = Command::new(program);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let mut child = command.spawn().map_err(|error| format!("could not start {}: {error}", program.display()))?;
    std::thread::Builder::new()
        .name("projectile-reap".into())
        .spawn(move || child.wait())
        .map_err(|error| format!("could not watch {}: {error}", program.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_text_quotes_and_shows_the_folder() {
        let task = Task {
            title: "Commit".into(),
            explanation: String::new(),
            program: PathBuf::from("/usr/bin/git"),
            args: vec!["commit".into(), "-m".into(), "first try".into()],
            cwd: Some(PathBuf::from("/tmp/p")),
            log: Log::Main,
            then: Then::Nothing,
        };
        assert_eq!(task.command_text(), "cd \"/tmp/p\" && git commit -m \"first try\"");
    }

    #[test]
    fn failing_and_missing_programs_are_errors() -> Result<(), String> {
        let missing = Task {
            title: "Missing".into(),
            explanation: String::new(),
            program: PathBuf::from("/definitely/not/a/program"),
            args: Vec::new(),
            cwd: None,
            log: Log::Main,
            then: Then::Nothing,
        };
        assert!(run(&missing, &mut |_| {}).is_err());
        if let Some(sh) = crate::toolchain::find_program("sh") {
            let mut lines = Vec::new();
            let ok = Task { program: sh.clone(), args: vec!["-c".into(), "echo hello; echo oops 1>&2".into()], ..missing.clone() };
            run(&ok, &mut |l| lines.push(l))?;
            if !lines.iter().any(|l| l == "hello") || !lines.iter().any(|l| l == "oops") {
                return Err(format!("output not streamed: {lines:?}"));
            }
            let fail = Task { program: sh, args: vec!["-c".into(), "exit 3".into()], ..missing };
            assert!(run(&fail, &mut |_| {}).is_err());
        }
        Ok(())
    }
}
