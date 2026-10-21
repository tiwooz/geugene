//! Rust toolchains for the Rust tab: what is installed (via rustup), which stable releases exist,
//! and the rustup commands that install, switch and remove them.
//!
//! Releases are derived from Rust's fixed six-week schedule (1.1 shipped 2015-06-25, then one
//! minor version every 42 days), so the list needs no network. rustup does the real work and
//! reports any version that does not exist.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Days from 1970-01-01 to 2015-06-25, the Rust 1.1 release.
const RUST_1_1_DAYS: i64 = 16_611;
const CYCLE_DAYS: i64 = 42;
/// Oldest release listed; older compilers cannot build PhotoCraft anyway.
pub const OLDEST_LISTED_MINOR: u32 = 70;
pub const CHANNELS: [(&str, &str); 3] = [
    ("stable", "The latest release, updated every six weeks"),
    ("beta", "The next release, for testing"),
    ("nightly", "Built every night; needed for some experimental features"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// Full rustup name, e.g. `1.95.0-x86_64-unknown-linux-gnu` or `stable-x86_64-…`.
    pub name: String,
    pub is_default: bool,
    pub is_active: bool,
}

impl Installed {
    /// `1.95.0`, `stable`, `nightly-2026-10-01`: the name without the host triple.
    pub fn short(&self) -> &str {
        short_name(&self.name)
    }
}

pub fn short_name(name: &str) -> &str {
    for arch in ["-x86_64-", "-aarch64-", "-i686-", "-armv7-", "-arm-", "-riscv64", "-powerpc64", "-s390x-", "-loongarch64-"] {
        if let Some(index) = name.find(arch) {
            return name.get(..index).unwrap_or(name);
        }
    }
    name
}

#[derive(Debug, Clone, Default)]
pub struct RustState {
    pub rustup: Option<PathBuf>,
    pub installed: Vec<Installed>,
    /// `rustc --version` of whatever `rustc` runs by default.
    pub active_rustc: Option<String>,
    pub rustc_path: Option<PathBuf>,
    /// Set when rustup could not list toolchains.
    pub error: Option<String>,
    /// The offline Rust book, when installed.
    pub offline_book: Option<PathBuf>,
    /// The compiler's host platform, e.g. `x86_64-unknown-linux-gnu`.
    pub host: Option<String>,
}

impl RustState {
    pub fn is_installed(&self, short: &str) -> bool {
        self.installed.iter().any(|t| t.short() == short)
    }

    pub fn default_short(&self) -> Option<&str> {
        self.installed.iter().find(|t| t.is_default).map(Installed::short)
    }

    /// Version number of the active compiler, e.g. `1.99.0`.
    pub fn active_version(&self) -> Option<&str> {
        self.active_rustc.as_deref()?.split_whitespace().nth(1)
    }

    /// Where the active compiler comes from, for people who never heard of rustup.
    pub fn origin(&self) -> String {
        match (&self.rustup, &self.rustc_path) {
            (Some(_), Some(path)) if path.to_string_lossy().contains(".cargo") || path.to_string_lossy().contains(".rustup") => "managed by rustup".into(),
            (_, Some(path)) => format!("system install at {}", path.display()),
            (_, None) => "not installed".into(),
        }
    }
}

pub fn scan(rustup: Option<PathBuf>, rustc: Option<String>, rustc_path: Option<PathBuf>) -> RustState {
    let offline_book = offline_book(rustc_path.as_deref());
    let host = rustc_path
        .as_deref()
        .and_then(|rustc| Command::new(rustc).arg("-vV").output().ok())
        .and_then(|output| String::from_utf8_lossy(&output.stdout).lines().find_map(|line| line.strip_prefix("host: ").map(|h| h.trim().to_owned())));
    let mut state = RustState { rustup: rustup.clone(), active_rustc: rustc, rustc_path, offline_book, host, ..RustState::default() };
    if let Some(rustup) = rustup {
        match Command::new(&rustup).args(["toolchain", "list"]).output() {
            Ok(output) if output.status.success() => state.installed = parse_toolchain_list(&String::from_utf8_lossy(&output.stdout)),
            Ok(output) => state.error = Some(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
            Err(error) => state.error = Some(format!("could not run rustup: {error}")),
        }
    }
    state
}

/// Parses `rustup toolchain list`: `stable-x86_64-unknown-linux-gnu (active, default)`.
pub fn parse_toolchain_list(text: &str) -> Vec<Installed> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("no installed toolchains"))
        .map(|line| {
            let (name, flags) = match line.split_once(' ') {
                Some((name, flags)) => (name, flags),
                None => (line, ""),
            };
            Installed { name: name.to_owned(), is_default: flags.contains("default"), is_active: flags.contains("active") }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Release {
    pub minor: u32,
    pub date: (i64, u32, u32),
}

impl Release {
    pub fn version(&self) -> String {
        format!("1.{}.0", self.minor)
    }

    pub fn date_text(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.date.0, self.date.1, self.date.2)
    }
}

/// Scheduled release day of `1.minor.0` (days since the Unix epoch).
pub fn release_day(minor: u32) -> i64 {
    RUST_1_1_DAYS + (i64::from(minor.max(1)) - 1) * CYCLE_DAYS
}

/// Stable releases already out on `today` (days since the epoch), newest first.
pub fn releases(today: i64) -> Vec<Release> {
    let latest = if today < RUST_1_1_DAYS { 1 } else { ((today - RUST_1_1_DAYS) / CYCLE_DAYS + 1).clamp(1, 999) as u32 };
    (OLDEST_LISTED_MINOR.min(latest)..=latest).rev().map(|minor| Release { minor, date: civil_from_days(release_day(minor)) }).collect()
}

pub fn today() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(RUST_1_1_DAYS, |d| (d.as_secs() / 86_400) as i64)
}

/// Gregorian date of a day count (Howard Hinnant's algorithm).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// `1.95` from a `rust-version` (or `1.95.0`), as a comparable minor number.
pub fn minor_of(version: &str) -> Option<u32> {
    let mut parts = version.trim().trim_start_matches("rustc ").split('.');
    (parts.next()? == "1").then_some(())?;
    parts.next()?.parse().ok()
}

/// A toolchain name ProjectIle will pass to rustup or write to `rust-toolchain.toml`.
pub fn valid_toolchain(name: &str) -> bool {
    let name = name.trim();
    if CHANNELS.iter().any(|(channel, _)| *channel == name) {
        return true;
    }
    let parts: Vec<&str> = name.split('.').collect();
    (2..=3).contains(&parts.len())
        && parts.first() == Some(&"1")
        && parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()))
}

const BOOK_PATH: &str = "share/doc/rust/html/book/index.html";

/// The offline copy of "The Rust Programming Language", if one is installed: in the active
/// compiler's sysroot (`rustup component add rust-docs`), in any rustup toolchain, or where
/// distribution packages put it.
pub fn offline_book(rustc: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(rustc) = rustc
        && let Ok(output) = Command::new(rustc).args(["--print", "sysroot"]).output()
        && output.status.success()
    {
        candidates.push(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()).join(BOOK_PATH));
    }
    let rustup_home = std::env::var_os("RUSTUP_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")));
    if let Some(toolchains) = rustup_home.map(|home| home.join("toolchains"))
        && let Ok(entries) = std::fs::read_dir(toolchains)
    {
        candidates.extend(entries.filter_map(Result::ok).map(|entry| entry.path().join(BOOK_PATH)));
    }
    candidates.extend(["/usr", "/usr/local", "/opt/homebrew"].iter().map(|prefix| Path::new(prefix).join(BOOK_PATH)));
    if let Some(dir) = docs_dir() {
        candidates.extend(downloaded_books(&dir));
    }
    first_existing(candidates)
}

/// ProjectIle's own data folder for downloaded docs: `$XDG_DATA_HOME/projectile/rust-docs`
/// (or `~/.local/share/…`), `~/Library/Application Support/ProjectIle/rust-docs` on macOS.
pub fn docs_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/ProjectIle/rust-docs"));
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(".local/share")))
        .map(|data| data.join("projectile").join("rust-docs"))
}

/// Books unpacked by [`Action::DownloadDocs`]: `<docs_dir>/<version>/rust-docs-*/rust-docs/<BOOK_PATH>`.
fn downloaded_books(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for version in std::fs::read_dir(root).into_iter().flatten().filter_map(Result::ok) {
        for archive in std::fs::read_dir(version.path()).into_iter().flatten().filter_map(Result::ok) {
            out.push(archive.path().join("rust-docs").join(BOOK_PATH));
        }
    }
    out.sort();
    out.reverse();
    out
}

fn first_existing(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|path| path.is_file())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Install(String),
    Uninstall(String),
    SetDefault(String),
    UpdateAll,
    /// `rust-docs` for the default toolchain: the offline Rust book and standard library docs.
    AddDocs,
    /// Official rustup-init, without touching shell profiles.
    InstallRustup,
    /// Without rustup: download the official rust-docs archive for `version`/`host` from
    /// static.rust-lang.org and unpack it into ProjectIle's own data folder (no root needed).
    DownloadDocs {
        version: String,
        host: String,
    },
}

impl Action {
    pub fn title(&self) -> String {
        match self {
            Self::Install(t) => format!("Install Rust {t}"),
            Self::Uninstall(t) => format!("Uninstall Rust {t}"),
            Self::SetDefault(t) => format!("Make Rust {t} the default"),
            Self::UpdateAll => "Update installed toolchains".into(),
            Self::AddDocs => "Install the offline Rust book".into(),
            Self::InstallRustup => "Install rustup".into(),
            Self::DownloadDocs { version, .. } => format!("Download the Rust {version} documentation"),
        }
    }

    pub fn explanation(&self) -> &'static str {
        match self {
            Self::Install(_) => "Downloads the compiler, Cargo, rustfmt and clippy into ~/.rustup. Nothing else changes.",
            Self::Uninstall(_) => "Removes this toolchain from ~/.rustup. Projects pinned to it will ask to reinstall it.",
            Self::SetDefault(_) => "`cargo` and `rustc` will use this version everywhere, unless a project pins another one.",
            Self::UpdateAll => "Updates stable, beta and nightly to their newest builds. Numbered versions never change.",
            Self::AddDocs => {
                "Adds the rust-docs component to your default toolchain: the Rust book and the standard library documentation, readable without internet."
            }
            Self::DownloadDocs { .. } => {
                "Downloads the official documentation archive (about 25 MB) from static.rust-lang.org and unpacks it into ProjectIle's data folder in your home. It contains the Rust book and the standard library docs, readable without internet. Nothing outside that folder changes, and no administrator rights are needed."
            }
            Self::InstallRustup => {
                "Downloads the official installer from sh.rustup.rs and installs rustup and stable Rust into ~/.rustup and ~/.cargo for your user only. It does not use sudo or edit your shell profile; add ~/.cargo/bin to PATH to use it in terminals. Your system Rust package is left alone."
            }
        }
    }

    /// The program and arguments, shown to the user before anything runs.
    pub fn command(&self, rustup: Option<&Path>) -> Result<(PathBuf, Vec<String>), String> {
        let rustup = || rustup.map(Path::to_path_buf).ok_or_else(|| "rustup is not installed.".to_owned());
        let check = |t: &str| if valid_toolchain(t) { Ok(t.trim().to_owned()) } else { Err(format!("`{t}` is not a toolchain name ProjectIle accepts.")) };
        Ok(match self {
            Self::Install(t) => (rustup()?, vec!["toolchain".into(), "install".into(), check(t)?, "--profile".into(), "default".into()]),
            Self::Uninstall(t) => (rustup()?, vec!["toolchain".into(), "uninstall".into(), check(t)?]),
            Self::SetDefault(t) => (rustup()?, vec!["default".into(), check(t)?]),
            Self::UpdateAll => (rustup()?, vec!["update".into()]),
            Self::AddDocs => (rustup()?, vec!["component".into(), "add".into(), "rust-docs".into()]),
            Self::DownloadDocs { version, host } => {
                if cfg!(windows) {
                    return Err("On Windows, install rustup and run `rustup component add rust-docs`.".into());
                }
                let valid_version = version.split('.').count() == 3 && version.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
                let valid_host = !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
                if !valid_version || !valid_host {
                    return Err(format!("`{version}` / `{host}` is not a Rust version and platform ProjectIle can download docs for."));
                }
                let dir = docs_dir().ok_or("no home folder to put the docs in")?.join(version);
                let dir = dir.to_string_lossy();
                if dir.contains('\'') {
                    return Err("The docs folder path contains a quote; ProjectIle won't build a shell command with it.".into());
                }
                let sh = crate::toolchain::find_program("sh").ok_or("sh was not found")?;
                let url = format!("https://static.rust-lang.org/dist/rust-docs-{version}-{host}.tar.xz");
                (sh, vec!["-c".into(), format!("mkdir -p '{dir}' && curl --proto '=https' --tlsv1.2 -sSfL '{url}' | tar -xJ -C '{dir}'")])
            }
            Self::InstallRustup => {
                if cfg!(windows) {
                    return Err("On Windows, download rustup-init.exe from https://rustup.rs/.".into());
                }
                let sh = crate::toolchain::find_program("sh").ok_or("sh was not found")?;
                (sh, vec!["-c".into(), "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path".into()])
            }
        })
    }

    /// The action as a [`crate::command::Task`] for the shared runner.
    pub fn task(&self, rustup: Option<&Path>) -> Result<crate::command::Task, String> {
        let (program, args) = self.command(rustup)?;
        Ok(crate::command::Task {
            title: self.title(),
            explanation: self.explanation().to_owned(),
            program,
            args,
            cwd: None,
            log: crate::command::Log::Rust,
            then: crate::command::Then::Nothing,
        })
    }

    pub fn command_text(&self, rustup: Option<&Path>) -> String {
        match self.command(rustup) {
            Ok((_, args)) if args.first().is_some_and(|a| a == "-c") => args.get(1).cloned().unwrap_or_default(),
            Ok((program, args)) => {
                let name = program.file_name().map_or_else(|| program.display().to_string(), |n| n.to_string_lossy().into_owned());
                format!("{name} {}", args.join(" "))
            }
            Err(error) => error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_matches_known_release_dates() {
        assert_eq!(civil_from_days(release_day(1)), (2015, 6, 25));
        assert_eq!(civil_from_days(release_day(85)), (2025, 2, 20));
        assert_eq!(civil_from_days(release_day(95)), (2026, 4, 16));
        let list = releases(release_day(99) + 3);
        assert_eq!(list.first().map(Release::version).as_deref(), Some("1.99.0"));
        assert_eq!(list.last().map(|r| r.minor), Some(OLDEST_LISTED_MINOR));
        assert_eq!(releases(release_day(99) - 1).first().map(|r| r.minor), Some(98));
        assert_eq!(releases(0).len(), 1, "a clock before 2015 still yields a sane list");
    }

    #[test]
    fn parses_rustup_toolchain_list() {
        let list = parse_toolchain_list(
            "stable-x86_64-unknown-linux-gnu (active, default)\n1.95.0-x86_64-unknown-linux-gnu\nnightly-2026-10-01-aarch64-apple-darwin\n",
        );
        assert_eq!(list.len(), 3);
        assert!(list[0].is_default && list[0].is_active);
        assert_eq!(list[0].short(), "stable");
        assert_eq!(list[1].short(), "1.95.0");
        assert_eq!(list[2].short(), "nightly-2026-10-01");
        assert!(parse_toolchain_list("no installed toolchains\n").is_empty());
    }

    #[test]
    fn only_plain_toolchain_names_reach_rustup() {
        for ok in ["stable", "beta", "nightly", "1.95", "1.95.0"] {
            assert!(valid_toolchain(ok), "{ok}");
        }
        for bad in ["", "1", "2.0.0", "1.95.0; rm -rf ~", "--force", "1..0", "stable2", "1.9999.0"] {
            assert!(!valid_toolchain(bad), "{bad}");
        }
        assert!(Action::Install("--force".into()).command(Some(Path::new("/x/rustup"))).is_err());
        assert!(Action::SetDefault("1.95.0".into()).command(None).is_err(), "no rustup, no command");
        assert_eq!(Action::Install("1.95.0".into()).command_text(Some(Path::new("/x/rustup"))), "rustup toolchain install 1.95.0 --profile default");
        let docs = Action::DownloadDocs { version: "1.99.0".into(), host: "x86_64-unknown-linux-gnu".into() };
        if !cfg!(windows) && crate::toolchain::find_program("sh").is_some() {
            assert!(docs.command_text(None).contains("https://static.rust-lang.org/dist/rust-docs-1.99.0-x86_64-unknown-linux-gnu.tar.xz"));
        }
        assert!(Action::DownloadDocs { version: "1.99.0; rm -rf ~".into(), host: "x".into() }.command(None).is_err());
        assert!(Action::DownloadDocs { version: "1.99.0".into(), host: "x86_64'$(id)".into() }.command(None).is_err());
    }

    #[test]
    fn offline_book_only_when_the_file_exists() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("projectile-book-{}", std::process::id()));
        let book = dir.join(BOOK_PATH);
        assert_eq!(first_existing(vec![book.clone()]), None);
        std::fs::create_dir_all(book.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
        std::fs::write(&book, "<html>").map_err(|e| e.to_string())?;
        let found = first_existing(vec![dir.join("missing.html"), book.clone()]);
        let _ = std::fs::remove_dir_all(&dir);
        if found != Some(book) {
            return Err("the existing book file was not found".into());
        }
        Ok(())
    }

    #[test]
    fn reads_minor_versions() {
        assert_eq!(minor_of("1.95"), Some(95));
        assert_eq!(minor_of("1.99.0"), Some(99));
        assert_eq!(minor_of("stable"), None);
        assert_eq!(minor_of("2.1"), None);
    }
}
