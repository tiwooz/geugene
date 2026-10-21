//! Where the PhotoCraft source lives. ProjectIle is a separate app, so it never assumes it was
//! built inside a PhotoCraft checkout: the source folder is chosen at run time (CMake's "Where is
//! the source code"), validated, and remembered. Detection only proposes a starting value.

use std::fs;
use std::path::{Path, PathBuf};

/// Environment variable that overrides detection (useful for scripts and MCP clients).
pub const SOURCE_ENV: &str = "PROJECTILE_SOURCE";
/// Canonical upstream used by the optional source checkout workflow.
pub const UPSTREAM_REPOSITORY: &str = "https://github.com/tiwooz/photocore.git";
pub const DEFAULT_BRANCH: &str = "main";
const MAX_FONT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

/// `Ok` when `root` is a PhotoCraft workspace ProjectIle can generate from.
pub fn validate(root: &Path) -> Result<PathBuf, String> {
    if root.as_os_str().is_empty() {
        return Err("Choose the PhotoCraft source folder.".into());
    }
    let root = root.canonicalize().map_err(|error| format!("PhotoCraft source {} is not reachable: {error}", root.display()))?;
    let manifest = read_small(&root.join("Cargo.toml"), MAX_MANIFEST_BYTES)
        .map_err(|_| format!("{} has no Cargo.toml; choose the root of a PhotoCraft checkout.", root.display()))?;
    if !manifest.contains("[workspace]") {
        return Err(format!("{} is not a Cargo workspace root; choose the root of a PhotoCraft checkout.", root.display()));
    }
    for required in ["apps/photocraft/Cargo.toml", "crates/engine/src/lib.rs", "crates/ui-egui/Cargo.toml"] {
        if !root.join(required).is_file() {
            return Err(format!("{} does not look like PhotoCraft source (missing {required}).", root.display()));
        }
    }
    Ok(root)
}

/// The first valid candidate: the saved choice, `$PROJECTILE_SOURCE`, then folders around the
/// executable and the working directory (a checkout that contains ProjectIle, or a sibling
/// `photocraft`/`photocore` folder), then the folder ProjectIle was compiled next to.
pub fn detect(saved: &str) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if !saved.trim().is_empty() {
        candidates.push(PathBuf::from(saved.trim()));
    }
    if let Some(env) = std::env::var_os(SOURCE_ENV) {
        candidates.push(PathBuf::from(env));
    }
    let starts = [std::env::current_exe().ok(), std::env::current_dir().ok()];
    for start in starts.into_iter().flatten() {
        for dir in start.ancestors().take(8) {
            candidates.push(dir.to_path_buf());
            for sibling in ["photocraft", "photocore"] {
                candidates.push(dir.join(sibling));
            }
        }
    }
    // A build-time hint only: when ProjectIle was compiled from `<checkout>/projectile`.
    if let Some(parent) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
        candidates.push(parent.to_path_buf());
    }
    candidates.into_iter().find_map(|candidate| validate(&candidate).ok())
}

/// A ref accepted by `git clone --branch` or `git checkout`. It is passed as one argument, never
/// through a shell, but validation still keeps the source UI and task log unambiguous.
pub fn validate_ref(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(DEFAULT_BRANCH.into());
    }
    let valid = value.len() <= 160
        && !value.starts_with('-')
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'));
    if valid { Ok(value.into()) } else { Err("Use a branch, tag, or commit name made of letters, numbers, -, _, . and /.".into()) }
}

/// Tags already present in a checkout, newest names first. Fetching happens only in the explicit
/// Clone / Update action.
pub fn local_versions(root: &Path) -> Vec<String> {
    let Some(git) = crate::toolchain::find_program("git") else { return Vec::new() };
    let output = std::process::Command::new(git)
        .current_dir(root)
        .args(["tag", "--list", "--sort=-version:refname"])
        .output()
        .ok();
    let Some(output) = output.filter(|output| output.status.success()) else { return Vec::new() };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .filter_map(|tag| validate_ref(tag).ok())
        .take(48)
        .collect()
}

/// `version` from the source's `[workspace.package]` (PhotoCraft's release version).
pub fn photocraft_version(root: &Path) -> Option<String> {
    workspace_package_value(root, "version")
}

/// `rust-version` from the source's `[workspace.package]`: the oldest Rust that builds PhotoCraft.
pub fn rust_version(root: &Path) -> Option<String> {
    workspace_package_value(root, "rust-version")
}

fn workspace_package_value(root: &Path, key: &str) -> Option<String> {
    let manifest = read_small(&root.join("Cargo.toml"), MAX_MANIFEST_BYTES).ok()?;
    let section = manifest.split("[workspace.package]").nth(1)?;
    let section = section.split("\n[").next()?;
    let line = section.lines().find(|line| line.split('=').next().is_some_and(|name| name.trim() == key))?;
    let value = line.split('=').nth(1)?.trim().trim_matches('"');
    (!value.is_empty() && value.len() < 32).then(|| value.to_owned())
}

/// Fonts for ProjectIle's interface. Inter and JetBrains Mono come from the PhotoCraft source's
/// `assets/fonts`; Satoshi (by Indian Type Foundry, from Fontshare) from the user's own copy. None is
/// bundled with ProjectIle or copied into projects; missing fonts fall back to egui's defaults.
pub struct Fonts {
    pub regular: Option<Vec<u8>>,
    pub medium: Option<Vec<u8>>,
    pub semibold: Option<Vec<u8>>,
    pub mono: Option<Vec<u8>>,
    pub satoshi_medium: Option<Vec<u8>>,
    pub satoshi_bold: Option<Vec<u8>>,
    /// Folder Satoshi was loaded from, for About.
    pub satoshi_from: Option<PathBuf>,
}

pub fn fonts(root: Option<&Path>) -> Fonts {
    let load = |name: &str| root.and_then(|root| read_bytes(&root.join("assets/fonts").join(name)));
    let satoshi = find_satoshi(root);
    let satoshi_file =
        |weight: &str| satoshi.as_ref().and_then(|dir| ["ttf", "otf"].iter().find_map(|ext| read_bytes(&dir.join(format!("Satoshi-{weight}.{ext}")))));
    Fonts {
        regular: load("Inter-Regular.ttf"),
        medium: load("Inter-Medium.ttf"),
        semibold: load("Inter-SemiBold.ttf"),
        mono: load("JetBrainsMono-Regular.ttf"),
        satoshi_medium: satoshi_file("Medium"),
        satoshi_bold: satoshi_file("Bold"),
        satoshi_from: satoshi.clone(),
    }
}

/// The folder holding `Satoshi-Medium` and `Satoshi-Bold` (OTF or TTF), searched in the source's
/// `font/` folder, `$CRAFT_FONTS_DIR`, a sibling `craft-fonts` checkout, then the user's and the
/// system's font folders. The search is bounded so a huge font folder can't stall startup.
pub fn find_satoshi(root: Option<&Path>) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut places: Vec<PathBuf> = Vec::new();
    if let Some(root) = root {
        places.push(root.join("font"));
        places.push(root.join("assets/fonts"));
        if let Some(parent) = root.parent() {
            places.push(parent.join("craft-fonts"));
        }
    }
    if let Some(dir) = std::env::var_os("CRAFT_FONTS_DIR") {
        places.push(PathBuf::from(dir));
    }
    if let Some(home) = &home {
        places.push(home.join(".local/share/fonts"));
        places.push(home.join(".fonts"));
        places.push(home.join("Library/Fonts"));
    }
    places.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
    for extension in ["ttf", "otf"] {
        let mut budget = 20_000usize;
        if let Some(found) = places.iter().filter(|p| p.is_dir()).find_map(|place| search_satoshi(place, 0, &mut budget, extension)) {
            return Some(found);
        }
    }
    None
}

fn search_satoshi(dir: &Path, depth: usize, budget: &mut usize, extension: &str) -> Option<PathBuf> {
    if depth > 6 || *budget == 0 {
        return None;
    }
    if ["Medium", "Bold"].iter().all(|weight| dir.join(format!("Satoshi-{weight}.{extension}")).is_file()) {
        return Some(dir.to_path_buf());
    }
    let mut subdirs = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.filter_map(Result::ok) {
        *budget = budget.saturating_sub(1);
        if *budget == 0 {
            return None;
        }
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            subdirs.push(entry.path());
        }
    }
    subdirs.sort();
    subdirs.iter().find_map(|sub| search_satoshi(sub, depth + 1, budget, extension))
}

fn read_bytes(path: &Path) -> Option<Vec<u8>> {
    let len = fs::metadata(path).ok()?.len();
    if len == 0 || len > MAX_FONT_BYTES {
        return None;
    }
    fs::read(path).ok()
}

fn read_small(path: &Path, limit: u64) -> Result<String, String> {
    let len = fs::metadata(path).map_err(|error| error.to_string())?.len();
    if len > limit {
        return Err(format!("{} is unexpectedly large", path.display()));
    }
    fs::read_to_string(path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_folders_that_are_not_photocraft() {
        assert!(validate(Path::new("")).is_err());
        assert!(validate(Path::new("/definitely/not/here")).is_err());
        assert!(validate(&std::env::temp_dir()).is_err());
    }

    #[test]
    fn detects_the_checkout_next_to_projectile() -> Result<(), String> {
        let root = detect("").ok_or("no PhotoCraft source detected next to projectile/")?;
        validate(&root)?;
        if photocraft_version(&root).is_none() || rust_version(&root).is_none() {
            return Err("could not read PhotoCraft's workspace version".into());
        }
        Ok(())
    }

    #[test]
    fn projectile_is_not_a_member_of_the_photocraft_workspace() -> Result<(), String> {
        let root = detect("").ok_or("no PhotoCraft source detected")?;
        let photocraft = fs::read_to_string(root.join("Cargo.toml")).map_err(|error| error.to_string())?;
        let own = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).map_err(|error| error.to_string())?;
        if photocraft.contains("projectile") || !own.contains("[workspace]") {
            return Err("ProjectIle must stay a standalone workspace that PhotoCraft does not build".into());
        }
        Ok(())
    }

    #[test]
    fn prefers_satoshi_ttf_when_otf_and_ttf_are_both_available() -> Result<(), String> {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos();
        let root = std::env::temp_dir().join(format!("projectile-font-test-{unique}"));
        let otf = root.join("font/Satoshi/Fonts/OTF");
        let ttf = root.join("font/Satoshi/Fonts/TTF");
        fs::create_dir_all(&otf).map_err(|error| error.to_string())?;
        fs::create_dir_all(&ttf).map_err(|error| error.to_string())?;
        for folder in [&otf, &ttf] {
            fs::write(folder.join("Satoshi-Medium.otf"), b"otf").map_err(|error| error.to_string())?;
            fs::write(folder.join("Satoshi-Bold.otf"), b"otf").map_err(|error| error.to_string())?;
        }
        for weight in ["Medium", "Bold"] {
            fs::write(ttf.join(format!("Satoshi-{weight}.ttf")), b"ttf").map_err(|error| error.to_string())?;
        }
        let result = find_satoshi(Some(&root)).ok_or("Satoshi was not found")?;
        let fonts = fonts(Some(&root));
        let _ = fs::remove_dir_all(&root);
        if result != ttf || fonts.satoshi_medium.as_deref() != Some(b"ttf".as_slice()) || fonts.satoshi_bold.as_deref() != Some(b"ttf".as_slice()) {
            return Err(format!("did not choose the usable TrueType Satoshi files: {}", result.display()));
        }
        Ok(())
    }
}
