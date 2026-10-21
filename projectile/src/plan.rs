//! What a generation *would* do, computed before anything is written.
//!
//! A [`Plan`] is built from a [`ProjectSpec`] and a scanned [`Inventory`] of the PhotoCraft source.
//! The Create page shows it as the component table, the `+`/`-` summary and the source-review
//! diffs; the generator then executes the very same [`FileEdit`]s, so the preview cannot drift
//! from what lands on disk.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Directory names that are never copied, wherever they appear (VCS data and build output).
const NEVER_COPIED_ANYWHERE: &[&str] = &[".git", "target"];
/// Top-level folders never copied: ProjectIle itself (a separate app), an earlier backup, local
/// fonts (`font/`: licences such as Satoshi's forbid redistributing them in a project) and
/// local-only folders (gitignored corpora, plans, logs, reference shots).
const NEVER_COPIED_TOP_LEVEL: &[&str] =
    &["projectile", "backup", "upstream", "font", "corpus", "plan", "log", "refs", "temp", "dist", "secrets", "craft-fonts"];
/// Recursion bound for scanning and copying; PhotoCraft's tree is far shallower.
pub const MAX_DEPTH: usize = 48;
/// Folder (inside the generated project) that holds the untouched source backup.
/// The untouched PhotoCraft source, as a folder inside the project: `backup/Cargo.toml`, …
pub const BACKUP_DIR: &str = "backup";
pub const BACKUP_SOURCE_DIR: &str = BACKUP_DIR;
/// A note at the top of the backup saying what it is (the only file ProjectIle adds there).
pub const BACKUP_NOTE: &str = "backup/ORIGINAL-SOURCE.md";
/// Folder for a recoverable copy of the ProjectIle source that made this project.
pub const PROJECTILE_SNAPSHOT_DIR: &str = "projectile-source-snapshot";
/// Archive name is dated when the source snapshot is taken, so its provenance remains visible.
pub const PROJECTILE_SNAPSHOT_FILE: &str = "projectile-source-snapshot/projectile_src_20261020_053551.tar.gz";
pub const TEMPLATE_REVISION: &str = "photocraft-base-3";
/// Where the Viewer profile's startup image goes inside the project.
pub const VIEWER_IMAGE_DIR: &str = "assets/viewer";
/// PhotoCraft's standard top-level menus, in display order.
pub const PROJECT_MENUS: &[&str] = &["File", "Edit", "Image", "Layer", "Type", "Select", "Filter", "View", "Window", "Help"];
const PROJECTILE_DEBUG_UI: &str = include_str!("projectile_debug_template.rs");
const PROJECTILE_SOURCE_SNAPSHOT: &[u8] = include_bytes!("../snapshots/projectile_src_20261020_053551.tar.gz");

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    #[default]
    Base,
    /// Shows one chosen image at startup; no tools, no editing, only About.
    Viewer,
    /// A small runnable PhotoCraft UI host plus an index of crate-level learning demos.
    Learning,
    Full,
    Minimal,
}

impl Profile {
    pub const ALL: [Profile; 5] = [Profile::Base, Profile::Viewer, Profile::Learning, Profile::Full, Profile::Minimal];

    pub fn label(self) -> &'static str {
        match self {
            Self::Base => "Base",
            Self::Viewer => "Viewer",
            Self::Learning => "Learning demos",
            Self::Full => "Full",
            Self::Minimal => "Minimal",
        }
    }

    pub fn tagline(self) -> &'static str {
        match self {
            Self::Base => "Original PhotoCraft UI with Brush, Eraser, Paint Bucket and file basics",
            Self::Viewer => "Opens your chosen image at startup: no tools, no editing, only About",
            Self::Learning => "A small PhotoCraft UI host plus guided examples for the crates it uses",
            Self::Full => "Complete, version-pinned PhotoCraft source (preview only)",
            Self::Minimal => "Pruned build: paint, layers, colour, files (preview only)",
        }
    }

    /// Base, Viewer and Learning generate today; Full and Minimal show their planned result as a preview.
    pub fn available(self) -> bool {
        matches!(self, Self::Base | Self::Viewer | Self::Learning)
    }

    /// The PhotoCraft Cargo feature that turns this profile on in the generated app.
    pub fn feature(self) -> Option<&'static str> {
        match self {
            Self::Base => Some("base-profile"),
            Self::Viewer => Some("viewer-profile"),
            Self::Learning => Some("base-profile"),
            Self::Full | Self::Minimal => None,
        }
    }
}

/// Generation choices. Every field here changes the generated project, never ProjectIle itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    pub scale_percent: u32,
    pub include_docs: bool,
    pub backup_source: bool,
    /// Include the ProjectIle source snapshot that generated this project.
    pub backup_projectile: bool,
    pub vscode_config: bool,
    pub mcp: bool,
    pub mcp_port: u16,
    pub git_init: bool,
    /// Rust toolchain written to the project's `rust-toolchain.toml` (empty: don't pin).
    pub toolchain: String,
    /// What to do when the project folder already exists.
    pub on_existing: OnExisting,
    /// Viewer profile: the image shown at startup (copied into the project).
    pub viewer_image: String,
    /// Also write an Android (Gradle, Java) build.
    pub android: bool,
    /// Android API levels; 0 lets ProjectIle pick (see `android::resolve`).
    pub android_min_sdk: u32,
    pub android_target_sdk: u32,
    /// Java runtime/source compatibility for the generated Android project.
    pub android_java_version: u32,
    /// Gradle distribution used by the generated Android project.
    pub android_gradle_version: String,
    /// Whether the generated app starts with its main menu bar visible.
    pub show_menu_bar: bool,
    /// Hidden top-level menus in the generated app.
    pub hidden_menus: Vec<String>,
    /// Hidden submenu paths, joined with ` > ` (for example `File > Export`).
    pub hidden_submenus: Vec<String>,
    /// Add egui's visual diagnostics menu and inspector window to the generated app.
    pub visual_debug_menu: bool,
}

/// Image types the viewer can open at startup.
pub const VIEWER_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "psd", "psb", "pcraft"];
const MAX_VIEWER_IMAGE: u64 = 1 << 30;

/// `Ok(extension)` when `path` is an image the viewer can show.
pub fn check_viewer_image(path: &str) -> Result<String, String> {
    let path = Path::new(path.trim());
    if path.as_os_str().is_empty() {
        return Err("The Viewer profile needs an image: choose one under Profile.".into());
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if !VIEWER_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("{} is not an image type the viewer opens ({}).", path.display(), VIEWER_EXTENSIONS.join(", ")));
    }
    let meta = fs::metadata(path).map_err(|error| format!("Can't read the viewer image {}: {error}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{} is not a file.", path.display()));
    }
    if meta.len() > MAX_VIEWER_IMAGE {
        return Err(format!("{} is larger than 1 GB.", path.display()));
    }
    Ok(ext)
}

/// What generation does with a project folder that already exists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnExisting {
    /// Don't generate; choose another name.
    #[default]
    Stop,
    /// Rename the old folder to `<name>-old-<date>-<time>`, then generate. Nothing is deleted.
    RenameAside,
    /// Generate into the folder: same-named files are overwritten, other files stay.
    Merge,
    /// Delete the folder (after confirmation in the window), then generate.
    Delete,
}

impl OnExisting {
    pub const ALL: [OnExisting; 4] = [OnExisting::Stop, OnExisting::RenameAside, OnExisting::Merge, OnExisting::Delete];

    pub fn label(self) -> &'static str {
        match self {
            Self::Stop => "Stop, and let me choose another name",
            Self::RenameAside => "Keep the old folder: rename it aside",
            Self::Merge => "Replace files inside it, keep my other files",
            Self::Delete => "Delete it and make it anew",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::Stop => "Nothing happens until you pick another name or option.",
            Self::RenameAside => "The old folder becomes <name>-old-<date>-<time> next to the new one. Nothing is deleted.",
            Self::Merge => "Files ProjectIle writes overwrite files with the same names; everything else in the folder stays as it is.",
            Self::Delete => "Everything in the folder is permanently deleted first. ProjectIle asks you to confirm.",
        }
    }
}

/// Folders ProjectIle must never delete or merge into: the filesystem root and its direct
/// children, the home folder and anything that contains it.
pub fn guard_existing(dir: &Path) -> Result<(), String> {
    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let depth = canonical.components().filter(|c| matches!(c, std::path::Component::Normal(_))).count();
    if depth < 2 {
        return Err(format!("{} is too close to the root of the disk to replace.", canonical.display()));
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from) {
        let home = home.canonicalize().unwrap_or(home);
        if home.starts_with(&canonical) {
            return Err(format!("{} is or contains your home folder; ProjectIle will not replace it.", canonical.display()));
        }
    }
    Ok(())
}

/// Number of files under `dir` (stops counting at `limit`), for the delete confirmation.
pub fn count_files(dir: &Path, limit: u64) -> u64 {
    fn walk(dir: &Path, count: &mut u64, limit: u64, depth: usize) {
        if depth > MAX_DEPTH || *count >= limit {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.filter_map(Result::ok) {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => walk(&entry.path(), count, limit, depth + 1),
                Ok(_) => *count = count.saturating_add(1),
                Err(_) => {}
            }
            if *count >= limit {
                return;
            }
        }
    }
    let mut count = 0;
    walk(dir, &mut count, limit, 0);
    count
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 800,
            scale_percent: 100,
            include_docs: true,
            backup_source: true,
            backup_projectile: true,
            vscode_config: true,
            mcp: false,
            mcp_port: 7878,
            git_init: true,
            toolchain: String::new(),
            on_existing: OnExisting::Stop,
            viewer_image: String::new(),
            android: false,
            android_min_sdk: 0,
            android_target_sdk: 0,
            android_java_version: 17,
            android_gradle_version: "9.6.0".into(),
            show_menu_bar: true,
            hidden_menus: Vec::new(),
            hidden_submenus: Vec::new(),
            visual_debug_menu: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectSpec {
    /// Root of the PhotoCraft checkout to generate from (validated by `source::validate`).
    pub source: PathBuf,
    pub name: String,
    pub destination: PathBuf,
    pub profile: Profile,
    pub options: Options,
}

impl ProjectSpec {
    pub fn project_dir(&self) -> Option<PathBuf> {
        project_directory(&self.name, &self.destination.to_string_lossy())
    }

    pub fn includes_backup(&self) -> bool {
        self.profile == Profile::Viewer || self.options.backup_source
    }
}

pub fn project_directory(name: &str, destination: &str) -> Option<PathBuf> {
    let package_name = package_name(name).ok()?;
    if destination.trim().is_empty() {
        return None;
    }
    Some(Path::new(destination.trim()).join(package_name))
}

pub fn package_name(input: &str) -> Result<String, String> {
    let name = input.trim();
    if name.is_empty() {
        return Err("Project name cannot be empty.".into());
    }
    if name.len() > 64 {
        return Err("Keep the project name to 64 characters or fewer.".into());
    }
    if !name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
        return Err("Use an ASCII letter first, followed by letters, numbers, hyphens, or underscores.".into());
    }
    let package = name.to_ascii_lowercase().replace('_', "-");
    let reserved = ["con", "prn", "aux", "nul"].contains(&package.as_str())
        || ((package.starts_with("com") || package.starts_with("lpt"))
            && package.len() == 4
            && package.as_bytes().get(3).is_some_and(|d| (b'1'..=b'9').contains(d)));
    if reserved {
        return Err("That name is reserved by Windows; choose another project name.".into());
    }
    Ok(package)
}

pub fn validate_options(options: &Options) -> Result<(), String> {
    if !(640..=3840).contains(&options.width) || !(480..=2160).contains(&options.height) {
        return Err("Window size must be between 640×480 and 3840×2160.".into());
    }
    if !(75..=200).contains(&options.scale_percent) {
        return Err("UI scale must be between 75% and 200%.".into());
    }
    if !options.toolchain.is_empty() && !crate::rust::valid_toolchain(&options.toolchain) {
        return Err(format!("`{}` is not a Rust toolchain ProjectIle can pin (use stable, beta, nightly or 1.N.N).", options.toolchain));
    }
    if options.mcp && options.mcp_port < 1024 {
        return Err("Choose an MCP control port from 1024 to 65535.".into());
    }
    if options.android && !crate::android::JAVA_VERSIONS.contains(&options.android_java_version) {
        return Err(format!(
            "Choose a supported Android Java version: {}.",
            crate::android::JAVA_VERSIONS.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
        ));
    }
    if options.android && !crate::android::GRADLE_VERSIONS.contains(&options.android_gradle_version.as_str()) {
        return Err(format!("Choose a supported Android Gradle version: {}.", crate::android::GRADLE_VERSIONS.join(", ")));
    }
    if options.hidden_menus.iter().any(|menu| !PROJECT_MENUS.contains(&menu.as_str())) {
        return Err("The menu selection contains an unknown top-level menu.".into());
    }
    if options.hidden_submenus.iter().any(|path| {
        let mut parts = path.split(" > ");
        !parts.next().is_some_and(|top| PROJECT_MENUS.contains(&top)) || parts.any(|part| part.trim().is_empty()) || !path.contains(" > ")
    }) {
        return Err("A hidden submenu path is invalid.".into());
    }
    Ok(())
}

/// Submenu paths in the source's public menu catalogue, grouped by top-level menu.
pub fn submenu_paths(source: &Path) -> Result<Vec<Vec<String>>, String> {
    let path = source.join("crates/ui-egui/src/menu_catalog.rs");
    let contents = fs::read_to_string(&path).map_err(|error| format!("Can't read {}: {error}", path.display()))?;
    let mut paths = std::collections::BTreeSet::new();
    for line in contents.lines() {
        let Some((_, rest)) = line.split_once("&[") else { continue };
        let Some((parts, _)) = rest.split_once(']') else { continue };
        let segments: Vec<String> = parts.split(',').filter_map(|part| part.trim().strip_prefix('"')?.strip_suffix('"').map(str::to_owned)).collect();
        if segments.len() > 1 && PROJECT_MENUS.contains(&segments.first().map(String::as_str).unwrap_or_default()) {
            for end in 2..=segments.len() {
                if let Some(prefix) = segments.get(..end) {
                    paths.insert(prefix.to_vec());
                }
            }
        }
    }
    Ok(paths.into_iter().collect())
}

// ---------------------------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Crate,
    App,
    Folder,
    File,
}

#[derive(Debug, Clone)]
pub struct SourceEntry {
    /// Workspace-relative path with `/` separators, e.g. `crates/paint`.
    pub path: String,
    pub kind: EntryKind,
    pub files: u64,
    pub bytes: u64,
}

/// Read-only view of the PhotoCraft source a project is generated from.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub root: PathBuf,
    pub entries: Vec<SourceEntry>,
    pub symlinks: Vec<String>,
    pub revision: Option<String>,
}

impl Inventory {
    pub fn totals(&self) -> (u64, u64) {
        self.entries.iter().fold((0, 0), |(files, bytes), entry| (files.saturating_add(entry.files), bytes.saturating_add(entry.bytes)))
    }

    fn entry(&self, path: &str) -> Option<&SourceEntry> {
        self.entries.iter().find(|entry| entry.path == path)
    }
}

pub fn excluded_from_snapshot(relative: &Path) -> bool {
    let mut components = relative.components().filter_map(|part| match part {
        std::path::Component::Normal(name) => name.to_str(),
        _ => None,
    });
    let Some(first) = components.next() else { return false };
    if NEVER_COPIED_TOP_LEVEL.contains(&first) || NEVER_COPIED_ANYWHERE.contains(&first) {
        return true;
    }
    relative.components().any(|part| part.as_os_str().to_str().is_some_and(|name| NEVER_COPIED_ANYWHERE.contains(&name)))
}

pub fn scan(root: &Path) -> Result<Inventory, String> {
    let root = root.canonicalize().map_err(|error| format!("could not resolve PhotoCraft source {}: {error}", root.display()))?;
    if !root.join("Cargo.toml").is_file() {
        return Err(format!("{} does not look like a PhotoCraft workspace (no Cargo.toml).", root.display()));
    }
    let mut inventory = Inventory { root: root.clone(), ..Inventory::default() };
    let mut top = read_sorted(&root)?;
    top.retain(|(name, _)| !excluded_from_snapshot(Path::new(name)));
    for (name, path) in top {
        let meta = fs::symlink_metadata(&path).map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        if meta.file_type().is_symlink() {
            inventory.symlinks.push(name);
            continue;
        }
        if meta.is_file() {
            inventory.entries.push(SourceEntry { path: name, kind: EntryKind::File, files: 1, bytes: meta.len() });
            continue;
        }
        if name == "crates" || name == "apps" {
            for (child, child_path) in read_sorted(&path)? {
                let relative = format!("{name}/{child}");
                let (files, bytes) = measure(&root, &child_path, 1, &mut inventory.symlinks)?;
                let kind = if name == "crates" { EntryKind::Crate } else { EntryKind::App };
                inventory.entries.push(SourceEntry { path: relative, kind, files, bytes });
            }
        } else {
            let (files, bytes) = measure(&root, &path, 1, &mut inventory.symlinks)?;
            inventory.entries.push(SourceEntry { path: name, kind: EntryKind::Folder, files, bytes });
        }
    }
    inventory.revision = crate::toolchain::source_revision(&root);
    Ok(inventory)
}

fn read_sorted(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| format!("could not list {}: {error}", dir.display()))? {
        let entry = entry.map_err(|error| format!("could not read an entry of {}: {error}", dir.display()))?;
        out.push((entry.file_name().to_string_lossy().into_owned(), entry.path()));
    }
    out.sort();
    Ok(out)
}

fn measure(root: &Path, path: &Path, depth: usize, symlinks: &mut Vec<String>) -> Result<(u64, u64), String> {
    if depth > MAX_DEPTH {
        return Err(format!("{} is nested more than {MAX_DEPTH} folders deep.", path.display()));
    }
    let meta = fs::symlink_metadata(path).map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    if meta.file_type().is_symlink() {
        symlinks.push(path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned());
        return Ok((0, 0));
    }
    if meta.is_file() {
        return Ok((1, meta.len()));
    }
    let (mut files, mut bytes) = (0u64, 0u64);
    for (_, child) in read_sorted(path)? {
        let relative = child.strip_prefix(root).unwrap_or(&child);
        if excluded_from_snapshot(relative) {
            continue;
        }
        let (f, b) = measure(root, &child, depth.saturating_add(1), symlinks)?;
        files = files.saturating_add(f);
        bytes = bytes.saturating_add(b);
    }
    Ok((files, bytes))
}

// ---------------------------------------------------------------------------------------------
// Components and the +/- summary
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Copied and used by the generated app.
    Enacted,
    /// Copied and edited by ProjectIle (see the review box).
    Patched,
    /// Copied and compiled (other crates depend on it), but its features are switched off.
    Dormant,
    /// Not copied into the generated workspace.
    Omitted,
    /// Written by ProjectIle; not part of the PhotoCraft source.
    Added,
}

impl Status {
    pub fn sign(self) -> char {
        match self {
            Self::Enacted => '+',
            Self::Patched => '~',
            Self::Dormant => '·',
            Self::Omitted => '-',
            Self::Added => '+',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Enacted => "enacted",
            Self::Patched => "patched",
            Self::Dormant => "dormant",
            Self::Omitted => "won't be used",
            Self::Added => "added",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Component {
    pub path: String,
    pub group: &'static str,
    pub role: &'static str,
    pub status: Status,
    pub note: String,
    pub files: u64,
    pub bytes: u64,
}

/// One bullet of the profile summary, drawn like a `git diff` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub sign: char,
    pub text: String,
}

fn change(sign: char, text: impl Into<String>) -> Change {
    Change { sign, text: text.into() }
}

/// Layer and one-line role of a workspace path, for beginners reading the component table.
pub fn describe(path: &str) -> (&'static str, &'static str) {
    match path {
        "crates/geom" => ("L0 foundation", "geometry: points, rects, transforms"),
        "crates/cms" => ("L0 foundation", "ICC colour management"),
        "crates/color" => ("L0 foundation", "colour models and pixel formats"),
        "crates/raster" => ("L0 foundation", "copy-on-write pixel tiles"),
        "crates/psd" => ("L0 formats", "Photoshop PSD/PSB reader and writer"),
        "crates/codecs" => ("L0 formats", "PNG, JPEG, TIFF and other flat formats"),
        "crates/raw" => ("L0 formats", "camera raw decoder"),
        "crates/affinity" => ("L0 formats", "Affinity document reader"),
        "crates/heif" => ("L0 formats", "optional HEIF/HEIC decoder"),
        "crates/tablet" => ("L0 input", "pen pressure and tilt"),
        "crates/doc" => ("L1 document", "layers, masks, effects: pure data"),
        "crates/ops" => ("L2 editing", "history and undoable operations"),
        "crates/paint" => ("L2 editing", "brush engine"),
        "crates/algo" => ("L2 editing", "filters and the bucket fill algorithm"),
        "crates/text" => ("L2 editing", "type engine and fonts"),
        "crates/vector" => ("L2 editing", "paths, shapes, vector masks"),
        "crates/compose" => ("L3 render", "CPU compositor (the oracle)"),
        "crates/gpu" => ("L3 render", "wgpu compositor"),
        "crates/format" => ("L3 render", "native .pcraft format"),
        "crates/io" => ("L4 files", "document import/export"),
        "crates/plugins" => ("L4 files", "sandboxed WebAssembly plug-ins"),
        "crates/engine" => ("L5 engine", "session and command registry"),
        "crates/ui-egui" => ("L6 interface", "the editor UI"),
        "crates/automation" => ("L6 interface", "MCP server and app bridge"),
        "crates/testkit" => ("tests", "test helpers"),
        "apps/photocraft" => ("app", "the desktop editor you will run"),
        "apps/photocraft-cli" => ("app", "headless CLI and MCP server"),
        "apps/photocraft-web" => ("app", "browser build"),
        "apps/projectile" => ("app", "this generator"),
        "xtask" => ("tooling", "repository maintenance tasks"),
        "docs" => ("docs", "architecture and contributor guides"),
        "Cargo.toml" => ("workspace", "workspace members and shared dependencies"),
        "Cargo.lock" => ("workspace", "exact resolved dependency versions"),
        _ if path.starts_with("crates/") => ("crate", "workspace crate"),
        _ if path.starts_with("apps/") => ("app", "workspace app"),
        _ => ("workspace", "copied as-is"),
    }
}

/// Crates the Base app actually exercises at run time.
const BASE_RUNTIME: &[&str] =
    &["geom", "cms", "color", "raster", "psd", "codecs", "tablet", "doc", "ops", "paint", "algo", "compose", "gpu", "format", "io", "engine", "ui-egui"];
/// Crates a Minimal build is planned to drop once the engine's features are modular.
const MINIMAL_DROPS: &[&str] = &["text", "vector", "plugins", "raw", "affinity", "automation", "heif"];

fn classify(spec: &ProjectSpec, entry: &SourceEntry) -> (Status, String) {
    let path = entry.path.as_str();
    let crate_name = path.strip_prefix("crates/");
    match spec.profile {
        Profile::Full => match path {
            "apps/projectile" => (Status::Omitted, "the generator is not part of PhotoCraft".into()),
            _ => (Status::Enacted, "kept exactly as upstream".into()),
        },
        Profile::Minimal => match (crate_name, path) {
            (Some(name), _) if MINIMAL_DROPS.contains(&name) => (Status::Omitted, "planned: removed once engine features are optional".into()),
            (_, "apps/photocraft-cli" | "apps/photocraft-web" | "apps/projectile" | "xtask") => (Status::Omitted, "not part of the Minimal app".into()),
            (_, "apps/photocraft" | "Cargo.toml") => (Status::Patched, "planned: Minimal feature set".into()),
            _ => (Status::Enacted, "planned: kept".into()),
        },
        Profile::Base | Profile::Viewer | Profile::Learning => match (crate_name, path) {
            (Some("automation"), _) if spec.options.mcp => (Status::Enacted, "serves MCP for Claude Code via the app bridge".into()),
            (Some("automation"), _) => (Status::Dormant, "compiled; MCP is off (tick Claude Code MCP to use it)".into()),
            (Some("heif"), _) => (Status::Dormant, "kept; the optional `heif` feature is off".into()),
            (Some("testkit"), _) => (Status::Dormant, "used by tests only".into()),
            (Some(name), _) if BASE_RUNTIME.contains(&name) => (Status::Enacted, format!("used by the {} app", spec.profile.label())),
            (Some(_), _) => (Status::Dormant, format!("compiled because the engine needs it; its tools are off in {}", spec.profile.label())),
            (_, "apps/photocraft") => (Status::Patched, format!("{} profile on by default, your window size and app id", spec.profile.label())),
            (_, "apps/photocraft-cli") if spec.options.mcp => (Status::Enacted, "MCP bridge; its headless mode is NOT restricted to Base".into()),
            (_, "apps/photocraft-cli") => (Status::Omitted, "could bypass the Base command allowlist".into()),
            (_, "apps/photocraft-web") => (Status::Omitted, "browser build is not part of Base".into()),
            (_, "apps/projectile") => (Status::Omitted, "the generator is not part of the project".into()),
            (_, "xtask") => (Status::Omitted, "PhotoCraft maintenance tooling".into()),
            (_, "docs") if !spec.options.include_docs => (Status::Omitted, "you chose not to include the docs".into()),
            (_, "Cargo.toml") => (Status::Patched, "workspace members trimmed".into()),
            _ => (Status::Enacted, "copied as-is".into()),
        },
    }
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub project_dir: PathBuf,
    pub package: String,
    pub components: Vec<Component>,
    pub capabilities: Vec<Change>,
    pub edits: Vec<FileEdit>,
    pub warnings: Vec<String>,
    /// Problems that block generation (the Generate button stays disabled).
    pub blockers: Vec<String>,
}

impl Plan {
    pub fn copied(&self) -> (u64, u64) {
        self.components
            .iter()
            .filter(|c| !matches!(c.status, Status::Omitted))
            .fold((0, 0), |(files, bytes), c| (files.saturating_add(c.files), bytes.saturating_add(c.bytes)))
    }

    /// Files and bytes copied from the PhotoCraft source into the project itself (no backup).
    pub fn copied_from_source(&self) -> (u64, u64) {
        self.components
            .iter()
            .filter(|c| !matches!(c.status, Status::Omitted | Status::Added))
            .fold((0, 0), |(files, bytes), c| (files.saturating_add(c.files), bytes.saturating_add(c.bytes)))
    }

    /// Short bullet list for the Create page: what is enacted and what won't be used. Crates are
    /// folded into one line per status; apps, folders and generated files get a line each.
    pub fn summary(&self) -> Vec<Change> {
        let mut out = Vec::new();
        for status in [Status::Enacted, Status::Patched, Status::Dormant, Status::Omitted] {
            let names: Vec<&str> = self.components.iter().filter(|c| c.status == status).filter_map(|c| c.path.strip_prefix("crates/")).collect();
            if names.is_empty() {
                continue;
            }
            let verb = match status {
                Status::Enacted => "would be enacted",
                Status::Patched => "would be enacted with ProjectIle edits",
                Status::Dormant => "compiled but dormant (features off; hover them in Components)",
                Status::Omitted | Status::Added => "won't be used",
            };
            let noun = if names.len() == 1 { "crate" } else { "crates" };
            out.push(change(status.sign(), format!("{} {noun} {verb}: {}", names.len(), names.join(", "))));
        }
        for c in self.components.iter().filter(|c| !c.path.starts_with("crates/")) {
            if c.group == "workspace" && c.status == Status::Enacted {
                continue;
            }
            let what = match c.path.strip_prefix("apps/") {
                Some(name) => format!("the {name} app"),
                None => format!("`{}`", c.path),
            };
            let verb = match c.status {
                Status::Enacted => "would be enacted",
                Status::Patched => "would be enacted with ProjectIle edits",
                Status::Dormant => "compiled but dormant",
                Status::Omitted => "won't be used",
                Status::Added => "would be added",
            };
            out.push(change(c.status.sign(), format!("{what} {verb}: {}", c.note)));
        }
        out
    }
}

fn capabilities(profile: Profile) -> Vec<Change> {
    match profile {
        Profile::Viewer => vec![
            change('+', "Shows your chosen image at startup (no Home screen)"),
            change('+', "Pan and zoom with the mouse; Help › About (help.about)"),
            change('-', "Every tool: Brush, Eraser, Paint Bucket, selections"),
            change('-', "Editing, layers panel, saving and the other menus"),
            change('-', "Toolbar, options bar, panels and status bar"),
        ],
        Profile::Base => vec![
            change('+', "Brush and Eraser (paint.stroke)"),
            change('+', "Paint Bucket, solid colour (paint.bucket) and Edit › Fill (edit.fill)"),
            change('+', "New, Open, Save, Save As, Save a Copy, recent files (file.*)"),
            change('+', "Undo and Redo (edit.undo, edit.redo)"),
            change('+', "Foreground/background colours (tools.setColors, tools.swapColors)"),
            change('+', "Zoom, Fit on Screen, Actual Pixels, Hand (view.*)"),
            change('+', "Layers panel selection (layer.select) and Help › About"),
            change('-', "Selections, masks and channels"),
            change('-', "Filters, adjustments and layer styles"),
            change('-', "Type, shapes, paths and the Pen tool"),
            change('-', "Transform, crop and image size"),
            change('-', "Plug-ins, actions and scripts"),
            change('-', "CLI and browser builds"),
        ],
        Profile::Learning => vec![
            change('+', "Runs a small PhotoCraft UI window (cargo run -p photocraft-learning)"),
            change('+', "Includes a guided index for geometry, colour, raster, document, paint, compose, engine and UI crates"),
            change('+', "Keeps PhotoCraft's source and its existing crate examples for inspection and modification"),
            change('-', "This profile is a learning workspace; it does not remove PhotoCraft capabilities"),
        ],
        Profile::Full => vec![
            change('+', "Every PhotoCraft tool, menu and panel"),
            change('+', "CLI, browser build and MCP server"),
            change('~', "Needs a pinned upstream release; not generated yet"),
        ],
        Profile::Minimal => vec![
            change('+', "Painting, brushes, layers, colour, file actions, About"),
            change('-', "Type, vector, plug-ins, camera raw and Affinity import"),
            change('~', "Needs optional engine features first; not generated yet"),
        ],
    }
}

// ---------------------------------------------------------------------------------------------
// File edits (shown in the review box, applied by the generator)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    Once,
    Exactly(usize),
    AtLeastOne,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub old: String,
    pub new: String,
    pub expect: Expect,
}

fn hunk(old: &str, new: impl Into<String>, expect: Expect) -> Hunk {
    Hunk { old: old.to_owned(), new: new.into(), expect }
}

#[derive(Debug, Clone)]
pub enum EditKind {
    /// Replace text in a copied PhotoCraft file.
    Patch(Vec<Hunk>),
    /// Write a new file (fails if it already exists).
    Create(String),
    /// Write binary data bundled with ProjectIle (the generator source archive).
    CreateBinary(Vec<u8>),
    /// Append to a copied file, or create it.
    Append(String),
    /// Copy a file from outside the source (the viewer's startup image).
    Copy(PathBuf),
}

#[derive(Debug, Clone)]
pub struct FileEdit {
    pub path: String,
    pub why: String,
    pub kind: EditKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Header,
    Context,
    Removed,
    Added,
    Note,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: LineKind,
    pub text: String,
}

pub fn apply_hunks(path: &str, source: &str, hunks: &[Hunk]) -> Result<String, String> {
    let mut text = source.to_owned();
    for h in hunks {
        let count = text.matches(&h.old).count();
        let ok = match h.expect {
            Expect::Once => count == 1,
            Expect::Exactly(n) => count == n,
            Expect::AtLeastOne => count >= 1,
        };
        if !ok {
            return Err(format!("{path} no longer contains `{}` the expected number of times (found {count}); the PhotoCraft source changed.", h.old.trim()));
        }
        text = text.replace(&h.old, &h.new);
    }
    Ok(text)
}

/// Unified-diff style preview of one edit against the current source.
pub fn preview(edit: &FileEdit, source_root: &Path) -> Result<Vec<DiffLine>, String> {
    let line = |kind, text: &str| DiffLine { kind, text: text.to_owned() };
    let mut out = vec![line(LineKind::Header, &format!("{}  ({})", edit.path, edit.why))];
    match &edit.kind {
        EditKind::Create(contents) => {
            out.push(line(LineKind::Note, "new file"));
            out.extend(contents.lines().take(400).map(|l| line(LineKind::Added, l)));
        }
        EditKind::CreateBinary(bytes) => {
            out.push(line(LineKind::Note, &format!("new binary file ({} bytes)", bytes.len())));
        }
        EditKind::Copy(from) => {
            out.push(line(LineKind::Note, &format!("copied from {}", from.display())));
        }
        EditKind::Append(contents) => {
            out.push(line(LineKind::Note, "appended at the end of the file"));
            out.extend(contents.lines().map(|l| line(LineKind::Added, l)));
        }
        EditKind::Patch(hunks) => {
            let path = source_root.join(&edit.path);
            let mut text = fs::read_to_string(&path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
            for h in hunks {
                let after = apply_hunks(&edit.path, &text, std::slice::from_ref(h))?;
                for (start, _) in text.match_indices(&h.old).take(8) {
                    out.extend(hunk_lines(&text, start, h));
                }
                text = after;
            }
        }
    }
    Ok(out)
}

fn hunk_lines(text: &str, start: usize, h: &Hunk) -> Vec<DiffLine> {
    let end = start.saturating_add(h.old.len());
    let line_start = text.get(..start).and_then(|s| s.rfind('\n')).map_or(0, |i| i.saturating_add(1));
    let line_end = text.get(end..).and_then(|s| s.find('\n')).map_or(text.len(), |i| end.saturating_add(i));
    let (Some(before), Some(after)) = (text.get(line_start..start), text.get(end..line_end)) else { return Vec::new() };
    let old_block = format!("{before}{}{after}", h.old);
    let new_block = format!("{before}{}{after}", h.new);
    let old_lines: Vec<&str> = old_block.lines().collect();
    let new_lines: Vec<&str> = new_block.lines().collect();
    let common_head = old_lines.iter().zip(&new_lines).take_while(|(a, b)| a == b).count();
    let max_tail = old_lines.len().min(new_lines.len()).saturating_sub(common_head);
    let common_tail = old_lines.iter().rev().zip(new_lines.iter().rev()).take(max_tail).take_while(|(a, b)| a == b).count();
    let mut out = Vec::new();
    let line_no = text.get(..start).map_or(0, |s| s.matches('\n').count()).saturating_add(1);
    out.push(DiffLine { kind: LineKind::Note, text: format!("@@ line {line_no} @@") });
    let previous = text.get(..line_start).and_then(|s| s.strip_suffix('\n')).and_then(|s| s.lines().last());
    if let Some(previous) = previous {
        out.push(DiffLine { kind: LineKind::Context, text: previous.to_owned() });
    }
    for l in old_lines.iter().take(common_head) {
        out.push(DiffLine { kind: LineKind::Context, text: (*l).to_owned() });
    }
    for l in old_lines.iter().skip(common_head).take(old_lines.len().saturating_sub(common_head + common_tail)) {
        out.push(DiffLine { kind: LineKind::Removed, text: (*l).to_owned() });
    }
    for l in new_lines.iter().skip(common_head).take(new_lines.len().saturating_sub(common_head + common_tail)) {
        out.push(DiffLine { kind: LineKind::Added, text: (*l).to_owned() });
    }
    for l in old_lines.iter().skip(old_lines.len().saturating_sub(common_tail)) {
        out.push(DiffLine { kind: LineKind::Context, text: (*l).to_owned() });
    }
    out
}

fn base_edits(spec: &ProjectSpec, package: &str, project_dir: &Path, inventory: &Inventory) -> Vec<FileEdit> {
    let o = &spec.options;
    let feature = spec.profile.feature().unwrap_or("base-profile");
    let app_id = format!("ai.storyteller.photocore.{package}");
    let app_dir = format!("PhotoCore-{package}");
    let mut workspace = Vec::new();
    if !o.mcp {
        workspace.push(hunk(", \"apps/photocraft-cli\"", "", Expect::Exactly(1)));
    }
    if inventory.entry("apps/projectile").is_some() {
        // Older checkouts carried ProjectIle as a PhotoCraft workspace member.
        workspace.push(hunk(", \"apps/projectile\"", "", Expect::Exactly(1)));
    }
    workspace.push(hunk(", \"xtask\"", "", Expect::Exactly(2)));
    if spec.includes_backup() {
        workspace.push(hunk(
            "[workspace]",
            format!("[workspace]\n# ProjectIle: the untouched source backup is a reference, not a member.\nexclude = [\"{BACKUP_DIR}\"]"),
            Expect::Once,
        ));
    }
    let mut edits = vec![
        FileEdit { path: "Cargo.toml".into(), why: "trim workspace members to what this project ships".into(), kind: EditKind::Patch(workspace) },
        FileEdit {
            path: "apps/photocraft/Cargo.toml".into(),
            why: format!("turn the {} capability profile on by default", spec.profile.label()),
            kind: EditKind::Patch(vec![hunk("base-profile = []", format!("base-profile = []\ndefault = [\"{feature}\"]"), Expect::Once)]),
        },
        FileEdit {
            path: "apps/photocraft/src/main.rs".into(),
            why: "own app id, startup window size and UI scale".into(),
            kind: EditKind::Patch(vec![
                hunk("\"ai.storyteller.photocraft\"", format!("\"{app_id}\""), Expect::Once),
                hunk(".with_inner_size([1440.0, 900.0])", format!(".with_inner_size([{}.0, {}.0])", o.width, o.height), Expect::Once),
                hunk(
                    "            let mut app = PhotocraftApp::new(session, services);",
                    format!(
                        "            cc.egui_ctx.set_zoom_factor({}.0 / 100.0);\n            let mut app = PhotocraftApp::new(session, services);",
                        o.scale_percent
                    ),
                    Expect::Once,
                ),
            ]),
        },
        FileEdit {
            path: "apps/photocraft/src/app_dirs.rs".into(),
            why: "keep settings separate from an installed PhotoCraft".into(),
            kind: EditKind::Patch(vec![
                hunk("\"Photocraft\"", format!("\"{app_dir}\""), Expect::AtLeastOne),
                hunk("\"photocraft\"", format!("\"{}\"", app_dir.to_ascii_lowercase()), Expect::AtLeastOne),
            ]),
        },
        FileEdit {
            path: "apps/photocraft/src/services.rs".into(),
            why: "desktop identity matches the new app id".into(),
            kind: EditKind::Patch(vec![hunk("ai.storyteller.photocraft", app_id.clone(), Expect::AtLeastOne)]),
        },
        FileEdit {
            path: "PROJECTILE.md".into(),
            why: "how to build, run and learn this project".into(),
            kind: EditKind::Create(guide(spec, package, project_dir)),
        },
    ];
    if spec.profile == Profile::Viewer {
        let ext = check_viewer_image(&o.viewer_image).unwrap_or_else(|_| "png".into());
        let image = format!("{VIEWER_IMAGE_DIR}/startup.{ext}");
        edits.push(FileEdit {
            path: image.clone(),
            why: "the image the viewer shows at startup".into(),
            kind: EditKind::Copy(PathBuf::from(o.viewer_image.trim())),
        });
        if let Some(main) = edits.iter_mut().find(|e| e.path == "apps/photocraft/src/main.rs")
            && let EditKind::Patch(hunks) = &mut main.kind
        {
            hunks.push(hunk(
                "            app.open_paths(&files);",
                format!(
                    "            // PhotoCore Viewer: show its bundled startup image unless another file was supplied.\n            if files.is_empty() {{\n                let startup = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../{image}\"));\n                if let Err(error) = app.open_bytes(\"startup.{ext}\", startup) {{\n                    app.open_failed(\"Viewer startup image\", &error);\n                }}\n            }} else {{\n                app.open_paths(&files);\n            }}"
                ),
                Expect::Once,
            ));
        }
    }
    if spec.profile == Profile::Learning {
        edits.extend(learning_edits());
    }
    let menu_choices = if spec.profile == Profile::Viewer { &["Help"][..] } else { PROJECT_MENUS };
    let top_menus = menu_choices
        .iter()
        .filter(|menu| !o.hidden_menus.iter().any(|hidden| hidden == **menu))
        .copied()
        .chain(o.visual_debug_menu.then_some("Debug"))
        .filter(|_| o.show_menu_bar)
        .collect::<Vec<_>>();
    let top_menus_rs =
        format!("pub const TOP_MENUS: [&str; {}] = [{}];", top_menus.len(), top_menus.iter().map(|menu| format!("{menu:?}")).collect::<Vec<_>>().join(", "));
    if o.visual_debug_menu {
        edits.push(FileEdit {
            path: "crates/ui-egui/src/projectile_debug.rs".into(),
            why: "egui visual debugging window".into(),
            kind: EditKind::Create(PROJECTILE_DEBUG_UI.into()),
            });
            edits.push(FileEdit {
            path: "crates/ui-egui/src/lib.rs".into(),
            why: "wire the generated egui diagnostics window".into(),
            kind: EditKind::Patch(vec![
                hunk("pub mod menus;", "pub mod menus;\npub(crate) mod projectile_debug;", Expect::Once),
                hunk("        self.frame += 1;", "        self.frame += 1;\n        projectile_debug::show(ctx);", Expect::Once),
            ]),
        });
    }
    if let Some(lib) = edits.iter_mut().find(|e| e.path == "apps/photocraft/src/main.rs")
        && let EditKind::Patch(hunks) = &mut lib.kind
    {
        hunks.push(hunk(
            "            let mut app = PhotocraftApp::new(session, services);",
            format!("            let mut app = PhotocraftApp::new(session, services);\n            app.ui.panels.menu_bar = {};", o.show_menu_bar),
            Expect::Once,
        ));
    }
    let hidden_submenus = o.hidden_submenus.iter().map(|path| format!("{path:?}")).collect::<Vec<_>>().join(", ");
    let debug_item = if o.visual_debug_menu {
        "    items.push(MenuItem { id: \"projectile.visualDebug\".into(), label: \"Visual Debug…\".into(), path: vec![\"Debug\".into()], shortcut: None, enabled: true, checked: None, color: None });\n"
    } else {
        ""
    };
    let mut menu_hunks = vec![
        hunk(
            "pub const TOP_MENUS: [&str; 10] = [\"File\", \"Edit\", \"Image\", \"Layer\", \"Type\", \"Select\", \"Filter\", \"View\", \"Window\", \"Help\"];",
            top_menus_rs,
            Expect::Once,
        ),
        hunk(
            "    items\n}\n\n/// Background tint of an Edit › Menus colour name.",
            format!(
                "{debug_item}    const PROJECT_HIDDEN_SUBMENUS: &[&str] = &[{hidden_submenus}];\n    items.retain(|item| {{\n        let path = item.path.join(\" > \");\n        !PROJECT_HIDDEN_SUBMENUS.iter().any(|hidden| path == *hidden || path.starts_with(&format!(\"{{hidden}} > \")))\n    }});\n    items\n}}\n\n/// Background tint of an Edit › Menus colour name."
            ),
            Expect::Once,
        ),
    ];
    if o.visual_debug_menu {
        menu_hunks.push(hunk(
            "pub fn invoke(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str, params: Value) -> Result<Value, String> {",
            "pub fn invoke(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str, params: Value) -> Result<Value, String> {\n    if id == \"projectile.visualDebug\" {\n        crate::projectile_debug::open(ctx);\n        return Ok(Value::Null);\n    }",
            Expect::Once,
        ));
    }
    edits.push(FileEdit {
        path: "crates/ui-egui/src/menus.rs".into(),
        why: "apply the generated project's selected menu layout".into(),
        kind: EditKind::Patch(menu_hunks),
    });
    if o.android {
        let sdk = crate::android::Sdk::detect();
        let (min, target) = crate::android::resolve(o.android_min_sdk, o.android_target_sdk, &sdk);
        if android_mobile_templates_available(&inventory.root) {
            edits.push(FileEdit {
                path: "crates/ui-egui/src/lib.rs".into(),
                why: "keep the Android dock rail available while its panel starts closed".into(),
                kind: EditKind::Patch(vec![hunk(
                    "        if chrome && self.ui.panels.dock {",
                    "        if chrome && (self.ui.panels.dock || self.ui.panels.rail) {",
                    Expect::Once,
                )]),
            });
            edits.push(FileEdit {
                path: "crates/ui-egui/src/panels.rs".into(),
                why: "let the Android dock rail reveal panels on demand".into(),
                kind: EditKind::Patch(vec![
                    hunk(
                        "    if t.pro {\n        dock_panels(app, ui, &p, &t);\n    }",
                        "    if t.pro && p.dock {\n        dock_panels(app, ui, &p, &t);\n    }",
                        Expect::Once,
                    ),
                    hunk(
                        "    if !t.pro {\n        dock_panels(app, ui, &p, &t);\n    }",
                        "    if !t.pro && p.dock {\n        dock_panels(app, ui, &p, &t);\n    }",
                        Expect::Once,
                    ),
                ]),
            });
            edits.push(FileEdit {
                path: "crates/ui-egui/src/canvas.rs".into(),
                why: "stack the Android Home screen actions on narrow phone displays".into(),
                kind: EditKind::Patch(vec![
                hunk(
                    "    let card = Rect::from_center_size(area.center(), egui::vec2(460.0, 330.0 + recent_h));",
                    "    // This changes only Home-screen geometry. Document zoom, pixels and resolution stay in the view and document models.\n    let narrow = area.width() < 640.0;\n    let card_width = if narrow { (area.width() - 24.0).max(240.0) } else { 460.0 };\n    let card = Rect::from_center_size(area.center(), egui::vec2(card_width, 330.0 + recent_h));",
                    Expect::Once,
                ),
                hunk(
                    "            ui.horizontal(|ui| {\n                ui.add_space(((card.width() - 2.0 * 190.0 - 12.0) / 2.0).max(0.0));\n                ui.spacing_mut().item_spacing.x = 12.0;\n                if crate::widgets::primary_button(ui, &new_label, 190.0).clicked() {\n                    let fields = app.new_document_fields();\n                    app.ui.open_dialog(crate::state::DialogKind::NewDocument, fields);\n                }\n                if crate::widgets::secondary_button(ui, &open_label, 190.0).clicked() {\n                    let _ = app.open_dialog_file();\n                }\n            });",
                    "            if narrow {\n                ui.vertical_centered(|ui| {\n                    if crate::widgets::primary_button(ui, &new_label, 220.0).clicked() {\n                        let fields = app.new_document_fields();\n                        app.ui.open_dialog(crate::state::DialogKind::NewDocument, fields);\n                    }\n                    ui.add_space(10.0);\n                    if crate::widgets::secondary_button(ui, &open_label, 220.0).clicked() {\n                        let _ = app.open_dialog_file();\n                    }\n                });\n            } else {\n                ui.horizontal(|ui| {\n                    ui.add_space(((card.width() - 2.0 * 190.0 - 12.0) / 2.0).max(0.0));\n                    ui.spacing_mut().item_spacing.x = 12.0;\n                    if crate::widgets::primary_button(ui, &new_label, 190.0).clicked() {\n                        let fields = app.new_document_fields();\n                        app.ui.open_dialog(crate::state::DialogKind::NewDocument, fields);\n                    }\n                    if crate::widgets::secondary_button(ui, &open_label, 190.0).clicked() {\n                        let _ = app.open_dialog_file();\n                    }\n                });\n            }",
                    Expect::Once,
                ),
                hunk("    let width = 380.0;", "    let width = ui.available_width().min(380.0).max(220.0);", Expect::Once),
                ]),
            });
        }
        for (path, contents) in crate::android::files(package, min, target, o.android_java_version, &o.android_gradle_version) {
            let why = if path.starts_with("apps/") { "Rust entry point for Android (experimental)" } else { "Android Gradle (Java) project" };
            edits.push(FileEdit { path, why: why.into(), kind: EditKind::Create(contents) });
        }
    }
    if o.vscode_config {
        edits.push(FileEdit {
            path: ".vscode/extensions.json".into(),
            why: "recommend rust-analyzer".into(),
            kind: EditKind::Create(VSCODE_EXTENSIONS.into()),
        });
        edits.push(FileEdit {
            path: ".vscode/settings.json".into(),
            why: "keep the backup and build output out of search and analysis".into(),
            kind: EditKind::Create(vscode_settings()),
        });
        edits.push(FileEdit {
            path: ".vscode/tasks.json".into(),
            why: "Run/Check/Test from Terminal › Run Task".into(),
            kind: EditKind::Create(vscode_tasks(o)),
        });
    }
    if o.mcp {
        edits.push(FileEdit {
            path: ".mcp.json".into(),
            why: "lets Claude Code drive the running editor".into(),
            kind: EditKind::Create(mcp_config(project_dir, o.mcp_port)),
        });
        edits.push(FileEdit {
            path: ".projectile/README.md".into(),
            why: "explains the local control token".into(),
            kind: EditKind::Create(
                "# Local agent files\n\n`control.token` is created by the editor the first time you run it with\n`--control-token-file .projectile/control.token`. It authorises local MCP clients;\nit is gitignored. Delete it to rotate the token.\n".into(),
            ),
        });
        edits.push(FileEdit {
            path: ".gitignore".into(),
            why: "never commit the control token".into(),
            kind: EditKind::Append("\n# ProjectIle: local MCP control token\n.projectile/control.token\n".into()),
        });
    }
    if !o.toolchain.is_empty() {
        edits.push(FileEdit {
            path: "rust-toolchain.toml".into(),
            why: "pins the Rust version rustup uses in this project".into(),
            kind: EditKind::Create(format!(
                "# Written by ProjectIle. rustup installs and uses this toolchain inside the project.\n[toolchain]\nchannel = \"{}\"\ncomponents = [\"rustfmt\", \"clippy\"]\n",
                o.toolchain.trim()
            )),
        });
    }
    if spec.includes_backup() {
        edits.push(FileEdit {
            path: BACKUP_NOTE.into(),
            why: "marks the backup as read-only reference".into(),
            kind: EditKind::Create(backup_readme(inventory)),
        });
    }
    if o.backup_projectile {
        edits.push(FileEdit {
            path: PROJECTILE_SNAPSHOT_FILE.into(),
            why: "recoverable source snapshot of the ProjectIle version that generated this project".into(),
            kind: EditKind::CreateBinary(PROJECTILE_SOURCE_SNAPSHOT.to_vec()),
        });
        edits.push(FileEdit {
            path: format!("{PROJECTILE_SNAPSHOT_DIR}/README.md"),
            why: "explains the included ProjectIle source snapshot".into(),
            kind: EditKind::Create(
                "# ProjectIle source snapshot\n\nThis archive is the checked source of ProjectIle that generated this project. Extract it outside this project if you need to inspect, restore, or compare the generator:\n\n```sh\ntar -xzf projectile_src_20261020_053551.tar.gz\n```\n\nIt is a backup artifact only; it is not a Cargo workspace member.\n".into(),
            ),
        });
    }
    edits
}

/// Older and synthetic source trees can still receive the Android wrapper, even when they do
/// not contain PhotoCraft's current dock and Home-screen implementation to patch.
fn android_mobile_templates_available(source: &Path) -> bool {
    [
        ("crates/ui-egui/src/lib.rs", "if chrome && self.ui.panels.dock {"),
        ("crates/ui-egui/src/panels.rs", "pub fn right_dock"),
        ("crates/ui-egui/src/canvas.rs", "fn start_screen"),
    ]
    .iter()
    .all(|(path, needle)| fs::read_to_string(source.join(path)).is_ok_and(|text| text.contains(needle)))
}

/// A small, independently runnable host and an honest map to the source's executable examples
/// and crate READMEs. The source snapshot keeps these examples aligned with its selected version.
fn learning_edits() -> Vec<FileEdit> {
    vec![
        FileEdit {
            path: "apps/photocraft-learning/Cargo.toml".into(),
            why: "a small PhotoCraft UI window for learning".into(),
            kind: EditKind::Create(
                "[package]\nname = \"photocraft-learning\"\ndescription = \"Small PhotoCraft UI host generated by ProjectIle\"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\npublish = false\n\n[lints]\nworkspace = true\n\n[dependencies]\neframe = { workspace = true, features = [\"default_fonts\", \"wgpu\", \"accesskit\", \"wayland\", \"x11\"] }\nphotocraft-engine = { workspace = true }\nphotocraft-ui-egui = { workspace = true }\n".into(),
            ),
        },
        FileEdit {
            path: "apps/photocraft-learning/src/main.rs".into(),
            why: "runs the real PhotoCraft egui interface in a compact educational host".into(),
            kind: EditKind::Create(
                "#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]\n\nfn main() -> eframe::Result {\n    let options = eframe::NativeOptions {\n        viewport: eframe::egui::ViewportBuilder::default()\n            .with_title(\"PhotoCraft UI learning demo\")\n            .with_inner_size([960.0, 680.0])\n            .with_min_inner_size([640.0, 480.0]),\n        ..Default::default()\n    };\n    eframe::run_native(\n        \"PhotoCraft UI learning demo\",\n        options,\n        Box::new(|_cc| {\n            let mut app = photocraft_ui_egui::PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());\n            app.ui.panels.dock = false;\n            app.ui.panels.rail = true;\n            Ok(Box::new(app))\n        }),\n    )\n}\n".into(),
            ),
        },
        FileEdit {
            path: "LEARNING.md".into(),
            why: "maps the generated PhotoCraft crates to runnable learning material".into(),
            kind: EditKind::Create(
                "# PhotoCraft learning demos\n\nRun the smallest real UI host with:\n\n```sh\ncargo run -p photocraft-learning\n```\n\nIt uses the same `photocraft-engine` session and `photocraft-ui-egui` app as PhotoCraft. Its dock starts closed; the icon rail opens it when you need panels.\n\n## Crate tours\n\nThe generated source keeps the upstream examples and tests next to each crate. Start with these focused, runnable tours:\n\n- `crates/geom/`: integer document geometry, rectangles and transforms.\n- `crates/cms/README.md`: ICC profiles and colour transforms.\n- `crates/color/`: pixel formats, blend modes and channel math.\n- `crates/raster/`: sparse copy-on-write tile surfaces.\n- `crates/doc/`: documents, layers, masks and effects as pure data.\n- `crates/ops/`: history and undoable document operations.\n- `crates/paint/`: brush engine and built-in presets.\n- `crates/algo/`: selection, filters and imaging algorithms.\n- `crates/compose/`: CPU compositor used as a rendering oracle.\n- `crates/gpu/`: wgpu compositor.\n- `crates/io/` and `crates/psd/`: import/export and PSD fidelity.\n- `crates/engine/`: the command registry and session boundary.\n- `crates/ui-egui/`: native egui widgets, chrome, panels and canvas input.\n\nUseful commands:\n\n```sh\ncargo test -p photocraft-geom\ncargo test -p photocraft-cms\ncargo test -p photocraft-raster\ncargo test -p photocraft-doc\ncargo test -p photocraft-paint\ncargo test -p photocraft-engine\ncargo test -p photocraft-ui-egui\n```\n\nRead each crate's `README.md` when present, then use `rg --files crates/<name>/examples crates/<name>/tests` to find its hands-on examples and focused tests.\n".into(),
            ),
        },
    ]
}

const VSCODE_EXTENSIONS: &str = "{\n  \"recommendations\": [\"rust-lang.rust-analyzer\", \"tamasfe.even-better-toml\"]\n}\n";

fn vscode_settings() -> String {
    format!(
        "{{\n  \"rust-analyzer.files.exclude\": [\"{BACKUP_DIR}\"],\n  \"files.watcherExclude\": {{ \"**/target/**\": true, \"{BACKUP_DIR}/**\": true }},\n  \"search.exclude\": {{ \"**/target\": true, \"{BACKUP_DIR}\": true }}\n}}\n"
    )
}

fn vscode_tasks(o: &Options) -> String {
    let mut tasks = vec![
        r#"    { "label": "PhotoCore: run", "type": "shell", "command": "cargo run -p photocraft", "group": { "kind": "build", "isDefault": true }, "problemMatcher": ["$rustc"] }"#.to_owned(),
        r#"    { "label": "PhotoCore: check", "type": "shell", "command": "cargo check -p photocraft", "problemMatcher": ["$rustc"] }"#.to_owned(),
        r#"    { "label": "PhotoCore: test Base allowlist", "type": "shell", "command": "cargo test -p photocraft-engine photocore_base_profile", "group": "test", "problemMatcher": ["$rustc"] }"#.to_owned(),
    ];
    if o.mcp {
        tasks.push(format!(
            r#"    {{ "label": "PhotoCore: run with MCP control", "type": "shell", "command": "cargo run -p photocraft -- --control {} --control-token-file .projectile/control.token", "problemMatcher": ["$rustc"] }}"#,
            o.mcp_port
        ));
    }
    format!("{{\n  \"version\": \"2.0.0\",\n  \"tasks\": [\n{}\n  ]\n}}\n", tasks.join(",\n"))
}

fn mcp_config(project_dir: &Path, port: u16) -> String {
    let manifest = project_dir.join("Cargo.toml");
    let token = project_dir.join(".projectile").join("control.token");
    let config = serde_json::json!({
        "mcpServers": {
            "photocore": {
                "command": "cargo",
                "args": ["run", "-q", "--manifest-path", manifest.to_string_lossy(), "-p", "photocraft-cli", "--",
                         "mcp", "--bridge", format!("127.0.0.1:{port}"), "--control-token-file", token.to_string_lossy()]
            }
        }
    });
    serde_json::to_string_pretty(&config)
        .map(|mut s| {
            s.push('\n');
            s
        })
        .unwrap_or_default()
}

fn backup_readme(inventory: &Inventory) -> String {
    let revision = inventory.revision.as_deref().unwrap_or("unknown revision");
    format!(
        "# Original PhotoCraft source (backup)\n\nThis folder is an untouched copy of the PhotoCraft source the project was generated from\n({revision}): a complete source tree of its own, apart from this note. It is excluded from the\nproject's Cargo workspace and from rust-analyzer.\n\nUse it as a fallback and a reference: compare your project with the original\n(`diff -ru {BACKUP_DIR}/crates crates`), restore a file you broke, or point an agent here instead of\nsearching for the PhotoCraft repository. Don't edit it; regenerating the project refreshes it.\n"
    )
}

fn guide(spec: &ProjectSpec, package: &str, project_dir: &Path) -> String {
    let o = &spec.options;
    let profile_description = match spec.profile {
        Profile::Base => {
            "The original PhotoCraft interface is kept; the Base runtime permits the Brush, Paint Bucket, Eraser, basic document/file operations, and essential navigation only."
        }
        Profile::Viewer => "The Viewer opens the chosen startup image and disables editing tools. Its source backup remains available under `backup/`.",
        Profile::Learning => "The Learning profile adds a small PhotoCraft UI host and a crate-by-crate guide in `LEARNING.md`.",
        Profile::Full => "The complete PhotoCraft source profile (preview only).",
        Profile::Minimal => "The pruned PhotoCraft source profile (preview only).",
    };
    let mut text = format!(
        r#"# {package}: PhotoCore {profile}

A PhotoCraft-derived native image editor generated by ProjectIle from the {profile} profile.
{profile_description}

## Build and run

Install Rust from <https://rustup.rs/>, then run from the project root:

```sh
cargo run -p photocraft
```

The generated app starts at {width} × {height} logical points with a {scale}% UI scale.
The included source tree is a snapshot, so it builds independently of the original checkout.
Project identity and snapshot metadata are in `.projectile.json`.

## Code layout

- `apps/photocraft/`: the native desktop application and its startup configuration.
- `crates/ui-egui/`: PhotoCraft's original editor interface and canvas interactions.
- `crates/engine/`: document commands and the Base capability allowlist.
- `crates/paint/`, `crates/algo/`, and `crates/raster/`: brush rendering, fill algorithms, and
  pixel/tile storage.
- `crates/doc/` and `crates/compose/`: document/layer data and canvas composition.
- `Cargo.toml`: workspace members and shared Rust dependencies.
- `PROJECTILE.md`: this guide.

## Useful Cargo commands

- `cargo check -p photocraft`: compile-check the generated editor.
- `cargo build -p photocraft`: build the editor.
- `cargo test -p photocraft-engine`: run engine tests.
- `cargo fmt -- --check`: check formatting; use `cargo fmt` to apply it.
- `cargo clippy --all-targets -- -D warnings`: lint all targets.
- `cargo tree -p photocraft`: inspect the editor's dependencies.
- `cargo run -p photocraft`: build and launch PhotoCore {profile}.
- `rustup show`: show the active Rust toolchain and installed targets.

The generated app enables its {profile} capability profile by default. To build unrestricted
PhotoCraft instead, disable the default feature with `cargo run -p photocraft --no-default-features`.
"#,
        profile = spec.profile.label(),
        profile_description = profile_description,
        width = o.width,
        height = o.height,
        scale = o.scale_percent,
    );
    let menu_choices = if spec.profile == Profile::Viewer { &["Help"][..] } else { PROJECT_MENUS };
    let visible_menus = menu_choices
        .iter()
        .filter(|menu| !o.hidden_menus.iter().any(|hidden| hidden == **menu))
        .copied()
        .chain(o.visual_debug_menu.then_some("Debug"))
        .filter(|_| o.show_menu_bar)
        .collect::<Vec<_>>();
    text.push_str(&format!(
        "\n## Menu layout\n\nMain menu bar at startup: **{}**. Visible top-level menus: {}.\n",
        if o.show_menu_bar { "shown" } else { "hidden" },
        if visible_menus.is_empty() { "none".to_owned() } else { visible_menus.join(", ") }
    ));
    if !o.hidden_submenus.is_empty() {
        text.push_str(&format!("Hidden submenu paths: {}.\n", o.hidden_submenus.join(", ")));
    }
    text.push_str("These settings affect presentation only; they do not remove commands or restrict shortcuts.\n");
    if o.visual_debug_menu {
        text.push_str("The **Debug > Visual Debug…** window exposes egui's inspection, memory, settings, style, and widget-hover diagnostics.\n");
    }
    if spec.includes_backup() {
        text.push_str(&format!(
            "\n## Original source backup\n\n`{BACKUP_SOURCE_DIR}/` holds the untouched PhotoCraft source this project came from, as a complete\nsource folder of its own. It is excluded from the workspace. Compare against it, restore files from it, or point an agent to it.\n"
        ));
    }
    if o.backup_projectile {
        text.push_str(&format!(
            "\n## ProjectIle source snapshot\n\n`{PROJECTILE_SNAPSHOT_DIR}/` contains the archived ProjectIle source that created this project. Extract it outside this workspace to inspect or recover the generator.\n"
        ));
    }
    if o.android {
        let (_, target) = crate::android::resolve(o.android_min_sdk, o.android_target_sdk, &crate::android::Sdk::detect());
        text.push_str(&format!(
            r#"
## Android development (experimental)

This project includes an Android Gradle/Java wrapper. PhotoCraft has not yet been verified on
Android. Install [Eclipse Temurin JDK {java}](https://adoptium.net/temurin/releases/?version={java})
and [Gradle {gradle}](https://gradle.org/releases/). Install the Android SDK
[command-line tools](https://developer.android.com/studio#command-line-tools-only), then use
`sdkmanager` to install the SDK platform/build tools for API {target}, platform tools, and an NDK
(r25 or newer). `sdkmanager --licenses` accepts the Android SDK licences.

Check the tools and install what is missing:

```sh
javac -version
gradle --version
sdkmanager --list
sdkmanager --licenses
sdkmanager --install "platform-tools" "platforms;android-{target}" "build-tools;{target}.0.0"
# Replace VERSION with a stable ndk;VERSION package ID shown by `sdkmanager --list`.
sdkmanager --install "ndk;VERSION"
```

Add Rust's Android targets and build/install the app:

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
sh android/build-rust.sh
cd android
gradle wrapper --gradle-version {gradle} --distribution-type bin
./gradlew assembleDebug
adb devices
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb logcat -s RustStdoutStderr:V
```

The selected JDK {java} is the Gradle runtime; Android Java source/bytecode stays at 17.
See the [Android SDK manager guide](https://developer.android.com/tools/sdkmanager),
[NDK downloads](https://developer.android.com/ndk/downloads), and
[adb guide](https://developer.android.com/tools/adb). The debug APK is written under
`android/app/build/outputs/apk/debug/`.
"#,
            java = o.android_java_version,
            gradle = o.android_gradle_version,
        ));
    }
    if o.mcp {
        text.push_str(&format!(
            "\n## Claude Code and MCP\n\n`.mcp.json` registers a `photocore` MCP server for Claude Code. It bridges to the running\neditor, so start the editor with its control port first:\n\n```sh\ncargo run -p photocraft -- --control {port} --control-token-file .projectile/control.token\n```\n\nThen run `claude` in this folder and approve the `photocore` server. Agents see and drive the\nlive editor through the same Base allowlist you have. `photocraft-cli` is included for this\nbridge; its *headless* `mcp` mode (without `--bridge`) is not restricted to Base.\n\nThe paths in `.mcp.json` are absolute ({dir}). Update them if you move the project.\n",
            port = o.mcp_port,
            dir = project_dir.display()
        ));
    }
    if !o.include_docs {
        text.push_str("\nThe PhotoCraft `docs/` folder was not included in this project.\n");
    }
    text
}

/// Build the plan. Never fails: problems become `blockers` so the UI can always show them.
pub fn build(spec: &ProjectSpec, inventory: &Inventory) -> Plan {
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let package = match package_name(&spec.name) {
        Ok(package) => package,
        Err(error) => {
            blockers.push(error);
            "project".to_owned()
        }
    };
    if spec.destination.as_os_str().is_empty() || spec.destination.to_string_lossy().trim().is_empty() {
        blockers.push("Choose a destination directory.".into());
    }
    if let Err(error) = validate_options(&spec.options) {
        blockers.push(error);
    }
    if !spec.profile.available() {
        blockers.push(format!("The {} profile is a preview; Base, Viewer, and Learning demos can be generated today.", spec.profile.label()));
    }
    let project_dir = spec.destination.join(&package);
    if project_dir.exists() {
        let canonical = project_dir.canonicalize().unwrap_or_else(|_| project_dir.clone());
        if !inventory.root.as_os_str().is_empty() && inventory.root.starts_with(&canonical) {
            blockers.push(format!("{} contains the PhotoCraft source itself; choose another name or location.", project_dir.display()));
        } else {
            let name = |p: &Path| p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            match spec.options.on_existing {
                OnExisting::Stop => {
                    blockers.push(format!("{} already exists; choose another name, or choose what to do with it under Creates.", project_dir.display()))
                }
                OnExisting::RenameAside => {
                    let aside = aside_path(&project_dir, crate::generator::unix_now());
                    warnings.push(format!("{} already exists and will be renamed to {} (nothing is deleted).", project_dir.display(), name(&aside)));
                }
                OnExisting::Merge | OnExisting::Delete => match guard_existing(&project_dir) {
                    Err(error) => blockers.push(error),
                    Ok(()) if spec.options.on_existing == OnExisting::Merge && !project_dir.is_dir() => {
                        blockers.push(format!("{} is a file, not a folder; files can't be merged into it.", project_dir.display()))
                    }
                    Ok(()) if spec.options.on_existing == OnExisting::Merge => warnings
                        .push(format!("Generating into the existing {}: files with the same names are overwritten, other files stay.", project_dir.display())),
                    Ok(()) => warnings.push(format!("{} and everything in it will be permanently deleted before generating.", project_dir.display())),
                },
            }
        }
    }
    if !inventory.root.as_os_str().is_empty() {
        let destination = spec.destination.canonicalize().unwrap_or_else(|_| spec.destination.clone());
        if destination.starts_with(&inventory.root) {
            blockers.push(format!("Choose a destination outside the PhotoCraft source directory {}.", inventory.root.display()));
        }
    }
    if spec.profile == Profile::Viewer
        && let Err(error) = check_viewer_image(&spec.options.viewer_image)
    {
        blockers.push(error);
    }
    if spec.options.android {
        let sdk = crate::android::Sdk::detect();
        let (min, target) = crate::android::resolve(spec.options.android_min_sdk, spec.options.android_target_sdk, &sdk);
        if let Err(error) = crate::android::validate(min, target) {
            blockers.push(error);
        }
        warnings.push(format!(
            "Android (minSdk {min}, targetSdk {target}) is experimental: PhotoCraft has not been verified on Android, and building it needs rustup's aarch64-linux-android target, cargo-ndk and an NDK. Its entry point is one exported symbol, the only unsafe attribute in the project."
        ));
        if sdk.root.is_some() && !sdk.has_platform(target) {
            warnings.push(format!("Android SDK platform {target} is not installed; install it with the SDK Manager before building."));
        }
    }
    if !spec.options.toolchain.is_empty() && inventory.entry("rust-toolchain.toml").is_some() {
        blockers.push("The source already has a rust-toolchain.toml; clear the pin on the Rust tab.".into());
    }
    if !inventory.symlinks.is_empty() {
        blockers.push(format!("The source contains symlinks, which snapshots do not copy: {}", inventory.symlinks.join(", ")));
    }

    let mut components: Vec<Component> = inventory
        .entries
        .iter()
        .map(|entry| {
            let (group, role) = describe(&entry.path);
            let (status, note) = classify(spec, entry);
            Component { path: entry.path.clone(), group, role, status, note, files: entry.files, bytes: entry.bytes }
        })
        .collect();

    let edits = if spec.profile.available() { base_edits(spec, &package, &project_dir, inventory) } else { Vec::new() };
    if spec.profile.available() {
        let added = |path: &str, role: &'static str, note: &str| Component {
            path: path.into(),
            group: "generated",
            role,
            status: Status::Added,
            note: note.into(),
            files: 1,
            bytes: 0,
        };
        components.push(added("PROJECTILE.md", "project guide", "build steps and source tour"));
        components.push(added(".projectile.json", "provenance", "profile, options and source revision"));
        if spec.options.vscode_config {
            components.push(added(".vscode", "editor setup", "rust-analyzer, tasks and excludes"));
        }
        if !spec.options.toolchain.is_empty() {
            components.push(added("rust-toolchain.toml", "Rust version pin", "set on the Rust tab"));
        }
        if spec.profile == Profile::Viewer {
            components.push(added(VIEWER_IMAGE_DIR, "startup image", "the picture the viewer opens"));
        }
        if spec.profile == Profile::Learning {
            components.push(added("apps/photocraft-learning", "learning app", "small runnable PhotoCraft UI host"));
            components.push(added("LEARNING.md", "learning guide", "crate-by-crate runnable demo index"));
        }
        if spec.options.android {
            components.push(added("android", "Android build", "Gradle (Java) project, experimental"));
            components.push(added("apps/photocraft-android", "Android entry point", "Rust cdylib for Android, experimental"));
        }
        if spec.options.mcp {
            components.push(added(".mcp.json", "Claude Code", "MCP bridge to the running editor"));
        }
        if spec.includes_backup() {
            let (files, bytes) = inventory.totals();
            let backup_files = files.saturating_sub(inventory.entry("apps/projectile").map_or(0, |e| e.files));
            let backup_bytes = bytes.saturating_sub(inventory.entry("apps/projectile").map_or(0, |e| e.bytes));
            components.push(Component {
                path: BACKUP_SOURCE_DIR.into(),
                group: "generated",
                role: "original source backup",
                status: Status::Added,
                note: "untouched PhotoCraft source, outside the workspace".into(),
                files: backup_files,
                bytes: backup_bytes,
            });
        }
        if spec.options.backup_projectile {
            components.push(Component {
                path: PROJECTILE_SNAPSHOT_DIR.into(),
                group: "generated",
                role: "generator source snapshot",
                status: Status::Added,
                note: "recoverable ProjectIle source archive and extraction note".into(),
                files: 2,
                bytes: PROJECTILE_SOURCE_SNAPSHOT.len() as u64,
            });
        }
        if spec.options.mcp {
            warnings.push("photocraft-cli is included for the MCP bridge; its headless mode is not limited to Base.".into());
        }
        for edit in &edits {
            if let EditKind::Patch(_) = edit.kind
                && let Err(error) = preview(edit, &inventory.root)
            {
                blockers.push(error);
            }
        }
    }
    Plan { project_dir, package, components, capabilities: capabilities(spec.profile), edits, warnings, blockers }
}

/// What a generated project's `.projectile.json` records about how it was made.
#[derive(Debug, Clone, Deserialize)]
pub struct SavedProject {
    pub profile: String,
    pub project_name: String,
    #[serde(default)]
    pub options: Options,
    #[serde(default)]
    pub source_revision: Option<String>,
    #[serde(default)]
    pub created_unix_seconds: u64,
}

impl SavedProject {
    pub fn profile(&self) -> Option<Profile> {
        Profile::ALL.into_iter().find(|p| p.label() == self.profile)
    }
}

/// Read `dir/.projectile.json`, if `dir` was made by ProjectIle.
pub fn read_project(dir: &Path) -> Result<SavedProject, String> {
    let path = dir.join(".projectile.json");
    let meta = fs::metadata(&path).map_err(|_| format!("{} was not made by ProjectIle (no .projectile.json).", dir.display()))?;
    if meta.len() > 4 * 1024 * 1024 {
        return Err(format!("{} is unexpectedly large.", path.display()));
    }
    let text = fs::read_to_string(&path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{} is not valid ProjectIle metadata: {error}", path.display()))
}

/// Where a regenerated Viewer keeps its startup image while the old folder is moved or deleted.
fn regen_image_dir() -> Option<PathBuf> {
    crate::rust::docs_dir().and_then(|docs| docs.parent().map(|p| p.join("regenerate")))
}

/// The spec that regenerates the project in `dir` from `source`: same name, place, profile and
/// options as recorded in its `.projectile.json`, with `on_existing` deciding what happens to
/// the current folder.
pub fn regenerate_spec(dir: &Path, source: PathBuf, on_existing: OnExisting) -> Result<ProjectSpec, String> {
    if on_existing == OnExisting::Stop {
        return Err("Choose what to do with the existing folder: keep it renamed aside, replace files inside it, or delete it.".into());
    }
    let saved = read_project(dir)?;
    let profile = saved.profile().ok_or_else(|| format!("{} records an unknown profile `{}`.", dir.display(), saved.profile))?;
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("the project folder has no name")?;
    if package_name(&name).ok().as_deref() != Some(name.as_str()) {
        return Err(format!("The folder name `{name}` is not a project name ProjectIle can regenerate under; rename the folder first."));
    }
    let destination = dir.parent().map(Path::to_path_buf).ok_or("the project folder has no parent")?;
    let mut options = saved.options;
    options.on_existing = on_existing;
    if profile == Profile::Viewer && check_viewer_image(&options.viewer_image).is_err() {
        // The original picture moved: reuse the copy inside the project, kept somewhere safe first.
        let ext = VIEWER_EXTENSIONS
            .iter()
            .find(|ext| dir.join(VIEWER_IMAGE_DIR).join(format!("startup.{ext}")).is_file())
            .ok_or("the viewer's startup image is missing")?;
        let keep = regen_image_dir().ok_or("no home folder to keep the viewer image in")?;
        fs::create_dir_all(&keep).map_err(|error| format!("could not create {}: {error}", keep.display()))?;
        let kept = keep.join(format!("startup.{ext}"));
        fs::copy(dir.join(VIEWER_IMAGE_DIR).join(format!("startup.{ext}")), &kept).map_err(|error| format!("could not keep the viewer image: {error}"))?;
        options.viewer_image = kept.to_string_lossy().into_owned();
    }
    Ok(ProjectSpec { source, name, destination, profile, options })
}

/// `<dir>-old-YYYYMMDD-HHMMSS` (UTC) next to `dir`, numbered if that name is taken too.
pub fn aside_path(dir: &Path, unix_seconds: u64) -> PathBuf {
    let days = (unix_seconds / 86_400) as i64;
    let (year, month, day) = crate::rust::civil_from_days(days);
    let secs = unix_seconds % 86_400;
    let stamp = format!("{year:04}{month:02}{day:02}-{:02}{:02}{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    let name = dir.file_name().map_or_else(|| "project".to_owned(), |n| n.to_string_lossy().into_owned());
    let parent = dir.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut candidate = parent.join(format!("{name}-old-{stamp}"));
    let mut counter = 2u32;
    while candidate.exists() && counter < 1000 {
        candidate = parent.join(format!("{name}-old-{stamp}-{counter}"));
        counter += 1;
    }
    candidate
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    let name = UNITS.get(unit).copied().unwrap_or("B");
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {name}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names_are_portable_and_normalized() {
        assert_eq!(package_name("My_Project"), Ok("my-project".into()));
        assert!(package_name("../escape").is_err());
        assert!(package_name("9bad").is_err());
        assert!(package_name("CON").is_err());
        assert!(package_name("com7").is_err());
        assert!(package_name("compass").is_ok());
        assert!(package_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn preview_rejects_invalid_name_and_empty_destination() {
        assert!(project_directory("../escape", "/tmp").is_none());
        assert!(project_directory("valid", " ").is_none());
        assert_eq!(project_directory("My-App", "/tmp").map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned())), Some(Some("my-app".into())));
    }

    #[test]
    fn local_only_folders_are_excluded_only_at_the_top_level() {
        assert!(excluded_from_snapshot(Path::new("target/debug")));
        assert!(excluded_from_snapshot(Path::new("crates/x/target")));
        assert!(excluded_from_snapshot(Path::new(".git")));
        assert!(excluded_from_snapshot(Path::new("refs")));
        assert!(excluded_from_snapshot(Path::new("log")));
        assert!(!excluded_from_snapshot(Path::new("crates/x/src/log")), "a nested `log` module must be copied");
        assert!(!excluded_from_snapshot(Path::new("crates/x/src/plan.rs")));
    }

    #[test]
    fn hunk_preview_shows_only_changed_lines() -> Result<(), String> {
        let text = "a\nbase-profile = []\nz\n";
        let h = hunk("base-profile = []", "base-profile = []\ndefault = [\"base-profile\"]", Expect::Once);
        let start = text.find("base-profile").ok_or("missing")?;
        let lines = hunk_lines(text, start, &h);
        let added: Vec<_> = lines.iter().filter(|l| l.kind == LineKind::Added).map(|l| l.text.as_str()).collect();
        let removed = lines.iter().filter(|l| l.kind == LineKind::Removed).count();
        if added != ["default = [\"base-profile\"]"] || removed != 0 {
            return Err(format!("unexpected diff: {lines:?}"));
        }
        Ok(())
    }

    #[test]
    fn apply_hunks_reports_source_drift() {
        let drift = apply_hunks("x", "nothing here", &[hunk("needle", "pin", Expect::Once)]);
        assert!(drift.is_err());
        assert_eq!(apply_hunks("x", "a, \"xtask\" b, \"xtask\"", &[hunk(", \"xtask\"", "", Expect::Exactly(2))]), Ok("a b".into()));
    }

    #[test]
    fn real_workspace_plan_has_no_blockers_beyond_destination() -> Result<(), String> {
        let inventory = scan(&crate::source::detect("").ok_or("no PhotoCraft source detected")?)?;
        let spec = ProjectSpec {
            source: inventory.root.clone(),
            name: "plan-check".into(),
            destination: std::env::temp_dir().join("projectile-plan-check-does-not-exist"),
            profile: Profile::Base,
            options: Options { mcp: true, ..Options::default() },
        };
        let plan = build(&spec, &inventory);
        if !plan.blockers.is_empty() {
            return Err(format!("the real PhotoCraft source no longer matches ProjectIle's edits: {:?}", plan.blockers));
        }
        for edit in &plan.edits {
            preview(edit, &inventory.root)?;
        }
        let summary = plan.summary();
        if !summary.iter().any(|c| c.sign == '-' && c.text.contains("photocraft-web")) || !summary.iter().any(|c| c.sign == '+' && c.text.contains("paint")) {
            return Err(format!("summary misses enacted/omitted bullets: {summary:?}"));
        }
        Ok(())
    }

    #[test]
    fn real_workspace_viewer_and_android_plans_apply() -> Result<(), String> {
        let inventory = scan(&crate::source::detect("").ok_or("no PhotoCraft source detected")?)?;
        let image = std::env::temp_dir().join(format!("projectile-viewer-{}.png", std::process::id()));
        fs::write(&image, b"png").map_err(|e| e.to_string())?;
        let spec = ProjectSpec {
            source: inventory.root.clone(),
            name: "viewer-check".into(),
            destination: std::env::temp_dir().join("projectile-plan-check-does-not-exist"),
            profile: Profile::Viewer,
            options: Options {
                viewer_image: image.to_string_lossy().into_owned(),
                android: true,
                visual_debug_menu: true,
                hidden_submenus: vec!["File > Export".into()],
                ..Options::default()
            },
        };
        let plan = build(&spec, &inventory);
        let _ = fs::remove_file(&image);
        if !plan.blockers.is_empty() {
            return Err(format!("viewer/android plan blocked on the real source: {:?}", plan.blockers));
        }
        let manifest = fs::read_to_string(inventory.root.join("apps/photocraft/Cargo.toml")).map_err(|e| e.to_string())?;
        if !manifest.contains("viewer-profile = []") {
            return Err("PhotoCraft's app manifest has no viewer-profile feature".into());
        }
        if !plan.edits.iter().any(|e| e.path == "android/app/build.gradle") || !plan.edits.iter().any(|e| matches!(e.kind, EditKind::Copy(_))) {
            return Err("the plan is missing the Android files or the viewer image".into());
        }
        Ok(())
    }

    #[test]
    fn base_capability_list_matches_the_engine_allowlist() -> Result<(), String> {
        let engine = fs::read_to_string(crate::source::detect("").ok_or("no PhotoCraft source detected")?.join("crates/engine/src/lib.rs"))
            .map_err(|error| error.to_string())?;
        let allowlist = engine.split("fn command_available").nth(1).and_then(|rest| rest.split("fn profile_params_error").next()).ok_or("allowlist moved")?;
        for c in capabilities(Profile::Base).iter().filter(|c| c.sign == '+') {
            for id in c.text.split(['(', ')', ',', ' ']).filter(|w| w.contains('.') && !w.ends_with(".*")) {
                if !allowlist.contains(&format!("\"{id}\"")) {
                    return Err(format!("Base summary advertises `{id}` but the engine allowlist does not permit it"));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn aside_names_carry_a_utc_timestamp() {
        let aside = aside_path(Path::new("/tmp/projectile-none/demo"), 1_760_268_645);
        assert_eq!(aside, PathBuf::from("/tmp/projectile-none/demo-old-20251012-113045"));
    }

    #[test]
    fn options_validation_rejects_bad_values() {
        assert!(validate_options(&Options::default()).is_ok());
        assert!(validate_options(&Options { width: 100, ..Options::default() }).is_err());
        assert!(validate_options(&Options { scale_percent: 500, ..Options::default() }).is_err());
        assert!(validate_options(&Options { mcp: true, mcp_port: 80, ..Options::default() }).is_err());
        assert!(validate_options(&Options { toolchain: "1.95.0".into(), ..Options::default() }).is_ok());
        assert!(validate_options(&Options { toolchain: "1.95; rm".into(), ..Options::default() }).is_err());
        assert!(validate_options(&Options { android: true, android_java_version: 99, ..Options::default() }).is_err());
        assert!(validate_options(&Options { android: true, android_gradle_version: "9.0".into(), ..Options::default() }).is_err());
        assert!(validate_options(&Options { hidden_menus: vec!["Preferences".into()], ..Options::default() }).is_err());
        assert!(validate_options(&Options { hidden_submenus: vec!["File".into()], ..Options::default() }).is_err());
    }

    #[test]
    fn reads_every_nested_menu_path_from_the_catalog() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!("projectile-menu-catalog-{}", std::process::id()));
        let catalog = root.join("crates/ui-egui/src/menu_catalog.rs");
        fs::create_dir_all(catalog.parent().ok_or("catalog has no parent")?).map_err(|error| error.to_string())?;
        fs::write(
            &catalog,
            "(&[\"File\", \"Export\"], \"Export…\", None, \"file.export\"),\n(&[\"File\", \"Export\", \"For Web\"], \"Web…\", None, \"file.web\"),\n",
        )
        .map_err(|error| error.to_string())?;
        let paths = submenu_paths(&root)?;
        let _ = fs::remove_dir_all(&root);
        if !paths.contains(&vec!["File".into(), "Export".into()]) || !paths.contains(&vec!["File".into(), "Export".into(), "For Web".into()]) {
            return Err(format!("submenu paths weren't expanded: {paths:?}"));
        }
        Ok(())
    }
}
