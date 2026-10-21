#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod access;
mod android;
mod command;
mod generator;
mod git;
mod help;
mod mcp;
mod pages;
mod plan;
mod rust;
mod source;
mod system;
mod theme;
mod toolchain;
mod tools;
mod tree;

use access::Access;
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use plan::{Inventory, Options, Plan, Profile, ProjectSpec};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use theme::{AppTheme, Palette};

const APP_ID: &str = "ai.storytold.projectile";
const SETTINGS_KEY: &str = "projectile.settings.v2";
const LEGACY_SETTINGS_KEY: &str = "projectile.settings.v1";
/// Bumped when the default window layout changes, so upgraders get the new size once.
const LAYOUT_VERSION: u32 = 2;
const MAX_LOG_LINES: usize = 2000;
const NO_SOURCE: &str = "No PhotoCraft source folder found. Choose it in Create Project › Source (the root of a PhotoCraft checkout).";

/// `claude mcp add` line for this very executable, wherever ProjectIle is installed.
pub fn mcp_command() -> String {
    let exe = std::env::current_exe().map_or_else(|_| "projectile".to_owned(), |path| path.display().to_string());
    let exe = if exe.contains(' ') { format!("\"{exe}\"") } else { exe };
    format!("claude mcp add projectile -- {exe} --mcp")
}

/// Portrait by default: a generator is a tall form, like the VST3 SDK Project Generator.
pub const WINDOW_PRESETS: &[(&str, f32, f32)] =
    &[("Portrait compact", 600.0, 800.0), ("Portrait", 720.0, 960.0), ("Portrait tall", 820.0, 1120.0), ("Landscape", 1200.0, 800.0)];
const DEFAULT_WINDOW: [f32; 2] = [720.0, 960.0];
const MIN_WINDOW: [f32; 2] = [560.0, 640.0];
const SCALE_RANGE: std::ops::RangeInclusive<u32> = 80..=160;

pub const TIPS: &[&str] = &[
    "Cargo.toml declares direct dependencies; Cargo.lock records the exact versions a build used.",
    "Rust's ownership rules let programs avoid a garbage collector while preventing data races.",
    "Run `cargo check` for a fast compile check, `cargo test` for tests, and `cargo fmt` to format.",
    "Base keeps PhotoCraft's native UI while its engine session restricts the available commands.",
    "Small, typed modules make a Rust project easier to read, test, and change safely.",
    "In PhotoCraft every menu item is an engine command with an id, like `paint.bucket`.",
    "Photoshop shortcuts work in PhotoCraft: B for Brush, E for Eraser, G for Paint Bucket.",
    "`git diff` in a generated project shows exactly what you changed since ProjectIle made it.",
    "Claude Code reads `.mcp.json` in a project folder and offers to start the servers it lists.",
    "`Result<T, E>` and the `?` operator are how Rust code reports errors instead of crashing.",
];

pub mod keys {
    use super::{Key, KeyboardShortcut, Modifiers};
    pub const GENERATE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::G);
    pub const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
    pub const CONFIGURE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F5);
    pub const HELP: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F1);
    pub const FULLSCREEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F11);
    pub const ZOOM_IN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Equals);
    pub const ZOOM_IN_PLUS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Plus);
    pub const ZOOM_OUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Minus);
    pub const ZOOM_RESET: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0);
    pub const ACCESS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::U);
    pub const COPY_LOG: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::C);
    pub const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
    pub const SCREENSHOT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F12);
    pub const PAGES: [KeyboardShortcut; 6] = [
        KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1),
        KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2),
        KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3),
        KeyboardShortcut::new(Modifiers::COMMAND, Key::Num4),
        KeyboardShortcut::new(Modifiers::COMMAND, Key::Num5),
        KeyboardShortcut::new(Modifiers::COMMAND, Key::Num6),
    ];
}

fn main() -> eframe::Result {
    if std::env::args().any(|arg| arg == "--mcp") {
        if let Err(error) = mcp::serve() {
            eprintln!("projectile --mcp: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_title("ProjectIle — PhotoCraft project generator")
            .with_inner_size(DEFAULT_WINDOW)
            .with_min_inner_size(MIN_WINDOW),
        // eframe restores the last size and position itself; ProjectIle never fights it.
        persist_window: true,
        centered: true,
        ..Default::default()
    };
    eframe::run_native("ProjectIle", options, Box::new(|cc| Ok(Box::new(ProjectIle::new(cc)))))
}

/// Use PhotoCraft's Inter and JetBrains Mono when the chosen source has them (ProjectIle bundles
/// no font files); otherwise egui's built-in fonts. The `strong` family always exists.
/// Satoshi (when the user has it) for headings, buttons and tabs, Inter for body text, JetBrains
/// Mono for code; PhotoCraft's own fonts are read from the source. The `strong` and `medium`
/// families always exist, falling back to Inter and egui's defaults.
fn install_fonts(ctx: &egui::Context, source: Option<&std::path::Path>) -> Option<PathBuf> {
    let loaded = source::fonts(source);
    let mut fonts = egui::FontDefinitions::default();
    let defaults = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    let mut strong = defaults.clone();
    let mut medium = defaults;
    let add = |fonts: &mut egui::FontDefinitions, name: &str, bytes: Option<Vec<u8>>| -> bool {
        match bytes {
            Some(bytes) => {
                fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
                true
            }
            None => false,
        }
    };
    // Fallbacks first, so the preferred face ends up at the front of each family.
    if add(&mut fonts, "projectile-inter", loaded.regular) {
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            list.insert(0, "projectile-inter".into());
        }
        strong.insert(0, "projectile-inter".into());
        medium.insert(0, "projectile-inter".into());
    }
    if add(&mut fonts, "projectile-jetbrains-mono", loaded.mono)
        && let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace)
    {
        list.insert(0, "projectile-jetbrains-mono".into());
    }
    if add(&mut fonts, "projectile-inter-semibold", loaded.semibold) {
        strong.insert(0, "projectile-inter-semibold".into());
    }
    if add(&mut fonts, "projectile-inter-medium", loaded.medium) {
        medium.insert(0, "projectile-inter-medium".into());
    }
    if add(&mut fonts, "projectile-satoshi-bold", loaded.satoshi_bold) {
        strong.insert(0, "projectile-satoshi-bold".into());
    }
    if add(&mut fonts, "projectile-satoshi-medium", loaded.satoshi_medium) {
        medium.insert(0, "projectile-satoshi-medium".into());
    }
    fonts.families.insert(egui::FontFamily::Name("strong".into()), strong);
    fonts.families.insert(egui::FontFamily::Name("medium".into()), medium);
    ctx.set_fonts(fonts);
    loaded.satoshi_from
}

pub fn next_tip_index(previous: usize) -> usize {
    (previous % TIPS.len() + 1) % TIPS.len()
}

pub fn screenshot_directory() -> Result<PathBuf, String> {
    std::env::var_os("XDG_PICTURES_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|pictures| pictures.join("ProjectIle").join("Screenshots"))
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from).map(|home| home.join("Pictures").join("ProjectIle").join("Screenshots")))
        .ok_or_else(|| "Cannot locate a screenshots folder: neither XDG_PICTURES_DIR nor HOME is set.".to_owned())
}

fn encode_screenshot(path: &std::path::Path, image: &egui::ColorImage) -> Result<(), String> {
    let width = u32::try_from(image.size[0]).map_err(|error| format!("Screenshot width is invalid: {error}"))?;
    let height = u32::try_from(image.size[1]).map_err(|error| format!("Screenshot height is invalid: {error}"))?;
    let expected_len = image.size[0].checked_mul(image.size[1]).ok_or_else(|| "Screenshot dimensions overflow.".to_owned())?;
    if width == 0 || height == 0 || image.pixels.len() != expected_len {
        return Err("Screenshot image has invalid dimensions or pixel data.".into());
    }
    let rgba = image.pixels.iter().flat_map(|pixel| pixel.to_array()).collect::<Vec<_>>();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("Could not create screenshot {}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| format!("Could not encode screenshot header: {error}"))?;
    writer.write_image_data(&rgba).map_err(|error| format!("Could not encode screenshot pixels: {error}"))
}

fn set_screenshot_clipboard(image: &egui::ColorImage) -> Result<(), String> {
    let bytes = image.pixels.iter().flat_map(|pixel| pixel.to_array()).collect::<Vec<_>>();
    let mut clipboard = arboard::Clipboard::new().map_err(|error| format!("Could not access clipboard: {error}"))?;
    clipboard
        .set_image(arboard::ImageData { width: image.size[0], height: image.size[1], bytes: std::borrow::Cow::Owned(bytes) })
        .map_err(|error| format!("Could not copy screenshot to clipboard: {error}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Welcome,
    Create,
    Rust,
    Preferences,
    Help,
    About,
}

impl Page {
    pub fn help_topic(self) -> &'static str {
        match self {
            Self::Welcome | Self::Create => "start",
            Self::Rust => "rust",
            Self::Preferences => "preferences",
            Self::Help => "help",
            Self::About => "open-source",
        }
    }

    pub const ALL: [Page; 6] = [Page::Welcome, Page::Create, Page::Rust, Page::Preferences, Page::Help, Page::About];

    pub fn label(self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::Create => "Create Project",
            Self::Rust => "Toolchains",
            Self::Preferences => "Preferences",
            Self::Help => "Help",
            Self::About => "About",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedSettings {
    /// Root of the PhotoCraft checkout to generate from, chosen at run time.
    pub source_dir: String,
    /// Branch, tag, or commit requested by the optional upstream checkout action.
    pub source_version: String,
    pub project_name: String,
    pub destination: String,
    pub profile: Profile,
    pub options: Options,
    pub theme: AppTheme,
    pub scale_percent: u32,
    pub access: Access,
    pub tip_index: usize,
    pub open_folder_after: bool,
    pub open_vscode_after: bool,
    /// After generating: build the editor with Cargo, and (if `run_after`) start it.
    pub build_after: bool,
    pub run_after: bool,
    /// How dialogs and spacing are arranged (Automatic follows the detected desktop).
    pub layout_choice: system::LayoutChoice,
    /// Heights the user dragged lists to, by list id.
    pub list_heights: std::collections::BTreeMap<String, f32>,
    pub layout_version: u32,
    /// Interface language (BCP 47 tag). Only "en" exists for now.
    pub language: String,
}

impl Default for SavedSettings {
    fn default() -> Self {
        Self {
            source_dir: String::new(),
            source_version: source::DEFAULT_BRANCH.into(),
            project_name: "my-photocore".into(),
            destination: default_destination().to_string_lossy().into_owned(),
            profile: Profile::Base,
            options: Options::default(),
            theme: AppTheme::Pro,
            scale_percent: 100,
            access: Access::default(),
            tip_index: 0,
            open_folder_after: true,
            open_vscode_after: false,
            build_after: false,
            run_after: false,
            layout_choice: system::LayoutChoice::Auto,
            list_heights: std::collections::BTreeMap::new(),
            layout_version: 0,
            language: "en".into(),
        }
    }
}

/// The first release's settings (`projectile.settings.v1`); only read, to migrate.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct LegacySettings {
    project_name: String,
    destination: String,
    window_width: u32,
    window_height: u32,
    ui_scale_percent: u32,
    projectile_scale_percent: u32,
    theme: AppTheme,
    tip_index: usize,
    open_after_generate: bool,
}

impl Default for LegacySettings {
    fn default() -> Self {
        let options = Options::default();
        let current = SavedSettings::default();
        Self {
            project_name: current.project_name,
            destination: current.destination,
            window_width: options.width,
            window_height: options.height,
            ui_scale_percent: options.scale_percent,
            projectile_scale_percent: 100,
            theme: AppTheme::Pro,
            tip_index: 0,
            open_after_generate: true,
        }
    }
}

impl From<LegacySettings> for SavedSettings {
    fn from(old: LegacySettings) -> Self {
        let options = Options { width: old.window_width, height: old.window_height, scale_percent: old.ui_scale_percent, ..Options::default() };
        Self {
            project_name: old.project_name,
            destination: old.destination,
            options,
            theme: old.theme,
            scale_percent: old.projectile_scale_percent,
            tip_index: old.tip_index,
            open_folder_after: old.open_after_generate,
            ..Self::default()
        }
    }
}

impl SavedSettings {
    /// Clamp anything a hand-edited or older settings file could put out of range.
    fn sanitized(mut self) -> Self {
        let defaults = Options::default();
        if self.profile == Profile::Viewer {
            self.options.backup_source = true;
        }
        if plan::validate_options(&Options { mcp: false, ..self.options.clone() }).is_err() {
            self.options.width = defaults.width;
            self.options.height = defaults.height;
            self.options.scale_percent = defaults.scale_percent;
        }
        if self.options.mcp_port < 1024 {
            self.options.mcp_port = defaults.mcp_port;
        }
        self.scale_percent = self.scale_percent.clamp(*SCALE_RANGE.start(), *SCALE_RANGE.end());
        if self.language != "en" {
            self.language = "en".into();
        }
        self
    }
}

fn default_destination() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("Projects")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Info,
    Success,
    Warning,
    Error,
}

/// Diff lines of one reviewed edit (shared, so the cache hands out cheap clones).
pub type ReviewLines = Result<std::sync::Arc<Vec<plan::DiffLine>>, String>;

enum JobMessage {
    Event(generator::Event),
    Finished(Result<generator::Report, String>),
}

struct Job {
    rx: mpsc::Receiver<JobMessage>,
    done: u64,
    total: u64,
}

enum TaskMessage {
    Line(String),
    Finished(Result<(), String>),
}

/// The "Regenerate" dialog: which project, what it records, and the user's choices.
#[derive(Debug, Clone)]
pub struct Regenerate {
    pub dir: PathBuf,
    pub saved: plan::SavedProject,
    pub mode: plan::OnExisting,
    /// Use the profile and options saved in the project (otherwise the current settings).
    pub use_saved: bool,
}

/// Which list the Rust tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustView {
    Installed,
    Releases,
    Channels,
}

/// A source file shown in Source review: its lines, or why it can't be shown.
pub type FileLines = Result<std::sync::Arc<Vec<String>>, String>;

pub struct ProjectIle {
    pub settings: SavedSettings,
    pub page: Page,
    pub rust: toolchain::ToolchainStatus,
    pub logs: Vec<(LogKind, String)>,
    pub notice: Option<(LogKind, String)>,
    pub inventory: Result<Inventory, String>,
    pub plan: Option<Plan>,
    plan_key: Option<(String, String, String, Profile, Options)>,
    pub review_index: usize,
    pub component_filter: String,
    pub tree_expanded: HashSet<String>,
    pub listings: tree::Listings,
    /// File picked in the Components tree; Source review shows it (or its diff when edited).
    pub review_file: Option<String>,
    file_cache: Option<(String, FileLines)>,
    pub rust_state: rust::RustState,
    pub rust_view: RustView,
    pub rust_filter: String,
    /// An action waiting for the user's confirmation in a dialog.
    /// A command waiting for the user's confirmation in a dialog.
    pub task_confirm: Option<command::Task>,
    pub rust_log: Vec<(LogKind, String)>,
    task_job: Option<(command::Task, mpsc::Receiver<TaskMessage>)>,
    /// Checkout directory that becomes the source after a successful Clone / Update task.
    source_after_task: Option<PathBuf>,
    /// Help topic to show (and scroll to once) on the Help tab.
    pub help_topic: Option<&'static str>,
    pub help_scroll: bool,
    pub help_filter: String,
    /// "Why?" explanations currently unfolded, by key.
    pub why_open: HashSet<String>,
    /// Pending "delete the existing folder?" question: path, file count, made by ProjectIle.
    pub delete_confirm: Option<(PathBuf, u64, bool)>,
    /// Set by the dialog for exactly one generation.
    pub delete_confirmed: bool,
    /// The open "Regenerate" dialog.
    pub regen: Option<Regenerate>,
    /// Android tools found by the last check, and the check in progress.
    pub android_tools: Option<tools::AndroidTools>,
    android_rx: Option<mpsc::Receiver<tools::AndroidTools>>,
    pub last_generated: Option<PathBuf>,
    pub palette: Palette,
    /// Where the Satoshi font was found, if anywhere.
    pub satoshi: Option<PathBuf>,
    pub system: system::System,
    pub git: git::GitState,
    pub commit_message: String,
    pub repo_name: String,
    pub repo_private: bool,
    /// Interface scale chosen in Preferences, applied with the Apply button.
    pub pending_scale: u32,
    plan_generation: u64,
    review_cache: Option<(usize, u64, ReviewLines)>,
    job: Option<Job>,
    request_screenshot: bool,
    toggle_access: bool,
    window_checked: bool,
}

impl ProjectIle {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = cc
            .storage
            .and_then(|storage| {
                eframe::get_value::<SavedSettings>(storage, SETTINGS_KEY)
                    .or_else(|| eframe::get_value::<LegacySettings>(storage, LEGACY_SETTINGS_KEY).map(SavedSettings::from))
            })
            .unwrap_or_default();
        Self::with_settings(&cc.egui_ctx, settings)
    }

    pub fn with_settings(ctx: &egui::Context, settings: SavedSettings) -> Self {
        let mut settings = settings.sanitized();
        settings.tip_index = next_tip_index(settings.tip_index);
        let rust = toolchain::detect();
        let inventory = match source::detect(&settings.source_dir) {
            Some(root) => {
                settings.source_dir = root.to_string_lossy().into_owned();
                plan::scan(&root)
            }
            None => Err(NO_SOURCE.into()),
        };
        let satoshi = install_fonts(ctx, inventory.as_ref().ok().map(|inventory| inventory.root.as_path()));
        let mut app = Self {
            palette: Palette::get(settings.theme, &settings.access),
            satoshi,
            system: system::System::detect(),
            git: git::GitState::default(),
            commit_message: String::new(),
            repo_name: String::new(),
            repo_private: true,
            pending_scale: settings.scale_percent,
            settings,
            page: Page::Create,
            rust,
            logs: Vec::new(),
            notice: None,
            inventory,
            plan: None,
            plan_key: None,
            review_index: 0,
            component_filter: String::new(),
            tree_expanded: ["g:app", "g:crates", "g:mcp", "g:docs", "g:backup", "g:projectile-snapshot", "g:vscode", "g:git", "g:pin"].into_iter().map(String::from).collect(),
            listings: tree::Listings::default(),
            review_file: None,
            file_cache: None,
            rust_state: rust::RustState::default(),
            rust_view: RustView::Installed,
            rust_filter: String::new(),
            task_confirm: None,
            rust_log: Vec::new(),
            task_job: None,
            source_after_task: None,
            help_topic: None,
            help_scroll: false,
            help_filter: String::new(),
            why_open: HashSet::new(),
            delete_confirm: None,
            delete_confirmed: false,
            regen: None,
            android_tools: None,
            android_rx: None,
            last_generated: None,
            plan_generation: 0,
            review_cache: None,
            job: None,
            request_screenshot: false,
            toggle_access: false,
            window_checked: false,
        };
        // egui's own Ctrl+/- zoom would bypass the saved scale; ProjectIle handles those keys.
        ctx.options_mut(|options| options.zoom_with_keyboard = false);
        app.apply_appearance(ctx);
        app.rescan_rust();
        app.refresh_git();
        app.log(LogKind::Info, "ProjectIle started.".into());
        app.log(if app.rust.ready { LogKind::Success } else { LogKind::Warning }, format!("Rust toolchain: {}", app.rust.summary()));
        match &app.inventory {
            Ok(inventory) => {
                let (files, bytes) = inventory.totals();
                let revision = inventory.revision.clone().unwrap_or_else(|| "revision unknown".into());
                app.log(LogKind::Info, format!("PhotoCraft source: {} ({revision}; {files} files, {}).", inventory.root.display(), plan::human_bytes(bytes)));
            }
            Err(error) => app.log(LogKind::Error, format!("PhotoCraft source unavailable: {error}")),
        }
        app.log(LogKind::Info, format!("Tip: {}", app.tip()));
        app
    }

    pub fn tip(&self) -> &'static str {
        TIPS.get(self.settings.tip_index).copied().unwrap_or_default()
    }

    pub fn log(&mut self, kind: LogKind, line: String) {
        self.logs.push((kind, line));
        if self.logs.len() > MAX_LOG_LINES {
            let excess = self.logs.len() - MAX_LOG_LINES;
            self.logs.drain(..excess);
        }
    }

    pub fn announce(&mut self, kind: LogKind, message: String) {
        self.log(kind, message.clone());
        self.notice = Some((kind, message));
    }

    pub fn apply_appearance(&mut self, ctx: &egui::Context) {
        self.palette = Palette::get(self.settings.theme, &self.settings.access);
        theme::apply(ctx, self.settings.theme, &self.settings.access, self.settings.scale_percent);
        if self.layout().compact {
            // Tiling window managers give each window less room: tighten the spacing a little.
            ctx.all_styles_mut(|style| {
                style.spacing.item_spacing.y = (style.spacing.item_spacing.y - 2.0).max(4.0);
                style.spacing.button_padding.y = (style.spacing.button_padding.y - 1.0).max(3.0);
            });
        }
    }

    pub fn set_scale(&mut self, ctx: &egui::Context, percent: u32) {
        self.settings.scale_percent = percent.clamp(*SCALE_RANGE.start(), *SCALE_RANGE.end());
        self.apply_appearance(ctx);
    }

    pub fn spec(&self) -> ProjectSpec {
        ProjectSpec {
            source: PathBuf::from(self.settings.source_dir.trim()),
            name: self.settings.project_name.trim().to_owned(),
            destination: PathBuf::from(self.settings.destination.trim()),
            profile: self.settings.profile,
            options: self.settings.options.clone(),
        }
    }

    /// Point ProjectIle at another PhotoCraft checkout: validate, rescan, reload its fonts.
    pub fn set_source(&mut self, ctx: &egui::Context, folder: &std::path::Path) {
        match source::validate(folder).and_then(|root| plan::scan(&root)) {
            Ok(inventory) => {
                self.settings.source_dir = inventory.root.to_string_lossy().into_owned();
                let version = source::photocraft_version(&inventory.root).map_or_else(String::new, |v| format!("PhotoCraft {v}, "));
                let revision = inventory.revision.clone().unwrap_or_else(|| "revision unknown".into());
                let message = format!("Source set to {} ({version}{revision}).", inventory.root.display());
                self.satoshi = install_fonts(ctx, Some(&inventory.root));
                self.listings.clear();
                self.review_file = None;
                self.file_cache = None;
                self.inventory = Ok(inventory);
                self.announce(LogKind::Success, message);
            }
            Err(error) => {
                self.settings.source_dir = folder.to_string_lossy().into_owned();
                self.inventory = Err(error.clone());
                self.announce(LogKind::Error, error);
            }
        }
        // Rebuild now, so a stale plan can never report the old source as ready to generate.
        self.plan_key = None;
        self.refresh_plan();
    }

    /// Build the explicit source checkout task. If the selected folder does not exist it clones
    /// the upstream; otherwise it fetches tags and fast-forwards the requested ref.
    pub fn source_sync_task(&self) -> Result<(command::Task, PathBuf), String> {
        let git = self.rust.git_path.clone().ok_or("git was not found; install it to clone or update PhotoCraft source.")?;
        let ref_name = source::validate_ref(&self.settings.source_version)?;
        let folder = if self.settings.source_dir.trim().is_empty() {
            default_destination().join("photocore")
        } else {
            PathBuf::from(self.settings.source_dir.trim())
        };
        if folder.exists() {
            let root = source::validate(&folder)?;
            let checkout = command::Task {
                title: "Select PhotoCraft source version".into(),
                explanation: format!("Checks out `{ref_name}` in {} after downloading current upstream refs.", root.display()),
                program: git.clone(),
                args: vec!["checkout".into(), "--detach".into(), ref_name],
                cwd: Some(root.clone()),
                log: command::Log::Main,
                then: command::Then::Nothing,
            };
            Ok((
                command::Task {
                    title: "Update PhotoCraft source".into(),
                    explanation: format!("Downloads current branches and release tags from {} and selects the requested source version.", source::UPSTREAM_REPOSITORY),
                    program: git,
                    args: vec!["fetch".into(), "--tags".into(), "--prune".into(), "origin".into()],
                    cwd: Some(root.clone()),
                    log: command::Log::Main,
                    then: command::Then::Task(Box::new(checkout)),
                },
                root,
            ))
        } else {
            let parent = folder.parent().ok_or("Choose a source folder with a parent directory.")?;
            if !parent.is_dir() {
                return Err(format!("Create {} first, then choose it as the parent of the source checkout.", parent.display()));
            }
            Ok((
                command::Task {
                    title: "Clone PhotoCraft source".into(),
                    explanation: format!("Clones `{ref_name}` from {} into {}.", source::UPSTREAM_REPOSITORY, folder.display()),
                    program: git,
                    args: vec!["clone".into(), "--branch".into(), ref_name, "--single-branch".into(), source::UPSTREAM_REPOSITORY.into(), folder.to_string_lossy().into_owned()],
                    cwd: None,
                    log: command::Log::Main,
                    then: command::Then::Nothing,
                },
                folder,
            ))
        }
    }

    /// Ask for the normal task confirmation, then select the checkout when the task succeeds.
    pub fn confirm_source_sync(&mut self) {
        match self.source_sync_task() {
            Ok((task, folder)) => {
                self.source_after_task = Some(folder);
                self.task_confirm = Some(task);
            }
            Err(error) => self.announce(LogKind::Warning, error),
        }
    }

    /// Switch to the Help tab on `topic`.
    pub fn show_help(&mut self, topic: &'static str) {
        self.page = Page::Help;
        self.help_topic = Some(topic);
        self.help_scroll = true;
    }

    pub fn rescan_rust(&mut self) {
        self.rust = toolchain::detect();
        self.rust_state = rust::scan(self.rust.rustup_path.clone(), self.rust.rustc.clone(), self.rust.rustc_path.clone());
    }

    /// The layout in use: the user's choice, or what fits the detected desktop.
    pub fn layout(&self) -> system::Layout {
        self.system.layout(self.settings.layout_choice).0
    }

    /// The project Git and GitHub act on: the last generated one, else the Creates folder if it
    /// already exists.
    pub fn git_project(&self) -> Option<PathBuf> {
        self.last_generated.clone().or_else(|| self.spec().project_dir().filter(|dir| dir.is_dir()))
    }

    pub fn refresh_git(&mut self) {
        let project = self.git_project();
        self.git = git::scan(project.as_deref());
        if self.repo_name.is_empty()
            && let Some(name) = project.as_ref().and_then(|p| p.file_name())
        {
            self.repo_name = name.to_string_lossy().into_owned();
        }
    }

    /// Ask to confirm a task built elsewhere (git, gh), or report why it can't run.
    pub fn confirm_task(&mut self, task: Result<command::Task, String>) {
        match task {
            Ok(task) => self.task_confirm = Some(task),
            Err(error) => self.announce(LogKind::Warning, error),
        }
    }

    /// `cargo build -p photocraft` in `project`, then optionally start the built editor.
    pub fn build_task(&self, project: &std::path::Path, run: bool) -> Result<command::Task, String> {
        let cargo = self.rust.cargo_path.clone().ok_or("Cargo was not found; install Rust on the Rust tab.")?;
        let binary = project.join("target").join("debug").join(if cfg!(windows) { "photocraft.exe" } else { "photocraft" });
        Ok(command::Task {
            title: if run { "Build and run the editor".into() } else { "Build the editor".into() },
            explanation: "Compiles the generated editor with Cargo. The first build downloads and compiles every dependency and takes several minutes; later builds are quick.".into(),
            program: cargo,
            args: vec!["build".into(), "-p".into(), "photocraft".into()],
            cwd: Some(project.to_path_buf()),
            log: command::Log::Main,
            then: if run { command::Then::Launch(binary) } else { command::Then::Nothing },
        })
    }

    /// A project that can be regenerated: the last generated one, else the Creates folder, when
    /// it carries ProjectIle's metadata.
    pub fn regen_candidate(&self) -> Option<PathBuf> {
        self.git_project().filter(|dir| dir.join(".projectile.json").is_file())
    }

    /// Open the Regenerate dialog for `dir`.
    pub fn open_regenerate(&mut self, dir: PathBuf) {
        match plan::read_project(&dir) {
            Ok(saved) => self.regen = Some(Regenerate { dir, saved, mode: plan::OnExisting::RenameAside, use_saved: true }),
            Err(error) => self.announce(LogKind::Warning, error),
        }
    }

    /// Run the Regenerate dialog's choice: load the project's settings (or keep the current
    /// ones), point Create at it, and generate over it as chosen.
    pub fn regenerate(&mut self, ctx: &egui::Context, choice: Regenerate) {
        let source = PathBuf::from(self.settings.source_dir.trim());
        if choice.use_saved {
            match plan::regenerate_spec(&choice.dir, source, choice.mode) {
                Ok(spec) => {
                    self.settings.profile = spec.profile;
                    self.settings.options = spec.options;
                    self.settings.project_name = spec.name;
                    self.settings.destination = spec.destination.to_string_lossy().into_owned();
                }
                Err(error) => {
                    self.announce(LogKind::Error, error);
                    return;
                }
            }
        } else {
            self.settings.project_name = choice.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            self.settings.destination = choice.dir.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            self.settings.options.on_existing = choice.mode;
        }
        self.page = Page::Create;
        self.log(LogKind::Info, format!("Regenerating {} from the current source…", choice.dir.display()));
        self.plan_key = None;
        self.start_generation(ctx);
    }

    /// Check the Android tools on a worker thread (gradle and sdkmanager take seconds).
    pub fn check_android_tools(&mut self, ctx: &egui::Context) {
        if self.android_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let system = self.system.clone();
        let (rustup, rustc, cargo) = (self.rust.rustup_path.clone(), self.rust.rustc_path.clone(), self.rust.cargo_path.clone());
        let sdk = android::Sdk::detect();
        let (_, target) = android::resolve(self.settings.options.android_min_sdk, self.settings.options.android_target_sdk, &sdk);
        let repaint = ctx.clone();
        let spawned = std::thread::Builder::new().name("projectile-android-tools".into()).spawn(move || {
            let found = tools::detect(&system, rustup.as_deref(), rustc.as_deref(), cargo.as_deref(), target);
            let _ = tx.send(found);
            repaint.request_repaint();
        });
        match spawned {
            Ok(_) => self.android_rx = Some(rx),
            Err(error) => self.announce(LogKind::Warning, format!("Could not check the Android tools: {error}")),
        }
    }

    pub fn checking_android_tools(&self) -> bool {
        self.android_rx.is_some()
    }

    fn poll_android_tools(&mut self) {
        let Some(rx) = &self.android_rx else { return };
        match rx.try_recv() {
            Ok(found) => {
                self.android_tools = Some(found);
                self.android_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.android_rx = None,
        }
    }

    /// Run one of the Useful commands in the generated project (only commands marked runnable).
    pub fn run_useful(&mut self, command: &str) {
        let Some(project) = self.git_project() else {
            self.announce(LogKind::Warning, "Generate a project first; the command runs inside it.".into());
            return;
        };
        let mut words = command.split_whitespace();
        let Some(program) = words.next().and_then(toolchain::find_program) else {
            self.announce(LogKind::Warning, format!("`{command}` needs a program that was not found."));
            return;
        };
        self.task_confirm = Some(command::Task {
            title: format!("Run `{command}`"),
            explanation: format!("Runs in {}; the output appears in the Create Project log.", project.display()),
            program,
            args: words.map(str::to_owned).collect(),
            cwd: Some(project),
            log: command::Log::Main,
            then: command::Then::Nothing,
        });
    }

    /// The command currently running, if any.
    pub fn task_busy(&self) -> Option<&command::Task> {
        self.task_job.as_ref().map(|(task, _)| task)
    }

    /// Ask the user to confirm a rustup action (it becomes a task shown in full first).
    pub fn confirm_rust(&mut self, action: rust::Action) {
        match action.task(self.rust.rustup_path.as_deref()) {
            Ok(task) => self.task_confirm = Some(task),
            Err(error) => self.announce(LogKind::Warning, error),
        }
    }

    fn task_log(&mut self, target: command::Log, kind: LogKind, line: String) {
        match target {
            command::Log::Rust => {
                self.rust_log.push((kind, line));
                if self.rust_log.len() > MAX_LOG_LINES {
                    let excess = self.rust_log.len() - MAX_LOG_LINES;
                    self.rust_log.drain(..excess);
                }
            }
            command::Log::Main => self.log(kind, line),
        }
    }

    /// Run a confirmed command on a worker thread, streaming its output to its log.
    pub fn start_task(&mut self, ctx: &egui::Context, task: command::Task) {
        if self.task_job.is_some() {
            self.announce(LogKind::Warning, "Another command is still running; wait for it to finish.".into());
            return;
        }
        if self.is_generating() && task.cwd.is_some() {
            self.announce(LogKind::Warning, "A project is being generated; run this when it has finished.".into());
            return;
        }
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        let job = task.clone();
        let spawned = std::thread::Builder::new().name("projectile-task".into()).spawn(move || {
            let result = {
                let mut line = |text| {
                    let _ = tx.send(TaskMessage::Line(text));
                    repaint.request_repaint();
                };
                command::run(&job, &mut line)
            };
            let _ = tx.send(TaskMessage::Finished(result));
            repaint.request_repaint();
        });
        match spawned {
            Ok(_) => {
                self.task_log(task.log, LogKind::Info, format!("{}…", task.title));
                self.task_job = Some((task, rx));
            }
            Err(error) => self.announce(LogKind::Error, format!("Could not start {}: {error}", task.title)),
        }
    }

    fn poll_task_job(&mut self, ctx: &egui::Context) {
        let Some((task, rx)) = &self.task_job else { return };
        let task = task.clone();
        let mut finished = None;
        let mut lines = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(TaskMessage::Line(line)) => lines.push(line),
                Ok(TaskMessage::Finished(result)) => {
                    finished = Some(result);
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    finished = Some(Err(format!("{} stopped unexpectedly.", task.title)));
                    break;
                }
            }
        }
        for line in lines {
            self.task_log(task.log, LogKind::Info, line);
        }
        let Some(result) = finished else { return };
        self.task_job = None;
        match result {
            Ok(()) => {
                self.task_log(task.log, LogKind::Success, format!("{}: done.", task.title));
                self.announce(LogKind::Success, format!("{}: done.", task.title));
                if matches!(task.title.as_str(), "Clone PhotoCraft source" | "Update PhotoCraft source")
                    && let Some(folder) = self.source_after_task.take()
                {
                    self.set_source(ctx, &folder);
                }
            }
            Err(error) => {
                self.task_log(task.log, LogKind::Error, error.clone());
                self.announce(LogKind::Error, error);
            }
        }
        match task.log {
            command::Log::Rust => self.rescan_rust(),
            command::Log::Main => self.refresh_git(),
        }
    }

    pub fn is_generating(&self) -> bool {
        self.job.is_some()
    }

    pub fn job_progress(&self) -> Option<(u64, u64)> {
        self.job.as_ref().map(|job| (job.done, job.total))
    }

    /// Rebuild the plan when the spec changed (cheap: a few small file reads).
    fn refresh_plan(&mut self) {
        let key = (
            self.settings.source_dir.clone(),
            self.settings.project_name.clone(),
            self.settings.destination.clone(),
            self.settings.profile,
            self.settings.options.clone(),
        );
        if self.plan_key.as_ref() == Some(&key) {
            return;
        }
        self.plan = self.inventory.as_ref().ok().map(|inventory| plan::build(&self.spec(), inventory));
        self.plan_key = Some(key);
        self.plan_generation = self.plan_generation.wrapping_add(1);
        if let Some(plan) = &self.plan
            && self.review_index >= plan.edits.len()
        {
            self.review_index = 0;
        }
    }

    /// CMake's Configure: rescan the source and toolchain, then rebuild the plan.
    pub fn configure(&mut self) {
        self.rust = toolchain::detect();
        self.inventory = source::validate(&PathBuf::from(self.settings.source_dir.trim())).and_then(|root| plan::scan(&root));
        self.plan_key = None;
        self.refresh_plan();
        match &self.plan {
            Some(plan) if plan.blockers.is_empty() => {
                let (files, bytes) = plan.copied();
                let message =
                    format!("Configured: {} would be created ({files} files, {}). Press Generate.", plan.project_dir.display(), plan::human_bytes(bytes));
                self.announce(LogKind::Success, message);
            }
            Some(plan) => {
                let message = format!("Configure found {} problem(s): {}", plan.blockers.len(), plan.blockers.join(" "));
                self.announce(LogKind::Warning, message);
            }
            None => {
                let error = self.inventory.as_ref().err().cloned().unwrap_or_default();
                self.announce(LogKind::Error, format!("Cannot configure: {error}"));
            }
        }
    }

    pub fn can_generate(&self) -> Result<(), String> {
        if self.is_generating() {
            return Err("A project is being generated.".into());
        }
        // Generating over a folder while a build or git runs inside it would pull files out from
        // under that command and leave a half-replaced project.
        if let (Some(task), Some(dir)) = (self.task_busy(), self.spec().project_dir())
            && task.cwd.as_deref().is_some_and(|cwd| cwd.starts_with(&dir))
        {
            return Err(format!("“{}” is running in that folder; wait for it to finish before generating over it.", task.title));
        }
        match &self.plan {
            Some(plan) => plan.blockers.first().map_or(Ok(()), |b| Err(b.clone())),
            None => Err(self.inventory.as_ref().err().cloned().unwrap_or_else(|| "Configure first.".into())),
        }
    }

    pub fn start_generation(&mut self, ctx: &egui::Context) {
        self.refresh_plan();
        if let Err(reason) = self.can_generate() {
            self.announce(LogKind::Warning, format!("Cannot generate yet: {reason}"));
            return;
        }
        let spec = self.spec();
        if spec.options.on_existing == plan::OnExisting::Delete
            && let Some(dir) = spec.project_dir().filter(|dir| dir.exists())
            && !std::mem::take(&mut self.delete_confirmed)
        {
            let files = plan::count_files(&dir, 100_000);
            let made_by_projectile = dir.join(".projectile.json").is_file();
            self.delete_confirm = Some((dir, files, made_by_projectile));
            return;
        }
        let total = self.plan.as_ref().map_or(1, |plan| {
            let (files, _) = plan.copied();
            files.max(1)
        });
        let (tx, rx) = mpsc::channel();
        let repaint = ctx.clone();
        let spawned = std::thread::Builder::new().name("projectile-generate".into()).spawn(move || {
            let result = {
                let mut forward = |event| {
                    let _ = tx.send(JobMessage::Event(event));
                    repaint.request_repaint();
                };
                generator::generate(&spec, &mut forward)
            };
            let _ = tx.send(JobMessage::Finished(result));
            repaint.request_repaint();
        });
        match spawned {
            Ok(_) => {
                self.job = Some(Job { rx, done: 0, total });
                self.notice = Some((LogKind::Info, "Generating…".into()));
                self.log(LogKind::Info, format!("Generating {} project “{}”…", self.settings.profile.label(), self.settings.project_name.trim()));
            }
            Err(error) => self.announce(LogKind::Error, format!("Could not start generation: {error}")),
        }
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.job else { return };
        let mut events = Vec::new();
        let mut finished = None;
        loop {
            match job.rx.try_recv() {
                Ok(JobMessage::Event(generator::Event::Progress { done, total })) => {
                    job.done = done;
                    job.total = total.max(1);
                }
                Ok(JobMessage::Event(generator::Event::Log(line))) => events.push(line),
                Ok(JobMessage::Finished(result)) => {
                    finished = Some(result);
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    finished = Some(Err("generation stopped unexpectedly; the output folder may be incomplete.".into()));
                    break;
                }
            }
        }
        for line in events {
            let kind = if line.starts_with("Warning") { LogKind::Warning } else { LogKind::Info };
            self.log(kind, line);
        }
        let Some(result) = finished else { return };
        self.job = None;
        self.plan_key = None;
        match result {
            Ok(report) => {
                let warnings = if report.warnings.is_empty() { String::new() } else { format!(" ({} warning(s), see output)", report.warnings.len()) };
                if let Some(aside) = &report.replaced {
                    self.log(LogKind::Info, format!("The previous folder is kept at {}.", aside.display()));
                }
                self.announce(LogKind::Success, format!("Generated {} ({} files){warnings}.", report.path.display(), report.files));
                if self.settings.open_folder_after {
                    self.open_path(&report.path);
                }
                if self.settings.open_vscode_after {
                    self.open_in_vscode(&report.path);
                }
                let built = report.path.clone();
                self.last_generated = Some(report.path);
                self.refresh_git();
                if self.settings.build_after || self.settings.run_after {
                    match self.build_task(&built, self.settings.run_after) {
                        Ok(task) => self.start_task(ctx, task),
                        Err(error) => self.announce(LogKind::Warning, error),
                    }
                }
            }
            Err(error) => self.announce(LogKind::Error, format!("Generation failed: {error}")),
        }
    }

    pub fn open_path(&mut self, path: &std::path::Path) {
        if let Err(error) = open::that(path) {
            self.announce(LogKind::Warning, format!("Could not open {}: {error}", path.display()));
        }
    }

    /// Open a web link in the system browser. Only http(s) links are followed.
    pub fn open_url(&mut self, url: &str) {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            self.announce(LogKind::Warning, format!("Not opening `{url}`: only web links are opened."));
            return;
        }
        match open::that_detached(url) {
            Ok(()) => self.notice = Some((LogKind::Info, format!("Opened {url} in your browser."))),
            Err(error) => self.announce(LogKind::Warning, format!("Could not open {url}: {error}")),
        }
    }

    pub fn open_in_vscode(&mut self, path: &std::path::Path) {
        let Some(code) = self.rust.vscode_path.clone() else {
            self.announce(LogKind::Warning, "VS Code was not found on PATH (looked for code, codium and code-oss).".into());
            return;
        };
        match std::process::Command::new(&code).arg(path).spawn() {
            Ok(mut child) => {
                // Reap the launcher when it exits so it never lingers as a zombie.
                let _ = std::thread::Builder::new().name("projectile-vscode".into()).spawn(move || child.wait());
                self.log(LogKind::Success, format!("Opened {} in VS Code.", path.display()));
            }
            Err(error) => self.announce(LogKind::Warning, format!("Could not start {}: {error}", code.display())),
        }
    }

    pub fn resize_window(&mut self, ctx: &egui::Context, width: f32, height: f32) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        let size = fit_to_monitor(ctx, egui::vec2(width, height));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    }

    pub fn toggle_fullscreen(ctx: &egui::Context) {
        let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
    }

    /// Once per launch: give upgraders the portrait layout, and never open larger than the screen.
    fn check_window(&mut self, ctx: &egui::Context) {
        if self.window_checked {
            return;
        }
        let (inner, monitor) = ctx.input(|input| (input.viewport().inner_rect.map(|r| r.size()), input.viewport().monitor_size));
        let Some(monitor) = monitor else { return };
        self.window_checked = true;
        let mut wanted = inner.unwrap_or(egui::Vec2::from(DEFAULT_WINDOW));
        if self.settings.layout_version < LAYOUT_VERSION {
            wanted = egui::Vec2::from(DEFAULT_WINDOW);
            self.settings.layout_version = LAYOUT_VERSION;
        }
        let fitted = fit_size(wanted, monitor);
        if inner.is_none_or(|inner| (inner - fitted).length() > 1.0) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(fitted));
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let pressed = |shortcut: &KeyboardShortcut| ctx.input_mut(|input| input.consume_shortcut(shortcut));
        if pressed(&keys::GENERATE) {
            self.page = Page::Create;
            self.start_generation(ctx);
        }
        if pressed(&keys::CONFIGURE) {
            self.page = Page::Create;
            self.configure();
        }
        if pressed(&keys::NEW) {
            self.page = Page::Create;
        }
        if pressed(&keys::HELP) {
            self.page = Page::Help;
        }
        for (shortcut, page) in keys::PAGES.iter().zip(Page::ALL) {
            if pressed(shortcut) {
                self.page = page;
            }
        }
        if pressed(&keys::ZOOM_IN) || pressed(&keys::ZOOM_IN_PLUS) {
            self.set_scale(ctx, self.settings.scale_percent.saturating_add(10));
        }
        if pressed(&keys::ZOOM_OUT) {
            self.set_scale(ctx, self.settings.scale_percent.saturating_sub(10));
        }
        if pressed(&keys::ZOOM_RESET) {
            self.set_scale(ctx, 100);
        }
        if pressed(&keys::FULLSCREEN) {
            Self::toggle_fullscreen(ctx);
        }
        if pressed(&keys::ACCESS) {
            self.toggle_access = true;
        }
        if pressed(&keys::COPY_LOG) {
            ctx.copy_text(self.log_text());
            self.notice = Some((LogKind::Info, "Output copied to the clipboard.".into()));
        }
        if pressed(&keys::SCREENSHOT) {
            self.request_screenshot = true;
        }
        if pressed(&keys::QUIT) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Lines of a file picked in the tree, read from the source (or from the plan, for files
    /// ProjectIle writes). Capped so a huge or binary file can't stall the window.
    pub fn file_lines(&mut self, path: &str) -> FileLines {
        if let Some((cached, lines)) = &self.file_cache
            && cached == path
        {
            return lines.clone();
        }
        const MAX_BYTES: u64 = 512 * 1024;
        let generated = self.plan.as_ref().and_then(|plan| {
            plan.edits.iter().find(|e| e.path == path).and_then(|e| match &e.kind {
                plan::EditKind::Create(text) => Some(text.clone()),
                _ => None,
            })
        });
        let lines = match generated {
            Some(text) => Ok(text.lines().map(str::to_owned).collect::<Vec<_>>()),
            None => match &self.inventory {
                Ok(inventory) => {
                    let full = if path.is_empty() || path.split('/').any(|part| part == "..") { None } else { Some(inventory.root.join(path)) };
                    match full.map(|full| (std::fs::metadata(&full), full)) {
                        Some((Ok(meta), _)) if meta.is_dir() => Err("This is a folder; expand it in the tree.".to_owned()),
                        Some((Ok(meta), _)) if meta.len() > MAX_BYTES => {
                            Err(format!("{} is {}; too large to preview here.", path, plan::human_bytes(meta.len())))
                        }
                        Some((Ok(_), full)) => match std::fs::read(&full) {
                            Ok(bytes) if bytes.contains(&0) => Err(format!("{path} is a binary file ({}).", plan::human_bytes(bytes.len() as u64))),
                            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).lines().map(str::to_owned).collect()),
                            Err(error) => Err(format!("could not read {path}: {error}")),
                        },
                        Some((Err(error), _)) => Err(format!("could not read {path}: {error}")),
                        None => Err("Nothing to show.".to_owned()),
                    }
                }
                Err(error) => Err(error.clone()),
            },
        }
        .map(std::sync::Arc::new);
        self.file_cache = Some((path.to_owned(), lines.clone()));
        lines
    }

    /// Diff lines for one reviewed edit, cached until the plan changes.
    pub fn review_lines(&mut self, index: usize) -> ReviewLines {
        if let Some((cached_index, generation, lines)) = &self.review_cache
            && *cached_index == index
            && *generation == self.plan_generation
        {
            return lines.clone();
        }
        let root = self.inventory.as_ref().map(|inventory| inventory.root.clone()).map_err(Clone::clone);
        let lines = match (self.plan.as_ref().and_then(|plan| plan.edits.get(index)), root) {
            (Some(edit), Ok(root)) => plan::preview(edit, &root).map(std::sync::Arc::new),
            (None, _) => Err("Nothing to review.".into()),
            (_, Err(error)) => Err(error),
        };
        self.review_cache = Some((index, self.plan_generation, lines.clone()));
        lines
    }

    pub fn review_text(&mut self, index: usize) -> String {
        match self.review_lines(index) {
            Ok(lines) => lines
                .iter()
                .map(|line| match line.kind {
                    plan::LineKind::Removed => format!("-{}", line.text),
                    plan::LineKind::Added => format!("+{}", line.text),
                    plan::LineKind::Context => format!(" {}", line.text),
                    plan::LineKind::Header | plan::LineKind::Note => line.text.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(error) => error,
        }
    }

    pub fn log_text(&self) -> String {
        self.logs.iter().map(|(_, line)| line.as_str()).collect::<Vec<_>>().join("\n")
    }

    pub fn request_screenshot(&mut self) {
        self.request_screenshot = true;
    }

    pub fn summary_text(&self) -> String {
        let Some(plan) = &self.plan else { return String::new() };
        let mut text = String::new();
        for c in plan.capabilities.iter().chain(plan.summary().iter()) {
            text.push_str(&format!("{} {}\n", c.sign, c.text));
        }
        text
    }

    /// Underlined text tabs on the left; the language selector and the clock on the right.
    fn draw_tabs(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            for page in Page::ALL {
                let selected = self.page == page;
                let mut text = egui::RichText::new(page.label()).size(14.0).family(egui::FontFamily::Name("medium".into())).color(if selected {
                    p.text
                } else {
                    p.text_dim
                });
                if selected {
                    text = text.family(egui::FontFamily::Name("strong".into()));
                }
                let response = ui.add(egui::Button::new(text).frame(false).min_size(egui::vec2(0.0, 40.0)));
                let r = response.rect;
                if selected {
                    ui.painter()
                        .line_segment([egui::pos2(r.left(), r.bottom() - 1.0), egui::pos2(r.right(), r.bottom() - 1.0)], egui::Stroke::new(2.5, p.accent));
                } else if response.hovered() {
                    ui.painter()
                        .line_segment([egui::pos2(r.left(), r.bottom() - 1.0), egui::pos2(r.right(), r.bottom() - 1.0)], egui::Stroke::new(1.0, p.text_dim));
                }
                help::target(ui, r, page.help_topic());
                if response.clicked() {
                    self.page = page;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let now = chrono::Local::now();
                let clock =
                    pages::top_chip(ui, &p, &now.format("%H:%M").to_string(), false).on_hover_text(now.format("%A, %-d %B %Y · %H:%M:%S %Z").to_string());
                help::target(ui, clock.rect, "language");
                let seconds_left = 60 - u64::from(chrono::Timelike::second(&now)).min(59);
                ui.ctx().request_repaint_after(std::time::Duration::from_secs(seconds_left));
                let language = pages::top_chip(ui, &p, "English ⏷", true).on_hover_text("Interface language (English only for now)");
                help::target(ui, language.rect, "language");
                egui::Popup::menu(&language).show(|ui| {
                    ui.selectable_value(&mut self.settings.language, "en".to_owned(), "English");
                    ui.add_enabled(false, egui::Label::new(egui::RichText::new("More languages later").small()));
                });
            });
        });
    }

    /// Status bar: what ProjectIle is working with, the latest notice, and the ease-of-access tray.
    fn draw_footer(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let (dot, rust) = match self.rust_state.active_version() {
                Some(version) => (p.added, format!("Rust {version}")),
                None => (p.warning, "Rust not found".to_owned()),
            };
            let item = pages::status_item(ui, &p, dot, &rust).on_hover_text(format!("{}. Manage on the Rust tab.", self.rust_state.origin()));
            help::target(ui, item.rect, "rust");
            if item.clicked() {
                self.page = Page::Rust;
            }
            let source = match &self.inventory {
                Ok(inventory) => (p.added, source::photocraft_version(&inventory.root).map_or_else(|| "PhotoCraft".to_owned(), |v| format!("PhotoCraft {v}"))),
                Err(_) => (p.removed, "No source".to_owned()),
            };
            let item = pages::status_item(ui, &p, source.0, &source.1).on_hover_text("PhotoCraft source folder (Create Project › Source)");
            help::target(ui, item.rect, "source");
            if item.clicked() {
                self.page = Page::Create;
            }
            let profile = self.settings.profile;
            let item = pages::status_item(ui, &p, if profile.available() { p.accent } else { p.text_dim }, &format!("{} profile", profile.label()))
                .on_hover_text(profile.tagline());
            help::target(ui, item.rect, "profile");
            let item = pages::status_item(ui, &p, self.system.brand_color(&p), &self.system.summary())
                .on_hover_text("Detected system. Click for Preferences › System (layout and install hints).");
            help::target(ui, item.rect, "system");
            if item.clicked() {
                self.page = Page::Preferences;
            }
            let item = pages::status_item(ui, &p, p.added, "MCP ready")
                .on_hover_text("ProjectIle is an MCP server (`projectile --mcp`). Click to see how to use it with Claude Code.");
            help::target(ui, item.rect, "projectile-mcp");
            if item.clicked() {
                self.show_help("projectile-mcp");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let response = access::button(ui, &p, false, self.settings.access.active_count());
                help::target(ui, response.rect, "access");
                let toggle = response.clicked() || std::mem::take(&mut self.toggle_access);
                let mut changed = false;
                egui::Popup::from_response(&response)
                    .open_memory(toggle.then_some(egui::SetOpenCommand::Toggle))
                    .align(egui::RectAlign::TOP_END)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| changed = access::flyout(ui, &mut self.settings.access, &p));
                if changed {
                    self.apply_appearance(ui.ctx());
                }
                ui.label(egui::RichText::new(format!("ProjectIle {}", env!("CARGO_PKG_VERSION"))).small().color(p.text_dim))
                    .on_hover_text("Middle-click anything for help");
                if let Some((kind, message)) = &self.notice {
                    let color = pages::log_color(&p, *kind);
                    ui.add(egui::Label::new(egui::RichText::new(message).small().color(color)).truncate()).on_hover_text(message);
                }
            });
        });
    }

    fn save_screenshot(&mut self, image: &egui::ColorImage) {
        let result = (|| {
            let directory = screenshot_directory()?;
            std::fs::create_dir_all(&directory).map_err(|error| format!("Could not create screenshot folder {}: {error}", directory.display()))?;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("System clock is before the Unix epoch: {error}"))?
                .as_nanos();
            let path = directory.join(format!("projectile-{timestamp}.png"));
            encode_screenshot(&path, image)?;
            Ok::<_, String>(path)
        })();
        match result {
            Ok(path) => {
                let mut message = format!("Screenshot saved to {}", path.display());
                match set_screenshot_clipboard(image) {
                    Ok(()) => message.push_str(" and copied to the clipboard."),
                    Err(error) => {
                        self.log(LogKind::Warning, format!("Screenshot clipboard copy unavailable: {error}"));
                        message.push_str("; clipboard copy was unavailable.");
                    }
                }
                self.announce(LogKind::Success, message);
            }
            Err(error) => self.announce(LogKind::Error, error),
        }
    }
}

/// Links egui asked to open this frame. ProjectIle opens them itself (with the `open` crate),
/// because eframe's own link support needs a dependency ProjectIle does not build with.
fn take_open_urls(ctx: &egui::Context) -> Vec<String> {
    ctx.output_mut(|output| {
        let mut urls = Vec::new();
        output.commands.retain(|command| match command {
            egui::OutputCommand::OpenUrl(open) => {
                urls.push(open.url.clone());
                false
            }
            _ => true,
        });
        urls
    })
}

/// `size` shrunk (keeping its aspect) to fit 92% of the monitor.
fn fit_size(size: egui::Vec2, monitor: egui::Vec2) -> egui::Vec2 {
    if monitor.x <= 0.0 || monitor.y <= 0.0 || !monitor.is_finite() {
        return size;
    }
    let limit = monitor * 0.92;
    let factor = (limit.x / size.x).min(limit.y / size.y).min(1.0);
    let fitted = size * factor;
    egui::vec2(fitted.x.max(MIN_WINDOW[0].min(limit.x)), fitted.y.max(MIN_WINDOW[1].min(limit.y)))
}

fn fit_to_monitor(ctx: &egui::Context, size: egui::Vec2) -> egui::Vec2 {
    match ctx.input(|input| input.viewport().monitor_size) {
        Some(monitor) => fit_size(size, monitor),
        None => size,
    }
}

impl eframe::App for ProjectIle {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let screenshots = ui.input(|input| {
            input
                .raw
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        });
        for image in screenshots {
            self.save_screenshot(&image);
        }
        self.check_window(&ctx);
        self.handle_shortcuts(&ctx);
        self.poll_job(&ctx);
        self.poll_task_job(&ctx);
        self.poll_android_tools();
        self.refresh_plan();
        if self.is_generating() || self.task_job.is_some() || self.android_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let p = self.palette;
        let bar = egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin::symmetric(8, 3));
        egui::Panel::top("tabs")
            .frame(egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin { left: 16, right: 16, top: 4, bottom: 0 }))
            .show(ui, |ui| self.draw_tabs(ui));
        egui::Panel::bottom("footer").frame(bar.inner_margin(egui::Margin::symmetric(10, 4))).show(ui, |ui| self.draw_footer(ui));
        if self.page == Page::Create {
            // Never let the output pane take more than ~40% of the window from the form.
            let max_output = (ui.available_height() * 0.4).max(110.0);
            egui::Panel::bottom("output")
                .resizable(true)
                .default_size(150.0_f32.min(max_output))
                .size_range(110.0..=max_output)
                .frame(egui::Frame::new().fill(p.background).inner_margin(egui::Margin { left: 16, right: 16, top: 6, bottom: 8 }))
                .show(ui, |ui| pages::draw_output(self, ui));
        }
        egui::CentralPanel::no_frame().frame(egui::Frame::new().fill(p.background).inner_margin(egui::Margin::symmetric(16, 6))).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.page {
                Page::Welcome => pages::draw_welcome(self, ui),
                Page::Create => pages::draw_create(self, ui),
                Page::Rust => pages::draw_rust(self, ui),
                Page::Preferences => pages::draw_preferences(self, ui),
                Page::Help => pages::draw_help(self, ui),
                Page::About => pages::draw_about(self, ui),
            });
        });
        pages::draw_task_confirm(self, &ctx);
        pages::draw_delete_confirm(self, &ctx);
        pages::draw_regenerate(self, &ctx);
        if let Some(topic) = help::take_middle_click(&ctx) {
            self.show_help(topic);
        }
        for url in take_open_urls(&ctx) {
            self.open_url(&url);
        }
        if self.request_screenshot {
            self.request_screenshot = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.settings);
    }
}

#[cfg(test)]
mod tests {
    use super::{LegacySettings, Page, ProjectIle, SavedSettings, TIPS, default_destination, fit_size, next_tip_index};
    use eframe::egui;
    use std::path::PathBuf;

    #[test]
    fn launch_tip_advances_between_launches() {
        for previous in 0..TIPS.len() {
            let next = next_tip_index(previous);
            assert_ne!(previous, next);
            assert!(TIPS.get(next).is_some_and(|tip| !tip.is_empty()));
        }
    }

    #[test]
    fn default_settings_have_valid_startup_values() {
        let settings = SavedSettings::default();
        assert!(!settings.project_name.is_empty());
        assert!(super::plan::validate_options(&settings.options).is_ok());
        assert!(super::SCALE_RANGE.contains(&settings.scale_percent));
        assert_eq!(default_destination().to_string_lossy(), settings.destination);
    }

    #[test]
    fn settings_load_previous_generated_app_fields() -> Result<(), String> {
        let old = r#"{"project_name":"demo","destination":"/tmp","window_width":1600,"window_height":900,"ui_scale_percent":125,"theme":"Light"}"#;
        let settings: SavedSettings = serde_json::from_str::<LegacySettings>(old).map_err(|error| error.to_string())?.into();
        if settings.options.width != 1600 || settings.options.height != 900 || settings.options.scale_percent != 125 || settings.project_name != "demo" {
            return Err("legacy ProjectIle settings did not migrate their generated app values".into());
        }
        if settings.theme != super::AppTheme::StudioLight || settings.layout_version != 0 {
            return Err("legacy theme or layout version did not migrate".into());
        }
        Ok(())
    }

    #[test]
    fn hostile_settings_are_clamped() {
        let json = r#"{"scale_percent":100000,"options":{"width":5,"height":99999,"scale_percent":0,"mcp_port":1}}"#;
        let settings = serde_json::from_str::<SavedSettings>(json).map(SavedSettings::sanitized).unwrap_or_default();
        assert!(super::SCALE_RANGE.contains(&settings.scale_percent));
        assert!(super::plan::validate_options(&settings.options).is_ok());
    }

    #[test]
    fn window_fits_small_monitors_and_keeps_portrait() {
        let fitted = fit_size(egui::vec2(720.0, 960.0), egui::vec2(1366.0, 768.0));
        assert!(fitted.y <= 768.0 * 0.92 + 0.5 && fitted.y > fitted.x, "{fitted:?}");
        assert_eq!(fit_size(egui::vec2(720.0, 960.0), egui::vec2(3840.0, 2160.0)), egui::vec2(720.0, 960.0));
        assert_eq!(fit_size(egui::vec2(720.0, 960.0), egui::vec2(0.0, f32::NAN)), egui::vec2(720.0, 960.0));
    }

    fn render(harness: &mut egui_kittest::Harness<'_, ProjectIle>, env: &str, size: egui::Vec2) -> Result<(), String> {
        harness.run_steps(6);
        let image = harness.render().map_err(|error| error.to_string())?;
        if image.width() != size.x as u32 || image.height() != size.y as u32 {
            return Err(format!("unexpected snapshot dimensions: {}x{}", image.width(), image.height()));
        }
        if let Some(dir) = std::env::var_os("PROJECTILE_SNAPSHOT_DIR") {
            let path = PathBuf::from(dir).join(format!("{env}.png"));
            image.save(&path).map_err(|error| format!("could not save snapshot {}: {error}", path.display()))?;
        }
        Ok(())
    }

    #[test]
    fn renders_every_page_offscreen() -> Result<(), String> {
        let size = egui::vec2(720.0, 960.0);
        let mut harness = egui_kittest::Harness::builder().with_size(size).with_pixels_per_point(1.0).with_max_steps(64).wgpu().build_eframe(|cc| {
            let settings =
                SavedSettings { destination: std::env::temp_dir().join("projectile-ui-test").to_string_lossy().into_owned(), ..SavedSettings::default() };
            ProjectIle::with_settings(&cc.egui_ctx, settings)
        });
        render(&mut harness, "create", size)?;
        for page in [Page::Welcome, Page::Rust, Page::Preferences, Page::Help, Page::About] {
            harness.state_mut().page = page;
            render(&mut harness, &format!("{page:?}").to_lowercase(), size)?;
        }
        harness.state_mut().show_help("backup");
        render(&mut harness, "help-backup", size)?;
        // Viewer profile with Android on: the image row, Android levels and the Android tree group.
        harness.state_mut().page = Page::Create;
        harness.state_mut().settings.profile = super::Profile::Viewer;
        harness.state_mut().settings.options.android = true;
        render(&mut harness, "create-viewer", size)?;
        harness.state_mut().settings.profile = super::Profile::Base;
        harness.state_mut().settings.options.android = false;
        for key in ["form:source", "quick:backup"] {
            harness.state_mut().why_open.insert(key.into());
        }
        render(&mut harness, "create-why", size)?;
        // An existing project folder: the four choices, then the delete confirmation.
        let existing = std::env::temp_dir().join("projectile-ui-test").join("my-photocore");
        std::fs::create_dir_all(&existing).map_err(|error| error.to_string())?;
        harness.state_mut().settings.options.on_existing = super::plan::OnExisting::Delete;
        render(&mut harness, "create-existing", size)?;
        let ctx = harness.ctx.clone();
        harness.state_mut().start_generation(&ctx);
        if harness.state().delete_confirm.is_none() || harness.state().is_generating() {
            let _ = std::fs::remove_dir_all(&existing);
            return Err("Delete must ask for confirmation before anything runs".into());
        }
        render(&mut harness, "delete-confirm", size)?;
        harness.state_mut().delete_confirm = None;
        std::fs::write(
            existing.join(".projectile.json"),
            r#"{"profile":"Base","project_name":"my-photocore","source_revision":"commit 0123456789ab","options":{"mcp":true}}"#,
        )
        .map_err(|error| error.to_string())?;
        harness.state_mut().open_regenerate(existing.clone());
        if harness.state().regen.is_none() {
            let _ = std::fs::remove_dir_all(&existing);
            return Err("the Regenerate dialog did not open for a ProjectIle project".into());
        }
        render(&mut harness, "regenerate", size)?;
        harness.state_mut().regen = None;
        harness.state_mut().settings.options.on_existing = super::plan::OnExisting::Stop;
        let _ = std::fs::remove_dir_all(&existing);
        harness.state_mut().why_open.clear();
        harness.state_mut().settings.options.mcp = true;
        harness.state_mut().settings.profile = super::Profile::Minimal;
        render(&mut harness, "create-minimal", size)?;
        harness.state_mut().settings.profile = super::Profile::Base;
        harness.state_mut().toggle_access = true;
        render(&mut harness, "access", size)?;
        let state = harness.state_mut();
        state.settings.access.high_contrast = true;
        state.settings.access.large_text = true;
        state.settings.access.explain = true;
        let ctx = harness.ctx.clone();
        harness.state_mut().apply_appearance(&ctx);
        render(&mut harness, "create-high-contrast", size)?;
        // A folder that is not PhotoCraft: a clear error, Generate disabled, no panic.
        let not_photocraft = std::env::temp_dir();
        harness.state_mut().set_source(&ctx, &not_photocraft);
        if harness.state().inventory.is_ok() || harness.state().can_generate().is_ok() {
            return Err("an invalid source folder was accepted".into());
        }
        let state = harness.state_mut();
        state.settings.access = super::Access::default();
        harness.state_mut().apply_appearance(&ctx);
        render(&mut harness, "create-no-source", size)?;
        Ok(())
    }

    #[test]
    fn renders_full_create_page() -> Result<(), String> {
        let size = egui::vec2(720.0, 2600.0);
        let mut harness = egui_kittest::Harness::builder().with_size(size).with_pixels_per_point(1.0).with_max_steps(64).wgpu().build_eframe(|cc| {
            let settings =
                SavedSettings { destination: std::env::temp_dir().join("projectile-ui-test").to_string_lossy().into_owned(), ..SavedSettings::default() };
            ProjectIle::with_settings(&cc.egui_ctx, settings)
        });
        harness.state_mut().review_index = 2;
        let existing = std::env::temp_dir().join("projectile-ui-full").join("my-photocore");
        std::fs::create_dir_all(&existing).map_err(|error| error.to_string())?;
        harness.state_mut().settings.destination = existing.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        harness.state_mut().settings.options.on_existing = super::plan::OnExisting::Merge;
        let result = render(&mut harness, "create-full", size);
        let _ = std::fs::remove_dir_all(&existing);
        result
    }

    #[test]
    fn renders_at_minimum_window_size() -> Result<(), String> {
        let size = egui::vec2(super::MIN_WINDOW[0], super::MIN_WINDOW[1]);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(size)
            .with_pixels_per_point(1.0)
            .with_max_steps(64)
            .wgpu()
            .build_eframe(|cc| ProjectIle::with_settings(&cc.egui_ctx, SavedSettings::default()));
        render(&mut harness, "minimum", size)
    }

    #[test]
    fn link_clicks_are_collected_for_the_system_browser() {
        let ctx = egui::Context::default();
        let mut urls = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.ctx().open_url(egui::OpenUrl::new_tab("https://en.wikipedia.org/wiki/Rust_(programming_language)"));
            urls = super::take_open_urls(ui.ctx());
        });
        output.textures_delta.clear();
        assert_eq!(urls, ["https://en.wikipedia.org/wiki/Rust_(programming_language)"]);
        assert!(!output.platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::OpenUrl(_))));
    }

    #[test]
    fn screenshot_encoder_writes_png_data() -> Result<(), String> {
        let directory = std::env::temp_dir().join(format!("projectile-screenshot-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let path = directory.join("capture.png");
        let image = egui::ColorImage::new([2, 1], vec![egui::Color32::RED, egui::Color32::BLUE]);
        let encoded = super::encode_screenshot(&path, &image);
        let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
        let _ = std::fs::remove_dir_all(&directory);
        encoded?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err("Screenshot output is not a PNG file.".into());
        }
        Ok(())
    }
}
