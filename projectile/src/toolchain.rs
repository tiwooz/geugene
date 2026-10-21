use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Default)]
pub struct ToolchainStatus {
    pub rustc: Option<String>,
    pub cargo: Option<String>,
    pub rustup: Option<String>,
    pub rustc_path: Option<PathBuf>,
    pub cargo_path: Option<PathBuf>,
    pub rustup_path: Option<PathBuf>,
    /// VS Code (or VSCodium) launcher, for "Open in VS Code".
    pub vscode_path: Option<PathBuf>,
    pub git_path: Option<PathBuf>,
    pub ready: bool,
}

impl ToolchainStatus {
    pub fn summary(&self) -> String {
        match (self.rustc.as_deref(), self.cargo.as_deref()) {
            (Some(_), Some(_)) => "Rust compiler and Cargo detected.".into(),
            (Some(_), None) => "rustc detected; Cargo was not detected.".into(),
            (None, Some(_)) => "Cargo detected; rustc was not detected.".into(),
            (None, None) => "No Rust compiler or Cargo detected.".into(),
        }
    }

    /// `Rust 1.95.0` from `rustc 1.95.0 (hash date)`.
    pub fn short_version(&self) -> Option<String> {
        let version = self.rustc.as_deref()?.split_whitespace().nth(1)?;
        Some(format!("Rust {version}"))
    }
}

pub fn detect() -> ToolchainStatus {
    let rustc_path = find_program("rustc");
    let cargo_path = find_program("cargo");
    let rustup_path = find_program("rustup");
    let rustc = rustc_path.as_deref().and_then(|path| version(path, "--version"));
    let cargo = cargo_path.as_deref().and_then(|path| version(path, "--version"));
    let rustup = rustup_path.as_deref().and_then(|path| version(path, "--version"));
    let vscode_path = ["code", "codium", "code-oss"].iter().find_map(|name| find_program(name));
    let git_path = find_program("git");
    ToolchainStatus { ready: rustc.is_some() && cargo.is_some(), rustc, cargo, rustup, rustc_path, cargo_path, rustup_path, vscode_path, git_path }
}

/// `commit abc123 (+ local changes)` for a git checkout, or `None` when git or the repo is absent.
pub fn source_revision(root: &std::path::Path) -> Option<String> {
    let git = find_program("git")?;
    let commit = run_in(&git, root, &["rev-parse", "--short=12", "HEAD"])?;
    let dirty = run_in(&git, root, &["status", "--porcelain", "--untracked-files=no"]).is_some_and(|status| !status.trim().is_empty());
    Some(if dirty { format!("commit {commit} + local changes") } else { format!("commit {commit}") })
}

fn run_in(program: &std::path::Path, dir: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = Command::new(program).current_dir(dir).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok().map(|s| s.trim().to_owned())
}

/// A program on PATH, or in Cargo's bin folder (`$CARGO_HOME/bin`, `~/.cargo/bin`), where rustup
/// puts `rustup`, `cargo` and `rustc` even when the shell profile was not updated.
pub fn find_program(program: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    let names = [format!("{program}.exe"), format!("{program}.cmd"), format!("{program}.bat"), program.to_owned()];
    #[cfg(not(windows))]
    let names = [program.to_owned()];
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    if let Some(cargo_home) = std::env::var_os("CARGO_HOME") {
        directories.push(PathBuf::from(cargo_home).join("bin"));
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        directories.push(PathBuf::from(home).join(".cargo").join("bin"));
    }
    directories.iter().flat_map(|directory| names.iter().map(move |name| directory.join(name))).find(|candidate| candidate.is_file())
}

fn version(program: &std::path::Path, argument: &str) -> Option<String> {
    let output = Command::new(program).arg(argument).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let version = stdout.trim();
    (!version.is_empty()).then(|| version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::ToolchainStatus;

    #[test]
    fn toolchain_summary_distinguishes_missing_tools() {
        let status = ToolchainStatus::default();
        assert_eq!(status.summary(), "No Rust compiler or Cargo detected.");
        let partial = ToolchainStatus { rustc: Some("rustc 1.x".into()), ..ToolchainStatus::default() };
        assert_eq!(partial.summary(), "rustc detected; Cargo was not detected.");
    }
}
