//! Executes a [`plan::Plan`]: copies the PhotoCraft source, applies the reviewed edits, writes the
//! generated files and reports progress. Runs on a worker thread so the window stays responsive.

use crate::plan::{self, EditKind, Inventory, OnExisting, ProjectSpec, Status};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub enum Event {
    Log(String),
    Progress { done: u64, total: u64 },
}

#[derive(Debug, Clone)]
pub struct Report {
    pub path: PathBuf,
    /// Where an existing folder was moved when Replace was ticked.
    pub replaced: Option<PathBuf>,
    pub files: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProjectMetadata<'a> {
    generator: &'static str,
    generator_version: &'static str,
    profile: &'static str,
    project_name: &'a str,
    template_revision: &'static str,
    source_app_version: Option<String>,
    source_revision: Option<&'a str>,
    created_unix_seconds: u64,
    options: &'a plan::Options,
    components: Vec<ComponentRecord<'a>>,
}

#[derive(Debug, Serialize)]
struct ComponentRecord<'a> {
    path: &'a str,
    status: &'static str,
}

/// Generate from `spec.source`, the PhotoCraft checkout the user chose.
pub fn generate(spec: &ProjectSpec, progress: &mut dyn FnMut(Event)) -> Result<Report, String> {
    let source = crate::source::validate(&spec.source)?;
    generate_from_source(spec, &source, progress)
}

pub fn generate_from_source(spec: &ProjectSpec, source_root: &Path, progress: &mut dyn FnMut(Event)) -> Result<Report, String> {
    let mut spec = spec.clone();
    if spec.profile == plan::Profile::Viewer {
        spec.options.backup_source = true;
    }
    progress(Event::Log(format!("Scanning PhotoCraft source at {}…", source_root.display())));
    let inventory = plan::scan(source_root)?;
    let plan = plan::build(&spec, &inventory);
    if let Some(blocker) = plan.blockers.first() {
        return Err(blocker.clone());
    }
    for warning in &plan.warnings {
        progress(Event::Log(format!("Note: {warning}")));
    }
    let destination = &spec.destination;
    fs::create_dir_all(destination).map_err(|error| format!("could not create destination {}: {error}", destination.display()))?;
    let output = destination.join(&plan.package);
    let mut replaced = None;
    let mut merge = false;
    if fs::symlink_metadata(&output).is_ok() {
        match spec.options.on_existing {
            OnExisting::Stop => return Err(format!("{} already exists; choose another name, or choose what to do with it under Creates.", output.display())),
            OnExisting::RenameAside => {
                let aside = plan::aside_path(&output, unix_now());
                fs::rename(&output, &aside).map_err(|error| format!("could not move the existing {} aside: {error}", output.display()))?;
                progress(Event::Log(format!("Moved the existing folder to {} (nothing was deleted).", aside.display())));
                replaced = Some(aside);
            }
            OnExisting::Merge => {
                plan::guard_existing(&output)?;
                if !output.is_dir() {
                    return Err(format!("{} is a file, not a folder; files can't be merged into it.", output.display()));
                }
                progress(Event::Log(format!("Generating into the existing {} (same-named files are overwritten).", output.display())));
                merge = true;
            }
            OnExisting::Delete => {
                plan::guard_existing(&output)?;
                discard(&output, "the existing folder", progress)?;
                progress(Event::Log(format!("Deleted the existing {}.", output.display())));
            }
        }
    }
    let restore = |replaced: &Option<PathBuf>, progress: &mut dyn FnMut(Event)| {
        if let Some(aside) = replaced {
            match fs::rename(aside, &output) {
                Ok(()) => progress(Event::Log(format!("Restored the previous folder to {}.", output.display()))),
                Err(error) => progress(Event::Log(format!("Warning: could not restore {} to {}: {error}", aside.display(), output.display()))),
            }
        }
    };
    if !merge && let Err(error) = fs::create_dir(&output) {
        restore(&replaced, progress);
        return Err(if error.kind() == std::io::ErrorKind::AlreadyExists {
            format!("{} already exists; choose another name, or choose what to do with it under Creates.", output.display())
        } else {
            format!("could not create project directory {}: {error}", output.display())
        });
    }

    let result = write_project(&spec, &inventory, &plan, &output, merge, progress);
    match result {
        Ok(files) => {
            let mut warnings = Vec::new();
            if spec.options.git_init {
                match git_init(&output) {
                    Ok(message) => progress(Event::Log(message)),
                    Err(error) => {
                        progress(Event::Log(format!("Warning: {error}")));
                        warnings.push(error);
                    }
                }
            }
            Ok(Report { path: output, replaced, files, warnings })
        }
        // Never delete a folder we merged into: it holds the user's own files.
        Err(error) if merge => Err(format!("{error}. Generation stopped part-way; {} now holds a mix of old and new files.", output.display())),
        Err(error) => match discard(&output, "the incomplete project", progress) {
            Ok(()) => {
                progress(Event::Log(format!("Removed the incomplete project at {}.", output.display())));
                restore(&replaced, progress);
                Err(error)
            }
            Err(cleanup_error) => Err(format!("{error}; additionally {cleanup_error}")),
        },
    }
}

/// Remove `path` without ever leaving a half-deleted folder at that path: it is first renamed to
/// a hidden sibling (one atomic step; nothing is deleted if that fails), then the renamed copy is
/// deleted. A copy that can't be fully deleted (a file in use, say) is reported, not left in place.
fn discard(path: &Path, what: &str, progress: &mut dyn FnMut(Event)) -> Result<(), String> {
    let name = path.file_name().map_or_else(|| "project".to_owned(), |n| n.to_string_lossy().into_owned());
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut trash = parent.join(format!(".{name}-deleting-{}", unix_now()));
    let mut n = 2u32;
    while trash.exists() && n < 1000 {
        trash = parent.join(format!(".{name}-deleting-{}-{n}", unix_now()));
        n += 1;
    }
    fs::rename(path, &trash).map_err(|error| format!("could not delete {what} at {} (nothing was removed): {error}", path.display()))?;
    let removed = if trash.is_dir() { fs::remove_dir_all(&trash) } else { fs::remove_file(&trash) };
    if let Err(error) = removed {
        progress(Event::Log(format!("Warning: part of {what} is left at {} ({error}); delete it when nothing uses it.", trash.display())));
    }
    Ok(())
}

/// Sanity check before calling a generation successful: the files every project needs exist.
fn verify(output: &Path, spec: &ProjectSpec) -> Result<(), String> {
    let mut required =
        vec!["Cargo.toml", "Cargo.lock", "apps/photocraft/Cargo.toml", "apps/photocraft/src/main.rs", "crates/engine/Cargo.toml", ".projectile.json"];
    if spec.includes_backup() {
        required.extend(["backup/Cargo.toml", "backup/apps/photocraft/Cargo.toml", plan::BACKUP_NOTE]);
    }
    if spec.options.backup_projectile {
        required.extend([plan::PROJECTILE_SNAPSHOT_FILE, "projectile-source-snapshot/README.md"]);
    }
    let missing: Vec<&str> = required.into_iter().filter(|path| !output.join(path).exists()).collect();
    if missing.is_empty() { Ok(()) } else { Err(format!("the generated project is incomplete (missing {})", missing.join(", "))) }
}

pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn write_project(
    spec: &ProjectSpec,
    inventory: &Inventory,
    plan: &plan::Plan,
    output: &Path,
    merge: bool,
    progress: &mut dyn FnMut(Event),
) -> Result<u64, String> {
    let canonical_destination =
        spec.destination.canonicalize().map_err(|error| format!("could not resolve destination {}: {error}", spec.destination.display()))?;
    if canonical_destination.starts_with(&inventory.root) {
        return Err(format!("Choose a destination outside the PhotoCraft source directory {}.", inventory.root.display()));
    }
    let omitted: Vec<&str> = plan.components.iter().filter(|c| c.status == Status::Omitted).map(|c| c.path.as_str()).collect();
    let (project_files, _) = plan.copied_from_source();
    let backup_files = plan.components.iter().find(|c| c.path == plan::BACKUP_SOURCE_DIR).map_or(0, |c| c.files);
    let mut copier = Copier { done: 0, total: project_files.saturating_add(backup_files).max(1), merge, progress };

    (copier.progress)(Event::Log(format!("Copying the PhotoCraft source ({project_files} files)…")));
    copier.copy_tree(&inventory.root, &inventory.root, output, &|relative| omitted.iter().any(|path| relative == Path::new(path)), 1)?;

    for edit in &plan.edits {
        let path = output.join(&edit.path);
        match &edit.kind {
            EditKind::Patch(hunks) => {
                let text = fs::read_to_string(&path).map_err(|error| format!("could not read copied {}: {error}", edit.path))?;
                let patched = plan::apply_hunks(&edit.path, &text, hunks)?;
                fs::write(&path, patched).map_err(|error| format!("could not write {}: {error}", edit.path))?;
            }
            EditKind::Create(contents) => write_file(&path, contents, merge)?,
            EditKind::CreateBinary(bytes) => write_bytes(&path, bytes, merge)?,
            EditKind::Copy(from) => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|error| format!("could not create {}: {error}", parent.display()))?;
                }
                fs::copy(from, &path).map_err(|error| format!("could not copy {} into the project: {error}", from.display()))?;
            }
            // A merged project may already carry these lines from an earlier generation.
            EditKind::Append(contents) if merge && fs::read_to_string(&path).is_ok_and(|text| text.contains(contents.trim())) => {}
            EditKind::Append(contents) => {
                let mut file = OpenOptions::new().create(true).append(true).open(&path).map_err(|error| format!("could not open {}: {error}", edit.path))?;
                file.write_all(contents.as_bytes()).map_err(|error| format!("could not append to {}: {error}", edit.path))?;
            }
        }
        (copier.progress)(Event::Log(format!("{} {} ({})", edit_verb(&edit.kind), edit.path, edit.why)));
    }

    let created_unix_seconds =
        SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| format!("system clock is before the Unix epoch: {error}"))?.as_secs();
    let metadata = ProjectMetadata {
        generator: "ProjectIle",
        generator_version: env!("CARGO_PKG_VERSION"),
        profile: spec.profile.label(),
        project_name: &plan.package,
        template_revision: plan::TEMPLATE_REVISION,
        source_app_version: crate::source::photocraft_version(&inventory.root),
        source_revision: inventory.revision.as_deref(),
        created_unix_seconds,
        options: &spec.options,
        components: plan.components.iter().map(|c| ComponentRecord { path: &c.path, status: c.status.label() }).collect(),
    };
    let metadata_json = serde_json::to_string_pretty(&metadata).map_err(|error| format!("could not encode project metadata: {error}"))?;
    write_file(&output.join(".projectile.json"), &metadata_json, merge)?;

    if spec.includes_backup() {
        let backup = output.join(plan::BACKUP_SOURCE_DIR);
        fs::create_dir_all(&backup).map_err(|error| format!("could not create {}: {error}", backup.display()))?;
        (copier.progress)(Event::Log(format!("Copying the untouched source backup to {}…", plan::BACKUP_SOURCE_DIR)));
        copier.copy_tree(&inventory.root, &inventory.root, &backup, &|relative| relative == Path::new("apps/projectile"), 1)?; // older checkouts
    }
    if spec.options.mcp {
        // The editor creates control.token itself on first `--control-token-file` run.
        fs::create_dir_all(output.join(".projectile")).map_err(|error| format!("could not create .projectile: {error}"))?;
    }
    verify(output, spec)?;
    if spec.includes_backup() {
        let files = plan.components.iter().find(|c| c.path == plan::BACKUP_SOURCE_DIR).map_or(0, |c| c.files);
        (copier.progress)(Event::Log(format!("Backup: the original source ({files} files) is in {}/.", plan::BACKUP_DIR)));
    }
    Ok(copier.done)
}

fn edit_verb(kind: &EditKind) -> &'static str {
    match kind {
        EditKind::Patch(_) => "Patched",
        EditKind::Create(_) => "Wrote",
        EditKind::CreateBinary(_) => "Wrote",
        EditKind::Copy(_) => "Copied",
        EditKind::Append(_) => "Appended to",
    }
}

/// Write a generated file. A fresh project never overwrites; a merge replaces same-named files.
fn write_file(path: &Path, contents: &str, overwrite: bool) -> Result<(), String> {
    write_bytes(path, contents.as_bytes(), overwrite)
}

/// Write generated bytes. Binary generator snapshots use the same no-overwrite and flush rules
/// as generated text files.
fn write_bytes(path: &Path, contents: &[u8], overwrite: bool) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let mut options = OpenOptions::new();
    options.write(true);
    if overwrite {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options.open(path).map_err(|error| format!("could not create {}: {error}", path.display()))?;
    file.write_all(contents).map_err(|error| format!("could not write {}: {error}", path.display()))?;
    file.sync_all().map_err(|error| format!("could not flush {}: {error}", path.display()))
}

struct Copier<'a> {
    done: u64,
    total: u64,
    /// Generating into an existing folder: reuse folders, overwrite same-named files.
    merge: bool,
    progress: &'a mut dyn FnMut(Event),
}

impl Copier<'_> {
    fn copy_tree(&mut self, source_root: &Path, source: &Path, destination: &Path, skip: &dyn Fn(&Path) -> bool, depth: usize) -> Result<(), String> {
        if depth > plan::MAX_DEPTH {
            return Err(format!("{} is nested more than {} folders deep.", source.display(), plan::MAX_DEPTH));
        }
        for entry in fs::read_dir(source).map_err(|error| format!("could not list source {}: {error}", source.display()))? {
            let entry = entry.map_err(|error| format!("could not read a source directory entry: {error}"))?;
            let path = entry.path();
            let relative = path.strip_prefix(source_root).map_err(|error| format!("could not resolve source path {}: {error}", path.display()))?;
            if plan::excluded_from_snapshot(relative) || skip(relative) {
                continue;
            }
            let file_type = entry.file_type().map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
            if file_type.is_symlink() {
                return Err(format!("PhotoCraft source snapshot does not copy symlinks: {}", path.display()));
            }
            let output = destination.join(entry.file_name());
            if file_type.is_dir() {
                let created = if self.merge && output.is_dir() { Ok(()) } else { fs::create_dir(&output) };
                created.map_err(|error| format!("could not create snapshot directory {}: {error}", output.display()))?;
                self.copy_tree(source_root, &path, &output, skip, depth.saturating_add(1))?;
            } else if file_type.is_file() {
                fs::copy(&path, &output).map_err(|error| format!("could not copy {}: {error}", path.display()))?;
                self.done = self.done.saturating_add(1);
                if self.done.is_multiple_of(64) {
                    (self.progress)(Event::Progress { done: self.done, total: self.total });
                }
            } else {
                return Err(format!("unsupported file type in PhotoCraft source: {}", path.display()));
            }
        }
        Ok(())
    }
}

/// `git init` plus a first commit, so `git diff` later shows exactly what *you* changed.
fn git_init(project: &Path) -> Result<String, String> {
    let git = crate::toolchain::find_program("git").ok_or_else(|| "git was not found, so the project was not put under version control.".to_owned())?;
    let run = |args: &[&str]| -> Result<(), String> {
        let output = std::process::Command::new(&git).current_dir(project).args(args).output().map_err(|error| format!("could not run git: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().lines().last().unwrap_or("unknown git error").to_owned())
        }
    };
    run(&["init", "-q"]).map_err(|error| format!("git init failed: {error}"))?;
    run(&["add", "-A"]).map_err(|error| format!("git add failed: {error}"))?;
    match run(&["commit", "-q", "-m", "Initial PhotoCore project generated by ProjectIle"]) {
        Ok(()) => Ok("Initialised a git repository with the generated files as the first commit.".into()),
        Err(error) => Err(format!(
            "Initialised a git repository, but the first commit failed ({error}). Set `git config --global user.name` and `user.email`, then commit."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::generate_from_source;
    use crate::plan::{OnExisting, Options, Profile, ProjectSpec};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory() -> Result<PathBuf, String> {
        let unique = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("projectile-test-{}-{unique}", std::process::id()));
        fs::create_dir(&path).map_err(|error| format!("could not create test directory: {error}"))?;
        Ok(path)
    }

    fn source_fixture(root: &Path) -> Result<(), String> {
        for dir in [
            "apps/photocraft/src",
            "crates/ui-egui/src",
            "projectile/src",
            "apps/photocraft-cli/src",
            "apps/photocraft-web/src",
            "crates/engine/src/log",
            "docs",
            "xtask/src",
            ".git",
            "target",
            "plan",
            "refs",
        ] {
            fs::create_dir_all(root.join(dir)).map_err(|error| error.to_string())?;
        }
        let files = [
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/*\", \"apps/*\", \"xtask\"]\ndefault-members = [\"crates/*\", \"apps/photocraft\", \"apps/photocraft-cli\", \"xtask\"]\n",
            ),
            ("Cargo.lock", "lockfile"),
            (".gitignore", "/target\n"),
            (
                "apps/photocraft/src/main.rs",
                "const APP_ID: &str = \"ai.storyteller.photocraft\";\n            .with_inner_size([1440.0, 900.0])\n            let mut app = PhotocraftApp::new(session, services);\n            app.open_paths(&files);\n",
            ),
            ("apps/photocraft/src/app_dirs.rs", "\"Photocraft\" \"photocraft\""),
            ("apps/photocraft/src/services.rs", "ai.storyteller.photocraft"),
            (
                "crates/ui-egui/src/menus.rs",
                "pub const TOP_MENUS: [&str; 10] = [\"File\", \"Edit\", \"Image\", \"Layer\", \"Type\", \"Select\", \"Filter\", \"View\", \"Window\", \"Help\"];\n    items\n}\n\n/// Background tint of an Edit › Menus colour name.\npub fn invoke(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str, params: Value) -> Result<Value, String> {\n    if !app.session.command_available(id) {\n    if let Some(id) = clicked {\n        let ctx = ui.ctx().clone();\n        let id = alt_click(id, ctx.input(|i| i.modifiers.alt));\n        if let Err(e) = invoke(app, &ctx, &id, json!({})) {\n",
            ),
            ("crates/ui-egui/src/lib.rs", "pub mod menus;\n        self.frame += 1;\n"),
            ("crates/ui-egui/src/menu_catalog.rs", "(&[\"File\", \"Export\"], \"Export…\", None, \"file.export\"),\n"),
            ("apps/photocraft/Cargo.toml", "base-profile = []\n"),
            ("projectile/src/main.rs", "must not be copied"),
            ("apps/photocraft-cli/src/main.rs", "cli"),
            ("apps/photocraft-web/src/main.rs", "must not be copied"),
            ("crates/engine/Cargo.toml", "engine source"),
            ("crates/engine/src/log/mod.rs", "nested log module"),
            ("docs/guide.md", "docs"),
            (".git/secret", "must not be copied"),
            ("target/cache", "must not be copied"),
            ("plan/private", "must not be copied"),
            ("refs/shot.png", "must not be copied"),
            ("xtask/src/main.rs", "xtask"),
        ];
        for (path, contents) in files {
            fs::write(root.join(path), contents).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn spec(name: &str, destination: &Path, options: Options) -> ProjectSpec {
        ProjectSpec { source: PathBuf::new(), name: name.into(), destination: destination.to_path_buf(), profile: Profile::Base, options }
    }

    fn read(path: PathBuf) -> Result<String, String> {
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))
    }

    #[test]
    fn base_profile_generates_photocraft_snapshot_with_restricted_launch_guide() -> Result<(), String> {
        let root = temporary_directory()?;
        let source = root.join("source");
        fs::create_dir(&source).map_err(|error| error.to_string())?;
        source_fixture(&source)?;
        let options = Options { scale_percent: 125, git_init: false, ..Options::default() };
        let report = generate_from_source(&spec("My-App", &root, options), &source, &mut |_| {})?;
        let generated = report.path;
        let check = || -> Result<(), String> {
            let manifest = read(generated.join("Cargo.toml"))?;
            let app_manifest = read(generated.join("apps/photocraft/Cargo.toml"))?;
            let entry = read(generated.join("apps/photocraft/src/main.rs"))?;
            let app_dirs = read(generated.join("apps/photocraft/src/app_dirs.rs"))?;
            let guide = read(generated.join("PROJECTILE.md"))?;
            let provenance = read(generated.join(".projectile.json"))?;
            let checks = [
                (!manifest.contains("projectile"), "ProjectIle is not mentioned in the generated workspace"),
                (!manifest.contains("\"apps/photocraft-cli\""), "cli stays out without MCP"),
                (!manifest.contains("\"xtask\""), "xtask stays out"),
                (manifest.contains("exclude = [\"backup\"]"), "backup is excluded from the workspace"),
                (app_manifest.contains("default = [\"base-profile\"]"), "base profile on by default"),
                (entry.contains(".with_inner_size([1200.0, 800.0])"), "startup size"),
                (entry.contains("set_zoom_factor(125.0 / 100.0)"), "startup scale"),
                (entry.contains("\"ai.storyteller.photocore.my-app\""), "app id"),
                (app_dirs.contains("\"PhotoCore-my-app\"") && !app_dirs.contains("\"Photocraft\""), "own settings dir"),
                (guide.contains("cargo run -p photocraft") && guide.contains("Paint Bucket, Eraser"), "guide"),
                (provenance.contains("\"template_revision\": \"photocraft-base-3\""), "provenance"),
                (generated.join("crates/engine/src/log/mod.rs").exists(), "nested log module copied"),
                (generated.join("docs/guide.md").exists(), "docs copied"),
                (generated.join(".vscode/settings.json").exists() && generated.join(".vscode/tasks.json").exists(), "VS Code config"),
                (!generated.join(".mcp.json").exists(), "no MCP config unless asked"),
                (generated.join("backup/apps/photocraft-cli/src/main.rs").exists(), "backup keeps the original CLI"),
                (generated.join("backup/Cargo.toml").is_file() && generated.join("backup/ORIGINAL-SOURCE.md").is_file(), "backup is a source folder"),
                (read(generated.join("backup/apps/photocraft/Cargo.toml"))? == "base-profile = []\n", "backup is untouched"),
                (!generated.join("backup/projectile").exists() && !generated.join("backup/backup").exists(), "backup omits ProjectIle and itself"),
                (!generated.join("projectile").exists(), "ProjectIle not copied"),
                (!generated.join("apps/photocraft-cli").exists(), "cli not copied"),
                (!generated.join("apps/photocraft-web").exists(), "web not copied"),
                (!generated.join("xtask").exists(), "xtask not copied"),
                (!generated.join(".git/secret").exists() && !generated.join("target").exists(), "no vcs or build output"),
                (
                    !generated.join("plan").exists() && !generated.join("refs").exists() && !generated.join("font").exists(),
                    "no local-only folders or font redistribution",
                ),
                (!generated.join("backup/font").exists(), "the local Satoshi font is not copied into the backup"),
            ];
            match checks.iter().find(|(ok, _)| !ok) {
                Some((_, what)) => Err(format!("generated project check failed: {what}")),
                None => Ok(()),
            }
        };
        let result = check();
        let _ = fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn regenerate_rebuilds_with_saved_settings_and_refreshes_the_backup() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let options = Options { mcp: true, git_init: false, ..Options::default() };
        let first = generate_from_source(&spec("again", &root, options), &source, &mut |_| {})?.path;
        // Change the project and the source, then regenerate keeping a copy of the old folder.
        fs::write(first.join("PROJECTILE.md"), "my edits").map_err(|error| error.to_string())?;
        fs::write(source.join("crates/engine/Cargo.toml"), "engine source v2").map_err(|error| error.to_string())?;
        let check = (|| {
            let spec = crate::plan::regenerate_spec(&first, source.clone(), OnExisting::RenameAside)?;
            if !spec.options.mcp || spec.name != "again" || spec.destination != root {
                return Err(format!("saved settings were not restored: {spec:?}"));
            }
            let report = generate_from_source(&spec, &source, &mut |_| {})?;
            let guide = read(report.path.join("PROJECTILE.md"))?;
            let backup_engine = read(report.path.join("backup/crates/engine/Cargo.toml"))?;
            let kept = report.replaced.ok_or("the old folder was not kept")?;
            if guide.contains("my edits") || backup_engine != "engine source v2" || read(kept.join("PROJECTILE.md"))? != "my edits" {
                return Err("regeneration did not refresh the project and backup, or lost the old copy".into());
            }
            if crate::plan::regenerate_spec(&report.path, source.clone(), OnExisting::Stop).is_ok() {
                return Err("Stop must not be accepted for regeneration".into());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        check
    }

    #[test]
    fn viewer_profile_copies_the_image_and_opens_it_at_startup() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let image = root.join("picture.png");
        let mut image_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut image_bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().map_err(|error| error.to_string())?.write_image_data(&[24, 48, 96, 255]).map_err(|error| error.to_string())?;
        }
        fs::write(&image, &image_bytes).map_err(|error| error.to_string())?;
        let options = Options { viewer_image: image.to_string_lossy().into_owned(), git_init: false, backup_source: false, ..Options::default() };
        let mut viewer = spec("view", &root, options);
        viewer.profile = Profile::Viewer;
        let result = generate_from_source(&viewer, &source, &mut |_| {});
        let check = (|| {
            let project = result?.path;
            let manifest = read(project.join("apps/photocraft/Cargo.toml"))?;
            let main = read(project.join("apps/photocraft/src/main.rs"))?;
            let backup = read(project.join("backup/apps/photocraft/Cargo.toml"))?;
            let backup_note = read(project.join(crate::plan::BACKUP_NOTE))?;
            let provenance = read(project.join(".projectile.json"))?;
            let copied = fs::read(project.join("assets/viewer/startup.png")).map_err(|error| error.to_string())?;
            let startup = project
                .join("apps/photocraft")
                .join("../../assets/viewer/startup.png")
                .canonicalize()
                .map_err(|error| format!("the generated app cannot resolve its Viewer image: {error}"))?;
            if !manifest.contains("default = [\"viewer-profile\"]")
                || !main.contains("include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\")")
                || !main.contains("\"startup.png\", startup")
                || !main.contains("app.open_failed(\"Viewer startup image\"")
                || !copied.starts_with(b"\x89PNG")
                || copied != image_bytes
                || startup != project.join("assets/viewer/startup.png").canonicalize().map_err(|error| error.to_string())?
            {
                return Err("viewer project lacks the feature, the startup path or the copied image".to_owned());
            }
            if backup != "base-profile = []\n"
                || !backup_note.contains("untouched copy")
                || !project.join("backup/Cargo.toml").is_file()
                || project.join("backup/backup").exists()
                || !provenance.contains("\"backup_source\": true")
            {
                return Err("Viewer must include a complete, untouched source backup regardless of the saved backup toggle".into());
            }
            let guide = read(project.join("PROJECTILE.md"))?;
            if !guide.contains("PhotoCore Viewer") || !guide.contains("cargo test -p photocraft-engine") {
                return Err("Viewer guide is missing its profile name or Rust commands".into());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        check
    }

    #[test]
    fn viewer_without_an_image_is_blocked() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let mut viewer = spec("blind", &root, Options::default());
        viewer.profile = Profile::Viewer;
        let result = generate_from_source(&viewer, &source, &mut |_| {});
        let entries = fs::read_dir(&root).map_err(|error| error.to_string())?.count();
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        if result.is_ok() || entries != 0 {
            return Err("a viewer without an image was generated".into());
        }
        Ok(())
    }

    #[test]
    fn android_option_writes_the_gradle_project() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let options = Options {
            android: true,
            android_min_sdk: 28,
            android_target_sdk: 36,
            android_java_version: 21,
            android_gradle_version: "9.6.0".into(),
            git_init: false,
            backup_source: false,
            ..Options::default()
        };
        let result = generate_from_source(&spec("droid", &root, options), &source, &mut |_| {});
        let check = (|| {
            let project = result?.path;
            let gradle = read(project.join("android/app/build.gradle"))?;
            let crate_manifest = read(project.join("apps/photocraft-android/Cargo.toml"))?;
            let android_readme = read(project.join("android/README.md"))?;
            let guide = read(project.join("PROJECTILE.md"))?;
            let activity = project.join("android/app/src/main/java/ai/storyteller/photocore/droid/MainActivity.java");
            let wrapper = read(project.join("android/gradle/wrapper/gradle-wrapper.properties"))?;
            if !gradle.contains("minSdk 28")
                || !gradle.contains("targetSdk 36")
                || !gradle.contains("JavaVersion.VERSION_17")
                || !wrapper.contains("gradle-9.6.0-bin.zip")
                || !crate_manifest.contains("cdylib")
                || !activity.is_file()
            {
                return Err("android scaffold is incomplete".to_owned());
            }

            if !android_readme.contains("javac -version")
                || !android_readme.contains("gradle --version")
                || !android_readme.contains("developer.android.com/studio#command-line-tools-only")
                || !android_readme.contains("JDK 21")
                || !android_readme.contains("Gradle 9.6.0")
                || !guide.contains("cargo install cargo-ndk")
                || !guide.contains("sdkmanager --licenses")
                || !guide.contains("adb install")
            {
                return Err("Android guide is missing tool checks, installation links or development commands".into());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        check
    }

    #[test]
    fn menu_options_generate_selected_menus_and_egui_visual_debug() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let options = Options {
            hidden_menus: vec!["Edit".into()],
            hidden_submenus: vec!["File > Export".into()],
            show_menu_bar: true,
            visual_debug_menu: true,
            git_init: false,
            backup_source: false,
            ..Options::default()
        };
        let result = generate_from_source(&spec("menus", &root, options), &source, &mut |_| {});
        let check = (|| {
            let project = result?.path;
            let menu = read(project.join("crates/ui-egui/src/menus.rs"))?;
            let app = read(project.join("apps/photocraft/src/main.rs"))?;
            let lib = read(project.join("crates/ui-egui/src/lib.rs"))?;
            let debug = read(project.join("crates/ui-egui/src/projectile_debug.rs"))?;
            if !menu.contains("TOP_MENUS: [&str; 10] = [\"File\", \"Image\"")
                || menu.contains("\"Edit\", \"Image\"")
                || !menu.contains("File > Export")
                || !menu.contains("projectile.visualDebug")
                || !menu.contains("if id == \"projectile.visualDebug\"")
                || !app.contains("app.ui.panels.menu_bar = true")
                || !lib.contains("mod projectile_debug;")
                || !lib.contains("projectile_debug::show(ctx)")
                || !debug.contains("ctx.inspection_ui(ui)")
            {
                return Err("generated menu visibility or egui visual debug wiring is incomplete".into());
            }
            let hidden = generate_from_source(
                &spec("menu-less", &root, Options { show_menu_bar: false, git_init: false, backup_source: false, ..Options::default() }),
                &source,
                &mut |_| {},
            )?
            .path;
            if !read(hidden.join("crates/ui-egui/src/menus.rs"))?.contains("TOP_MENUS: [&str; 0] = [];")
                || !read(hidden.join("apps/photocraft/src/main.rs"))?.contains("app.ui.panels.menu_bar = false")
            {
                return Err("the no-menu-bar option didn't hide both in-window and native menu titles".into());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        check
    }

    #[test]
    fn mcp_option_keeps_the_cli_and_writes_a_bridge_config() -> Result<(), String> {
        let root = temporary_directory()?;
        let source = root.join("source");
        fs::create_dir(&source).map_err(|error| error.to_string())?;
        source_fixture(&source)?;
        let options = Options { mcp: true, mcp_port: 7999, backup_source: false, include_docs: false, git_init: false, ..Options::default() };
        let result = (|| {
            let generated = generate_from_source(&spec("agents", &root, options), &source, &mut |_| {})?.path;
            let manifest = read(generated.join("Cargo.toml"))?;
            let mcp: serde_json::Value = serde_json::from_str(&read(generated.join(".mcp.json"))?).map_err(|error| error.to_string())?;
            let args = mcp["mcpServers"]["photocore"]["args"].as_array().cloned().unwrap_or_default();
            let gitignore = read(generated.join(".gitignore"))?;
            if !manifest.contains("\"apps/photocraft-cli\"")
                || manifest.contains("exclude")
                || !generated.join("apps/photocraft-cli/src/main.rs").exists()
                || !args.iter().any(|a| a == "127.0.0.1:7999")
                || !gitignore.contains(".projectile/control.token")
                || generated.join("docs").exists()
                || generated.join("backup").exists()
            {
                return Err("MCP project is missing the CLI, bridge config or token ignore, or kept docs/backup".to_owned());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&root);
        result
    }

    /// A fixture PhotoCraft source in its own temporary folder (so it never counts as output).
    fn fixture_source() -> Result<PathBuf, String> {
        let source = temporary_directory()?;
        source_fixture(&source)?;
        Ok(source)
    }

    #[test]
    fn generation_never_overwrites_an_existing_project() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let existing = root.join("protected");
        fs::create_dir(&existing).map_err(|error| error.to_string())?;
        fs::write(existing.join("keep.txt"), "keep").map_err(|error| error.to_string())?;
        let result = generate_from_source(&spec("protected", &root, Options::default()), &source, &mut |_| {});
        let contents = fs::read_to_string(existing.join("keep.txt")).map_err(|error| error.to_string())?;
        let entries = fs::read_dir(&existing).map_err(|error| error.to_string())?.count();
        let _ = fs::remove_dir_all(&source);
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        if result.is_ok() || contents != "keep" || entries != 1 {
            return Err("existing project was overwritten".into());
        }
        Ok(())
    }

    #[test]
    fn replace_moves_the_old_project_aside_and_keeps_it() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let existing = root.join("again");
        fs::create_dir(&existing).map_err(|error| error.to_string())?;
        fs::write(existing.join("mine.txt"), "keep me").map_err(|error| error.to_string())?;
        let options = Options { on_existing: OnExisting::RenameAside, git_init: false, backup_source: false, ..Options::default() };
        let result = generate_from_source(&spec("again", &root, options), &source, &mut |_| {});
        let check = (|| {
            let report = result?;
            let aside = report.replaced.ok_or("no aside folder reported")?;
            let kept = fs::read_to_string(aside.join("mine.txt")).map_err(|error| error.to_string())?;
            if kept != "keep me" || !report.path.join("PROJECTILE.md").exists() || report.path.join("mine.txt").exists() {
                return Err("the old project was not kept aside, or the new one is incomplete".to_owned());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        check
    }

    #[test]
    fn merge_overwrites_generated_files_and_keeps_the_users_own() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let existing = root.join("merged");
        fs::create_dir_all(existing.join("apps/photocraft")).map_err(|error| error.to_string())?;
        fs::write(existing.join("notes.txt"), "my notes").map_err(|error| error.to_string())?;
        fs::write(existing.join("PROJECTILE.md"), "stale guide").map_err(|error| error.to_string())?;
        let options = Options { on_existing: OnExisting::Merge, git_init: false, backup_source: false, mcp: true, ..Options::default() };
        // Twice: a second merge must not fail on files the first one wrote, nor duplicate appended lines.
        let first = generate_from_source(&spec("merged", &root, options.clone()), &source, &mut |_| {});
        let second = generate_from_source(&spec("merged", &root, options), &source, &mut |_| {});
        let check = (|| {
            first?;
            second?;
            let notes = read(existing.join("notes.txt"))?;
            let guide = read(existing.join("PROJECTILE.md"))?;
            let gitignore = read(existing.join(".gitignore"))?;
            if notes != "my notes" || guide.contains("stale guide") || gitignore.matches(".projectile/control.token").count() != 1 {
                return Err(format!("merge lost user files, kept a stale generated file, or duplicated lines: {gitignore:?}"));
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        check
    }

    #[test]
    #[cfg(unix)]
    fn a_failed_generation_leaves_nothing_half_made() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let (root, source) = (temporary_directory()?, fixture_source()?);
        // Planning only reads metadata; copying this unreadable file fails half-way through.
        let locked = source.join("docs/locked.md");
        fs::write(&locked, "secret").map_err(|error| error.to_string())?;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).map_err(|error| error.to_string())?;
        let result = generate_from_source(&spec("broken", &root, Options { git_init: false, ..Options::default() }), &source, &mut |_| {});
        let leftovers: Vec<_> = fs::read_dir(&root).map_err(|error| error.to_string())?.filter_map(Result::ok).map(|e| e.file_name()).collect();
        let _ = fs::set_permissions(&locked, fs::Permissions::from_mode(0o644));
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        if result.is_ok() || !leftovers.is_empty() {
            return Err(format!("a failed generation left {leftovers:?} ({result:?})"));
        }
        Ok(())
    }

    #[test]
    fn delete_removes_the_old_folder_first() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let existing = root.join("fresh");
        fs::create_dir(&existing).map_err(|error| error.to_string())?;
        fs::write(existing.join("old.txt"), "old").map_err(|error| error.to_string())?;
        let options = Options { on_existing: OnExisting::Delete, git_init: false, backup_source: false, ..Options::default() };
        let result = generate_from_source(&spec("fresh", &root, options), &source, &mut |_| {});
        let check = (|| {
            let report = result?;
            if report.path.join("old.txt").exists() || !report.path.join("PROJECTILE.md").exists() {
                return Err("the old folder was not deleted, or the new project is incomplete".to_owned());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&root);
        check
    }

    #[test]
    fn home_and_root_are_never_replaced() {
        assert!(crate::plan::guard_existing(Path::new("/")).is_err());
        assert!(crate::plan::guard_existing(Path::new("/home")).is_err());
        if let Some(home) = std::env::var_os("HOME") {
            assert!(crate::plan::guard_existing(Path::new(&home)).is_err());
        }
        assert!(crate::plan::guard_existing(&std::env::temp_dir().join("projectile-some-project")).is_ok());
    }

    #[test]
    fn generation_rejects_out_of_range_startup_options() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let options = Options { width: 100, height: 100, scale_percent: 500, ..Options::default() };
        let result = generate_from_source(&spec("bad-size", &root, options), &source, &mut |_| {});
        let entries = fs::read_dir(&root).map_err(|error| error.to_string())?.count();
        let _ = fs::remove_dir_all(&source);
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        if result.is_ok() || entries != 0 {
            return Err("invalid startup settings created a project".into());
        }
        Ok(())
    }

    #[test]
    fn preview_profiles_cannot_be_generated() -> Result<(), String> {
        let (root, source) = (temporary_directory()?, fixture_source()?);
        let mut full = spec("full", &root, Options::default());
        full.profile = Profile::Full;
        let result = generate_from_source(&full, &source, &mut |_| {});
        let entries = fs::read_dir(&root).map_err(|error| error.to_string())?.count();
        let _ = fs::remove_dir_all(&source);
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        if result.is_ok() || entries != 0 {
            return Err("a preview-only profile generated a project".into());
        }
        Ok(())
    }

    #[test]
    fn generate_requires_a_valid_photocraft_source() -> Result<(), String> {
        let root = temporary_directory()?;
        let mut bad = spec("nowhere", &root, Options::default());
        bad.source = root.clone();
        let result = super::generate(&bad, &mut |_| {});
        let entries = fs::read_dir(&root).map_err(|error| error.to_string())?.count();
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        if result.is_ok() || entries != 0 {
            return Err("generation ran without a PhotoCraft source".into());
        }
        Ok(())
    }
}
