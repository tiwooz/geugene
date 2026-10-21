//! The tabs (Welcome, Create Project, Rust, Preferences, About), the output pane and the rustup
//! confirmation dialog. Ease of access lives only in the status bar's tray icon (`access.rs`).

use crate::plan::{self, Change, LineKind, Profile};
use crate::rust::{self, Action};
use crate::theme::{AppTheme, Palette};
use crate::{LogKind, Page, ProjectIle, RustView, WINDOW_PRESETS, help, keys, tree};
use eframe::egui::{self, Color32, RichText};
use std::path::PathBuf;

pub const REPOSITORY: &str = "https://github.com/tiwooz/photocore";
/// The official ArtCraft community, as linked from PhotoCraft's README.
pub const DISCORD: &str = "https://discord.gg/artcraft";
/// Anthropic's Claude Console (console.anthropic.com now redirects here).
pub const CLAUDE_CONSOLE: &str = "https://platform.claude.com/";
pub const LINKS: &[(&str, &str)] = &[
    ("ProjectIle and PhotoCore on GitHub", REPOSITORY),
    ("Report an issue", "https://github.com/tiwooz/photocore/issues"),
    ("ArtCraft Discord (community)", DISCORD),
    ("Claude Console", CLAUDE_CONSOLE),
    ("PhotoCraft on GitHub", "https://github.com/storytold/photocraft"),
    ("The Rust Programming Language (book)", "https://doc.rust-lang.org/book/"),
    ("Install Rust (rustup)", "https://rustup.rs/"),
    ("Claude Code: MCP servers", "https://docs.anthropic.com/en/docs/claude-code/mcp"),
];
/// Libraries ProjectIle is built on, with the licences their manifests declare.
const BUILT_WITH: &[(&str, &str, &str)] = &[
    ("egui / eframe", "immediate-mode UI and native window", "MIT OR Apache-2.0"),
    ("wgpu", "GPU rendering", "MIT OR Apache-2.0"),
    ("AccessKit", "screen reader support", "MIT OR Apache-2.0"),
    ("rfd", "native folder pickers", "MIT"),
    ("arboard", "clipboard", "MIT OR Apache-2.0"),
    ("serde / serde_json", "settings, metadata, MCP messages", "MIT OR Apache-2.0"),
    ("png", "screenshots", "MIT OR Apache-2.0"),
    ("open", "opening folders and links", "MIT"),
];

pub fn log_color(p: &Palette, kind: LogKind) -> Color32 {
    match kind {
        LogKind::Info => p.text_dim,
        LogKind::Success => p.added,
        LogKind::Warning => p.warning,
        LogKind::Error => p.removed,
    }
}

/// A clickable status-bar item: a coloured dot and a label.
pub fn status_item(ui: &mut egui::Ui, p: &Palette, dot: Color32, label: &str) -> egui::Response {
    let response = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 3.5, dot);
            ui.label(RichText::new(label).small().color(p.text));
        })
        .response
        .interact(egui::Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// A chip for the top-right corner. Language and clock share it so they line up exactly.
pub fn top_chip(ui: &mut egui::Ui, p: &Palette, text: &str, clickable: bool) -> egui::Response {
    let font = egui::FontId::new(13.0, egui::FontFamily::Name("strong".into()));
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, p.text);
    let size = egui::vec2(galley.size().x + 22.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, if clickable { egui::Sense::click() } else { egui::Sense::hover() });
    let fill = if clickable && response.hovered() { p.hover } else { p.field };
    ui.painter().rect(rect, f32::from(p.radius), fill, egui::Stroke::new(1.0, p.field_border), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, p.text);
    response
}

/// A small rounded badge.
fn pill(ui: &mut egui::Ui, p: &Palette, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.6)))
        .corner_radius(9.0)
        .inner_margin(egui::Margin::symmetric(7, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(if p.dark { color } else { color.gamma_multiply(0.85) }));
        });
}

/// A card with a title and an optional one-line subtitle: the building block of every page.
pub fn section<R>(ui: &mut egui::Ui, p: &Palette, title: &str, subtitle: &str, topic: &'static str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.add_space(8.0);
    let inner = egui::Frame::new()
        .fill(p.card)
        .stroke(egui::Stroke::new(1.0, p.card_border))
        .corner_radius(f32::from(p.radius) + 4.0)
        .inner_margin(egui::Margin::symmetric(18, 14))
        .shadow(egui::Shadow { offset: [0, 1], blur: 6, spread: 0, color: Color32::from_black_alpha(if p.dark { 60 } else { 18 }) })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let heading = ui.label(RichText::new(title).size(15.5).family(egui::FontFamily::Name("strong".into())).color(p.text));
            let r = heading.rect;
            ui.painter()
                .line_segment([egui::pos2(r.left() - 10.0, r.top() + 2.0), egui::pos2(r.left() - 10.0, r.bottom() - 2.0)], egui::Stroke::new(3.0, p.accent));
            if !subtitle.is_empty() {
                plain(ui, RichText::new(subtitle).small().color(p.text_dim));
            }
            ui.add_space(6.0);
            add(ui)
        });
    help::target(ui, inner.response.rect, topic);
    ui.add_space(4.0);
    inner.inner
}

fn well<R>(ui: &mut egui::Ui, p: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(p.field)
        .stroke(egui::Stroke::new(1.0, p.card_border))
        .corner_radius(f32::from(p.radius))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

const MIN_LIST: f32 = 90.0;
const MAX_LIST: f32 = 1600.0;

/// The height of a resizable list: what the user dragged it to, or `default`.
fn list_height(app: &ProjectIle, id: &str, default: f32) -> f32 {
    app.settings.list_heights.get(id).copied().unwrap_or(default).clamp(MIN_LIST, MAX_LIST)
}

/// A grip under a list: drag to make the list taller or shorter, double-click to reset.
fn resize_handle(app: &mut ProjectIle, ui: &mut egui::Ui, id: &str, default: f32) {
    let p = app.palette;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 12.0), egui::Sense::click_and_drag());
    let active = response.hovered() || response.dragged();
    let color = if active { p.accent } else { p.field_border };
    let c = rect.center();
    for dy in [-2.0, 2.0] {
        ui.painter().line_segment([egui::pos2(c.x - 18.0, c.y + dy), egui::pos2(c.x + 18.0, c.y + dy)], egui::Stroke::new(1.5, color));
    }
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if response.dragged() {
        let height = (list_height(app, id, default) + response.drag_delta().y).clamp(MIN_LIST, MAX_LIST);
        app.settings.list_heights.insert(id.to_owned(), height);
    }
    if response.double_clicked() {
        app.settings.list_heights.remove(id);
    }
    response.on_hover_text("Drag to resize · double-click to reset");
}

fn explain(ui: &mut egui::Ui, app: &ProjectIle, text: &str) {
    if app.settings.access.explain {
        plain(ui, RichText::new(text).small().color(app.palette.text_dim));
    }
}

/// Wrapped, left-aligned text. Justified parent layouts (columns, grids) would otherwise stretch
/// the word spacing of every wrapped line.
fn plain(ui: &mut egui::Ui, text: RichText) {
    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.add(egui::Label::new(text).wrap());
    });
}

/// One `label | widget` form row in a two-column grid.
fn row(ui: &mut egui::Ui, p: &Palette, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    let topic = match label {
        "Source" => "source",
        "Name" => "name",
        "Location" | "Creates" => "location",
        "Profile" => "profile",
        "When done" => "after",
        "Status" => "status",
        "Image" => "viewer",
        "Theme" | "Scale" => "preferences",
        "System" | "Desktop" | "Packages" | "Layout" => "system",
        _ => "start",
    };
    let left = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(RichText::new(label).color(p.text_dim));
    });
    let right = ui.horizontal(add);
    help::target(ui, left.response.rect.union(right.response.rect), topic);
    ui.end_row();
}

/// A small "Why?" link that unfolds `topic`'s short answer (see [`why_body`]).
fn why_link(app: &mut ProjectIle, ui: &mut egui::Ui, key: &str) {
    let open = app.why_open.contains(key);
    let text = RichText::new(if open { "Hide" } else { "Why?" }).small().color(app.palette.accent);
    if ui.add(egui::Label::new(text).sense(egui::Sense::click()).selectable(false)).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        if open {
            app.why_open.remove(key);
        } else {
            app.why_open.insert(key.to_owned());
        }
    }
}

/// The unfolded explanation for a "Why?" link, with a way into the Help tab.
fn why_body(app: &mut ProjectIle, ui: &mut egui::Ui, key: &str, topic: &'static str) {
    if !app.why_open.contains(key) {
        return;
    }
    let p = app.palette;
    egui::Frame::new()
        .fill(p.accent.gamma_multiply(0.08))
        .stroke(egui::Stroke::new(1.0, p.accent.gamma_multiply(0.35)))
        .corner_radius(f32::from(p.radius))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            plain(ui, RichText::new(help::short(topic)).small());
            ui.horizontal_wrapped(|ui| {
                if ui.add(egui::Label::new(RichText::new("More in Help →").small().color(p.accent)).sense(egui::Sense::click()).selectable(false)).clicked() {
                    app.show_help(topic);
                }
                for (label, url) in help::find(topic).map_or(&[][..], |t| t.links).iter().take(2) {
                    ui.hyperlink_to(RichText::new(*label).small(), *url);
                }
            });
        });
}

/// A bullet or numbered line with a trailing "Why?" link and its explanation.
fn bullet(app: &mut ProjectIle, ui: &mut egui::Ui, marker: RichText, text: &str, key: &str, topic: &'static str) {
    let response = ui.horizontal_top(|ui| {
        ui.label(marker);
        let width = (ui.available_width() - 44.0).max(80.0);
        ui.allocate_ui(egui::vec2(width, 0.0), |ui| plain(ui, RichText::new(text)));
        why_link(app, ui, key);
    });
    help::target(ui, response.response.rect, topic);
    why_body(app, ui, key, topic);
}

/// A grid row under a form field: a short hint, a "Why?" link, and its explanation when open.
fn hint_row(app: &mut ProjectIle, ui: &mut egui::Ui, text: RichText, key: &str, topic: &'static str) {
    ui.label("");
    ui.vertical(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(text);
            why_link(app, ui, key);
        });
        why_body(app, ui, key, topic);
    });
    ui.end_row();
}

fn change_line(ui: &mut egui::Ui, p: &Palette, change: &Change) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(change.sign.to_string()).monospace().strong().color(tree::sign_color(p, change.sign)));
        ui.add(egui::Label::new(RichText::new(&change.text).color(if change.sign == '·' { p.text_dim } else { p.text })).wrap());
    });
}

fn folder_picker(ui: &mut egui::Ui, current: &str) -> Option<PathBuf> {
    let clicked = ui.add(egui::Button::new("Browse…")).clicked();
    if !clicked {
        return None;
    }
    let current = PathBuf::from(current.trim());
    let dialog = if current.is_dir() { rfd::FileDialog::new().set_directory(current) } else { rfd::FileDialog::new() };
    dialog.pick_folder()
}

// ---------------------------------------------------------------------------------------------
// Create Project
// ---------------------------------------------------------------------------------------------

pub fn draw_create(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    section(ui, &p, "Project", "What to generate, from which PhotoCraft source, and where.", "start", |ui| {
        egui::Grid::new("project-form").num_columns(2).spacing([12.0, 8.0]).min_col_width(80.0).show(ui, |ui| {
            row(ui, &p, "Source", |ui| {
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut app.settings.source_dir)
                        .hint_text("root of a PhotoCraft checkout")
                        .font(egui::TextStyle::Monospace)
                        .desired_width((ui.available_width() - 80.0).max(80.0)),
                );
                if edit.lost_focus() {
                    let folder = PathBuf::from(app.settings.source_dir.trim());
                    let current = app.inventory.as_ref().ok().map(|inventory| inventory.root.clone());
                    if current.as_deref() != Some(folder.as_path()) {
                        app.set_source(ui.ctx(), &folder);
                    }
                }
                if let Some(path) = folder_picker(ui, &app.settings.source_dir) {
                    app.set_source(ui.ctx(), &path);
                }
            });
            let source_note = match &app.inventory {
                Ok(inventory) => {
                    let version = crate::source::photocraft_version(&inventory.root).map_or_else(|| "PhotoCraft".to_owned(), |v| format!("PhotoCraft {v}"));
                    let revision = inventory.revision.as_deref().unwrap_or("revision unknown");
                    RichText::new(format!("✔ {version} · {revision}")).small().color(p.added)
                }
                Err(error) => RichText::new(format!("✖ {error}")).small().color(p.removed),
            };
            hint_row(app, ui, source_note, "form:source", "source");
            row(ui, &p, "Source version", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.settings.source_version)
                        .hint_text("main, a release tag, or a commit")
                        .font(egui::TextStyle::Monospace)
                        .desired_width((ui.available_width() - 150.0).max(120.0)),
                );
                let versions = app
                    .inventory
                    .as_ref()
                    .ok()
                    .map(|inventory| crate::source::local_versions(&inventory.root))
                    .unwrap_or_default();
                if !versions.is_empty() {
                    egui::ComboBox::from_id_salt("source-version")
                        .selected_text("local releases")
                        .width(105.0)
                        .show_ui(ui, |ui| {
                            for version in versions {
                                ui.selectable_value(&mut app.settings.source_version, version.clone(), version);
                            }
                        });
                }
                if ui.button("Clone / Update").on_hover_text("Clone the upstream source here when it is absent, or fetch tags and select this version when it is present.").clicked() {
                    app.confirm_source_sync();
                }
            });
            let version_note = match crate::source::validate_ref(&app.settings.source_version) {
                Ok(version) => RichText::new(format!("Source ref: {version}. Clone / Update downloads from the official repository only after its task confirmation.")).small().color(p.text_dim),
                Err(error) => RichText::new(error).small().color(p.warning),
            };
            hint_row(app, ui, version_note, "form:source-version", "source");
            row(ui, &p, "Name", |ui| {
                ui.add(egui::TextEdit::singleline(&mut app.settings.project_name).hint_text("my-photocore").desired_width(f32::INFINITY));
            });
            hint_row(app, ui, RichText::new("A letter first, then letters, digits, - or _").small().color(p.text_dim), "form:name", "naming");
            row(ui, &p, "Location", |ui| {
                ui.add(egui::TextEdit::singleline(&mut app.settings.destination).desired_width((ui.available_width() - 80.0).max(80.0)));
                if let Some(path) = folder_picker(ui, &app.settings.destination) {
                    app.settings.destination = path.to_string_lossy().into_owned();
                }
            });
            hint_row(app, ui, RichText::new("The project gets its own new folder inside").small().color(p.text_dim), "form:location", "naming");
            row(ui, &p, "Profile", |ui| {
                let current = app.settings.profile;
                let mut viewer_selected = false;
                egui::ComboBox::from_id_salt("profile").selected_text(current.label()).width(150.0).show_ui(ui, |ui| {
                    for profile in Profile::ALL {
                        let label = if profile.available() { profile.label().to_owned() } else { format!("{} (preview)", profile.label()) };
                        if ui.selectable_value(&mut app.settings.profile, profile, label).changed() && profile == Profile::Viewer {
                            viewer_selected = true;
                        }
                    }
                });
                if viewer_selected || app.settings.profile == Profile::Viewer {
                    app.settings.options.backup_source = true;
                }
                if current.available() {
                    pill(ui, &p, "ready", p.added);
                } else {
                    pill(ui, &p, "preview only", p.warning);
                }
            });
            let tagline = RichText::new(app.settings.profile.tagline()).small().color(p.text_dim);
            hint_row(app, ui, tagline, "form:profile", "profile");
            if app.settings.profile == Profile::Viewer {
                row(ui, &p, "Image", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut app.settings.options.viewer_image)
                            .hint_text("the picture the viewer opens")
                            .desired_width((ui.available_width() - 80.0).max(80.0)),
                    );
                    if ui.button("Browse…").clicked() {
                        let current = PathBuf::from(app.settings.options.viewer_image.trim());
                        let mut dialog = rfd::FileDialog::new().add_filter("Images", plan::VIEWER_EXTENSIONS);
                        if let Some(dir) = current.parent().filter(|d| d.is_dir()) {
                            dialog = dialog.set_directory(dir);
                        }
                        if let Some(path) = dialog.pick_file() {
                            app.settings.options.viewer_image = path.to_string_lossy().into_owned();
                        }
                    }
                });
                let note = match plan::check_viewer_image(&app.settings.options.viewer_image) {
                    Ok(_) => RichText::new(format!("✔ Copied into the project as {}/startup.*", plan::VIEWER_IMAGE_DIR)).small().color(p.added),
                    Err(error) => RichText::new(format!("✖ {error}")).small().color(p.warning),
                };
                hint_row(app, ui, note, "form:viewer", "viewer");
            }
            row(ui, &p, "Creates", |ui| {
                let preview = app.spec().project_dir();
                if let Some(path) = &preview
                    && ui.small_button("Copy").on_hover_text("Copy the project path").clicked()
                {
                    ui.ctx().copy_text(path.display().to_string());
                }
                let text = preview.map_or_else(|| "check the name and location".to_owned(), |path| path.display().to_string());
                ui.add(egui::Label::new(RichText::new(text).monospace().color(p.accent)).truncate());
            });
            if app.spec().project_dir().is_some_and(|dir| dir.exists()) {
                ui.label("");
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("⚠ This folder already exists. What should happen to it?").small().color(p.warning));
                        why_link(app, ui, "form:replace");
                    });
                    if let Some(dir) = app.spec().project_dir().filter(|dir| dir.join(".projectile.json").is_file()) {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("It was made by ProjectIle.").small().color(p.text_dim));
                            if ui.small_button("Regenerate it with its saved settings…").clicked() {
                                app.open_regenerate(dir);
                            }
                        });
                    }
                    why_body(app, ui, "form:replace", "replace");
                    for mode in plan::OnExisting::ALL {
                        let selected = app.settings.options.on_existing == mode;
                        let color = if mode == plan::OnExisting::Delete { p.removed } else { p.text };
                        if ui.radio(selected, RichText::new(mode.label()).color(color)).on_hover_text(mode.detail()).clicked() {
                            app.settings.options.on_existing = mode;
                        }
                        if selected {
                            ui.indent(mode.label(), |ui| plain(ui, RichText::new(mode.detail()).small().color(p.text_dim)));
                        }
                    }
                });
                ui.end_row();
            }
            row(ui, &p, "When done", |ui| {
                ui.checkbox(&mut app.settings.open_folder_after, "Open folder");
                let has_code = app.rust.vscode_path.is_some();
                ui.add_enabled_ui(has_code, |ui| {
                    ui.checkbox(&mut app.settings.open_vscode_after, "Open in VS Code")
                        .on_disabled_hover_text("VS Code was not found (code, codium or code-oss).");
                });
                ui.checkbox(&mut app.settings.build_after, "Build").on_hover_text("Run `cargo build -p photocraft` in the new project");
                if ui.checkbox(&mut app.settings.run_after, "Run").on_hover_text("Build, then start the editor").changed() && app.settings.run_after {
                    app.settings.build_after = true;
                }
            });
            let (status, color) = match (app.job_progress(), app.can_generate()) {
                (Some((done, total)), _) => (format!("Generating… {}%", done.saturating_mul(100) / total.max(1)), p.accent),
                (None, Ok(())) => (format!("✔ Ready to generate. Press Generate ({}).", ui.ctx().format_shortcut(&keys::GENERATE)), p.added),
                (None, Err(reason)) => (format!("✖ {reason}"), p.warning),
            };
            row(ui, &p, "Status", |ui| {
                ui.add(egui::Label::new(RichText::new(status).strong().color(color)).wrap());
            });
            hint_row(app, ui, RichText::new("Checked again whenever you change something").small().color(p.text_dim), "form:status", "status");
        });
    });

    draw_summary(app, ui);
    draw_components(app, ui);
    draw_review(app, ui);
    draw_git(app, ui);
}

fn draw_summary(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let Some(plan) = app.plan.clone() else { return };
    let subtitle = format!("{} profile compared with full PhotoCraft, then what goes into the project.", app.settings.profile.label());
    section(ui, &p, "Summary", &subtitle, "summary", |ui| {
        for blocker in &plan.blockers {
            change_line(ui, &p, &Change { sign: '✖', text: blocker.clone() });
        }
        for warning in &plan.warnings {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new("⚠").color(p.warning));
                ui.add(egui::Label::new(RichText::new(warning).color(p.warning)).wrap());
            });
        }
        well(ui, &p, |ui| {
            for change in &plan.capabilities {
                change_line(ui, &p, change);
            }
        });
        ui.add_space(6.0);
        let (files, bytes) = plan.copied();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Into {}: {files} files, {}", plan.package, plan::human_bytes(bytes))).color(p.text_dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Copy summary").clicked() {
                    ui.ctx().copy_text(app.summary_text());
                }
            });
        });
        well(ui, &p, |ui| {
            for change in plan.summary() {
                change_line(ui, &p, &change);
            }
        });
    });
}

fn draw_components(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let Some(plan) = app.plan.clone() else { return };
    let backup_required = app.settings.profile == Profile::Viewer;
    if backup_required {
        app.settings.options.backup_source = true;
    }
    section(ui, &p, "Components", "Tick what goes into the project. Expand a component to browse its files; click a file to review it.", "components", |ui| {
        let enabled = app.settings.profile.available() && app.job_progress().is_none();
        for (key, topic, title, detail) in [
            (
                "quick:backup",
                "backup",
                "Original PhotoCraft source backup",
                if backup_required {
                    "Viewer projects always include an untouched source copy in backup/: a fallback for you and a reference agents can read without finding the repository."
                } else {
                    "An untouched copy of the whole source in backup/: a fallback for you, and a reference agents can read without finding the repository."
                },
            ),
            (
                "quick:projectile-snapshot",
                "projectile-snapshot",
                "ProjectIle source snapshot",
                "Adds the archived source of this ProjectIle release to the project, so you can recover or inspect the generator that made it.",
            ),
            (
                "quick:mcp",
                "mcp",
                "Agent tools for Claude Code (MCP)",
                "A .mcp.json that lets Claude Code drive the running editor, with the same Base limits as you.",
            ),
            (
                "quick:android",
                "android",
                "Android build (Gradle, Java)",
                "Adds an android/ Gradle project and a Rust entry crate so the editor can be packaged as an APK. Experimental.",
            ),
        ] {
            let frame = egui::Frame::new()
                .fill(p.accent.gamma_multiply(0.07))
                .stroke(egui::Stroke::new(1.0, p.accent.gamma_multiply(0.4)))
                .corner_radius(f32::from(p.radius))
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_top(|ui| {
                        let value = match topic {
                            "backup" => &mut app.settings.options.backup_source,
                            "projectile-snapshot" => &mut app.settings.options.backup_projectile,
                            "android" => &mut app.settings.options.android,
                            _ => &mut app.settings.options.mcp,
                        };
                        ui.add_enabled(enabled && !(topic == "backup" && backup_required), egui::Checkbox::without_text(value));
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(title).strong());
                                if topic == "backup" {
                                    pill(ui, &p, "great for agents", p.accent);
                                }
                                if topic == "projectile-snapshot" {
                                    pill(ui, &p, "recovery", p.accent);
                                }
                                if topic == "android" {
                                    pill(ui, &p, "experimental", p.warning);
                                }
                                why_link(app, ui, key);
                            });
                            plain(ui, RichText::new(detail).small().color(p.text_dim));
                        });
                    });
                    if topic == "android" && app.settings.options.android {
                        android_levels(app, ui);
                    }
                    why_body(app, ui, key, topic);
                });
            help::target(ui, frame.response.rect, topic);
            ui.add_space(2.0);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut app.component_filter).hint_text("Search components…").desired_width(220.0));
            if ui.small_button("Expand all").clicked() {
                for id in ["g:app", "g:crates", "g:mcp", "g:docs", "g:backup", "g:projectile-snapshot", "g:vscode", "g:git", "g:pin", "g:workspace", "g:left-out"] {
                    app.tree_expanded.insert(id.into());
                }
                for layer in [
                    "L6 interface",
                    "L5 engine",
                    "L4 files",
                    "L3 render",
                    "L2 editing",
                    "L1 document",
                    "L0 input",
                    "L0 formats",
                    "L0 foundation",
                    "crate",
                    "tests",
                ] {
                    app.tree_expanded.insert(format!("g:layer:{layer}"));
                }
            }
            if ui.small_button("Collapse all").clicked() {
                app.tree_expanded.clear();
            }
        });
        let mut nodes = tree::build(&plan, &app.settings.options, app.settings.profile.available());
        let submenu_paths = match app.inventory.as_ref() {
            Ok(inventory) => plan::submenu_paths(&inventory.root),
            Err(_) => Err("Select a PhotoCraft source to list its submenus.".to_owned()),
        };
        nodes.push(tree::menu_node(&app.settings.options, submenu_paths));
        let edited: std::collections::HashSet<String> = plan.edits.iter().map(|e| e.path.clone()).collect();
        let root = app.inventory.as_ref().ok().map(|inventory| inventory.root.clone());
        let filter = app.component_filter.trim().to_lowercase();
        let toggles_enabled = app.settings.profile.available() && app.job_progress().is_none();
        let height = list_height(app, "components", 460.0);
        well(ui, &p, |ui| {
            egui::ScrollArea::vertical().id_salt("components").min_scrolled_height(height).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                let mut state = tree::TreeState {
                    options: &mut app.settings.options,
                    expanded: &mut app.tree_expanded,
                    listings: &mut app.listings,
                    selected: &mut app.review_file,
                    root: root.as_deref(),
                    edited: &edited,
                    filter,
                    toggles_enabled,
                };
                tree::show(ui, &p, &nodes, &mut state);
            });
        });
        resize_handle(app, ui, "components", 460.0);
        ui.horizontal_wrapped(|ui| {
            for (sign, label) in [('+', "enacted / added"), ('~', "edited by ProjectIle"), ('·', "compiled, dormant"), ('-', "won't be used")] {
                ui.label(RichText::new(format!("{sign} {label}")).small().color(tree::sign_color(&p, sign)));
            }
            if app.settings.options.mcp {
                ui.separator();
                ui.label(RichText::new("MCP port").small());
                ui.add(egui::DragValue::new(&mut app.settings.options.mcp_port).range(1024..=65535));
            }
        });
    });
}

fn draw_review(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let Some(plan) = app.plan.clone() else { return };
    section(ui, &p, "Source review", "ProjectIle's edits as a diff, or any file you pick in Components.", "review", |ui| {
        let mut index = app.review_index.min(plan.edits.len().saturating_sub(1));
        let picked = app.review_file.clone();
        let picked_edit = picked.as_ref().and_then(|path| plan.edits.iter().position(|e| &e.path == path));
        ui.horizontal(|ui| {
            let selected = match (&picked, picked_edit) {
                (Some(path), None) => path.clone(),
                _ => plan.edits.get(picked_edit.unwrap_or(index)).map_or_else(|| "no edits in this profile".to_owned(), |e| format!("~ {}", e.path)),
            };
            egui::ComboBox::from_id_salt("review-file")
                .selected_text(RichText::new(selected).monospace())
                .width((ui.available_width() - 90.0).max(120.0))
                .show_ui(ui, |ui| {
                    for (i, edit) in plan.edits.iter().enumerate() {
                        if ui
                            .selectable_label(picked.is_none() && i == index, RichText::new(format!("~ {}", edit.path)).monospace())
                            .on_hover_text(&edit.why)
                            .clicked()
                        {
                            index = i;
                            app.review_file = None;
                        }
                    }
                });
            if ui.button("Copy").clicked() {
                let text = match (&picked, picked_edit) {
                    (Some(path), None) => app.file_lines(path).map(|lines| lines.join("\n")).unwrap_or_else(|error| error),
                    (_, Some(i)) => app.review_text(i),
                    _ => app.review_text(index),
                };
                ui.ctx().copy_text(text);
            }
        });
        app.review_index = index;
        let height = list_height(app, "review", 360.0);
        well(ui, &p, |ui| {
            egui::ScrollArea::both().id_salt("review").min_scrolled_height(height).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                match (&picked, picked_edit) {
                    (Some(path), None) => match app.file_lines(path) {
                        Ok(lines) => {
                            let width = lines.len().to_string().len();
                            for (number, line) in lines.iter().enumerate().take(4000) {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{:>width$} ", number + 1)).monospace().color(p.text_dim.gamma_multiply(0.7)));
                                    ui.add(egui::Label::new(RichText::new(line).monospace()).extend());
                                });
                            }
                        }
                        Err(error) => {
                            ui.label(RichText::new(error).color(p.text_dim));
                        }
                    },
                    _ if plan.edits.is_empty() => {
                        ui.label(RichText::new("This profile is a preview: no edits yet. Pick a file in Components to read it.").color(p.text_dim));
                    }
                    (_, edit) => match app.review_lines(edit.unwrap_or(index)) {
                        Ok(lines) => {
                            for line in lines.iter() {
                                let (prefix, color, fill) = match line.kind {
                                    LineKind::Header => ("", p.accent, Color32::TRANSPARENT),
                                    LineKind::Note => ("", p.text_dim, Color32::TRANSPARENT),
                                    LineKind::Context => ("  ", p.text_dim, Color32::TRANSPARENT),
                                    LineKind::Removed => ("- ", p.removed, p.removed.gamma_multiply(0.12)),
                                    LineKind::Added => ("+ ", p.added, p.added.gamma_multiply(0.12)),
                                };
                                egui::Frame::new().fill(fill).show(ui, |ui| {
                                    ui.add(egui::Label::new(RichText::new(format!("{prefix}{}", line.text)).monospace().color(color)).extend());
                                });
                            }
                        }
                        Err(error) => {
                            ui.label(RichText::new(error).color(p.removed));
                        }
                    },
                }
            });
        });
        resize_handle(app, ui, "review", 360.0);
    });
}

/// Minimum and target Android API levels: Automatic (ProjectIle's pick) or a number.
fn android_levels(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let sdk = crate::android::Sdk::detect();
    let (auto_min, auto_target) = crate::android::resolve(0, 0, &sdk);
    ui.add_space(4.0);
    for (label, value, auto, hint) in [
        ("Minimum API", &mut app.settings.options.android_min_sdk, auto_min, "oldest Android the app installs on"),
        ("Target API", &mut app.settings.options.android_target_sdk, auto_target, "Google Play requires 36 since 31 Aug 2026"),
    ] {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).color(p.text_dim));
            let mut automatic = *value == 0;
            if ui.checkbox(&mut automatic, format!("Automatic ({auto})")).changed() {
                *value = if automatic { 0 } else { auto };
            }
            if !automatic {
                ui.add(egui::DragValue::new(value).range(crate::android::LOWEST_MIN_SDK..=crate::android::HIGHEST_SDK));
            }
            ui.label(RichText::new(hint).small().color(p.text_dim));
        });
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("JDK version").color(p.text_dim));
        egui::ComboBox::from_id_salt("android-java-version").selected_text(format!("Java {}", app.settings.options.android_java_version)).show_ui(ui, |ui| {
            for version in crate::android::JAVA_VERSIONS {
                ui.selectable_value(&mut app.settings.options.android_java_version, *version, format!("Java {version}"));
            }
        });
        ui.label(RichText::new("Gradle version").color(p.text_dim));
        egui::ComboBox::from_id_salt("android-gradle-version").selected_text(&app.settings.options.android_gradle_version).show_ui(ui, |ui| {
            for version in crate::android::GRADLE_VERSIONS {
                ui.selectable_value(&mut app.settings.options.android_gradle_version, (*version).into(), *version);
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        let java = app.settings.options.android_java_version;
        ui.hyperlink_to(format!("Download Temurin JDK {java}"), format!("https://adoptium.net/temurin/releases/?version={java}"));
        let gradle = &app.settings.options.android_gradle_version;
        ui.hyperlink_to(format!("Download Gradle {gradle}"), "https://gradle.org/releases/");
    });
    let platforms = if sdk.platforms.is_empty() { "none found".to_owned() } else { sdk.platforms.iter().map(u32::to_string).collect::<Vec<_>>().join(", ") };
    let root = sdk.root.as_ref().map_or_else(|| "Android SDK not found".to_owned(), |r| r.display().to_string());
    let list = |items: &[String]| if items.is_empty() { "none".to_owned() } else { items.join(", ") };
    plain(
        ui,
        RichText::new(format!("SDK: {root} · platforms {platforms} · NDK {} · build tools {}", list(&sdk.ndks), list(&sdk.build_tools)))
            .small()
            .color(p.text_dim),
    );
    if sdk.ndks.is_empty() {
        plain(ui, RichText::new("No NDK found: install one with Android Studio's SDK Manager to build the Rust side.").small().color(p.warning));
    }
}

fn draw_git(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let git = app.git.clone();
    let subtitle = match &git.project {
        Some(project) => format!("For {}", project.display()),
        None => "Generate a project first; then commit it and publish it to GitHub from here.".to_owned(),
    };
    section(ui, &p, "Git and GitHub", &subtitle, "github", |ui| {
        let busy = app.task_busy().is_some();
        ui.horizontal_wrapped(|ui| {
            let status = match (&git.project, git.git.is_some(), git.repo) {
                (None, _, _) => ("no project yet".to_owned(), p.text_dim),
                (_, false, _) => ("git is not installed".to_owned(), p.warning),
                (_, true, false) => ("not a git repository yet".to_owned(), p.text_dim),
                (_, true, true) => (
                    format!(
                        "branch {} · {} · {}",
                        git.branch.as_deref().unwrap_or("?"),
                        if git.changes == 0 { "no uncommitted changes".to_owned() } else { format!("{} changed file(s)", git.changes) },
                        git.remote.as_deref().unwrap_or("not on GitHub yet")
                    ),
                    p.text,
                ),
            };
            ui.label(RichText::new(status.0).color(status.1));
            if ui.small_button("Refresh").clicked() {
                app.refresh_git();
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.commit_message)
                    .hint_text("Commit message, e.g. Try a softer brush")
                    .desired_width((ui.available_width() - 110.0).max(120.0)),
            );
            if ui.add_enabled(git.project.is_some() && git.git.is_some() && !busy, egui::Button::new("Commit")).clicked() {
                let task = crate::git::commit(&git, &app.commit_message);
                app.confirm_task(task);
            }
        });
        ui.add_space(6.0);
        match (&git.gh, &git.gh_user) {
            (None, _) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("GitHub needs the GitHub CLI (gh).");
                    if let Some(command) = app.system.package_manager.and_then(|pm| pm.install(crate::system::Tool::Gh)) {
                        ui.label(RichText::new(&command).monospace());
                        if ui.small_button("Copy").clicked() {
                            ui.ctx().copy_text(command.clone());
                        }
                    }
                    ui.hyperlink_to("cli.github.com", "https://cli.github.com/");
                });
            }
            (Some(_), None) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Sign in to GitHub once, in a terminal:");
                    let command = "gh auth login --web --git-protocol https";
                    ui.label(RichText::new(command).monospace());
                    if ui.small_button("Copy").clicked() {
                        ui.ctx().copy_text(command.to_owned());
                    }
                    if ui.small_button("Check again").clicked() {
                        app.refresh_git();
                    }
                });
            }
            (Some(_), Some(user)) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("Signed in to GitHub as {user}")).color(p.added));
                    if git.remote.is_some() {
                        if ui.add_enabled(!busy, egui::Button::new("Push")).clicked() {
                            let task = crate::git::push(&git);
                            app.confirm_task(task);
                        }
                    } else {
                        ui.label("Repository");
                        ui.add(egui::TextEdit::singleline(&mut app.repo_name).desired_width(160.0));
                        ui.checkbox(&mut app.repo_private, "Private");
                        if ui.add_enabled(!busy, egui::Button::new("Publish to GitHub…")).clicked() {
                            let task = crate::git::publish(&git, &app.repo_name, app.repo_private);
                            app.confirm_task(task);
                        }
                    }
                });
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Output pane (Configure / Generate, progress, log)
// ---------------------------------------------------------------------------------------------

pub fn draw_output(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let configure = ui.button("Configure").on_hover_text(format!("Rescan the source and check the plan ({})", ui.ctx().format_shortcut(&keys::CONFIGURE)));
        help::target(ui, configure.rect, "configure");
        if configure.clicked() {
            app.configure();
        }
        let ready = app.can_generate();
        let generate = egui::Button::new(RichText::new("  Generate  ").strong().color(if ready.is_ok() { p.accent_text } else { p.text_dim }))
            .fill(if ready.is_ok() { p.accent } else { p.field });
        let response = ui.add_enabled(ready.is_ok(), generate);
        let response = match &ready {
            Ok(()) => response.on_hover_text(format!("Create the project ({})", ui.ctx().format_shortcut(&keys::GENERATE))),
            Err(reason) => response.on_disabled_hover_text(reason),
        };
        help::target(ui, response.rect, if ready.is_ok() { "generate" } else { "cant-generate" });
        if response.clicked() {
            app.start_generation(ui.ctx());
        }
        let candidate = app.regen_candidate();
        let regen = ui
            .add_enabled(candidate.is_some() && !app.is_generating(), egui::Button::new("Regenerate…"))
            .on_hover_text("Rebuild an existing ProjectIle project from the current source, with its saved settings")
            .on_disabled_hover_text("Generate a project first, or point Creates at a folder ProjectIle made");
        help::target(ui, regen.rect, "regenerate");
        if regen.clicked()
            && let Some(dir) = candidate
        {
            app.open_regenerate(dir);
        }
        let quick = ui
            .add_enabled(ready.is_ok() && app.task_busy().is_none(), egui::Button::new("Generate, build & run"))
            .on_hover_text("Skip straight through: generate, compile with Cargo, then start the editor");
        help::target(ui, quick.rect, "quick-run");
        if quick.clicked() {
            app.settings.build_after = true;
            app.settings.run_after = true;
            app.start_generation(ui.ctx());
        }
        let last = app.last_generated.clone();
        let open_folder = ui.add_enabled(last.is_some(), egui::Button::new("Open Folder")).on_hover_text("Show the generated project in your file manager");
        help::target(ui, open_folder.rect, "after");
        if open_folder.clicked()
            && let Some(path) = &last
        {
            app.open_path(path);
        }
        if ui
            .add_enabled(last.is_some() && app.rust.vscode_path.is_some(), egui::Button::new("VS Code"))
            .on_hover_text("Open the generated project in VS Code")
            .clicked()
            && let Some(path) = &last
        {
            app.open_in_vscode(path);
        }
        if let Some((done, total)) = app.job_progress() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let fraction = (done as f32 / total.max(1) as f32).clamp(0.0, 1.0);
                ui.add(egui::ProgressBar::new(fraction).desired_width(ui.available_width().min(220.0)).show_percentage().animate(true));
            });
        }
    });
    ui.add_space(4.0);
    let footer_height = ui.spacing().interact_size.y + 8.0;
    let log_frame = egui::Frame::new()
        .fill(p.field)
        .stroke(egui::Stroke::new(1.0, p.card_border))
        .corner_radius(f32::from(p.radius))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt("output")
                .max_height((ui.available_height() - footer_height).max(40.0))
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for (kind, line) in &app.logs {
                        ui.add(egui::Label::new(RichText::new(line).monospace().color(log_color(&p, *kind))).wrap());
                    }
                });
        });
    help::target(ui, log_frame.response.rect, "generate");
    ui.horizontal(|ui| {
        if ui.small_button("Copy output").clicked() {
            ui.ctx().copy_text(app.log_text());
            app.notice = Some((LogKind::Info, "Output copied to the clipboard.".into()));
        }
        if ui.small_button("Clear").clicked() {
            app.logs.clear();
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------------------------

/// What one row of the Rust list stands for.
struct Entry {
    toolchain: String,
    badge: String,
    title: String,
    subtitle: String,
    installed: bool,
    is_default: bool,
    active: bool,
}

pub fn draw_rust(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let ctx = ui.ctx().clone();
    let min = app.inventory.as_ref().ok().and_then(|inventory| crate::source::rust_version(&inventory.root));
    let min_minor = min.as_deref().and_then(rust::minor_of);
    let has_rustup = app.rust_state.rustup.is_some();
    let busy = app.task_busy().is_some();

    section(ui, &p, "Active toolchain", "The Rust that `cargo` and `rustc` use right now.", "rust", |ui| {
        ui.horizontal(|ui| {
            let version = app.rust_state.active_version().unwrap_or("not installed");
            ui.label(RichText::new(format!("Rust {version}")).size(22.0).family(egui::FontFamily::Name("strong".into())));
            if let (Some(active), Some(min_minor)) = (app.rust_state.active_version().and_then(rust::minor_of), min_minor) {
                if active >= min_minor {
                    pill(ui, &p, &format!("builds PhotoCraft (needs {})", min.as_deref().unwrap_or("?")), p.added);
                } else {
                    pill(ui, &p, &format!("too old: PhotoCraft needs {}", min.as_deref().unwrap_or("?")), p.removed);
                }
            }
        });
        ui.label(RichText::new(app.rust_state.origin()).small().color(p.text_dim));
        rust_book_link(app, ui);
        if let Some(error) = &app.rust_state.error {
            plain(ui, RichText::new(error).small().color(p.removed));
        }
        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                app.rescan_rust();
            }
            if ui.add_enabled(has_rustup && !busy, egui::Button::new("Update all")).on_disabled_hover_text("Needs rustup").clicked() {
                app.confirm_rust(Action::UpdateAll);
            }
        });
    });

    if !has_rustup {
        section(
            ui,
            &p,
            "Switching versions needs rustup",
            "rustup is Rust's official toolchain manager. Your Rust comes from your system instead.",
            "no-rustup",
            |ui| {
                plain(
                    ui,
                    RichText::new(
                        "Install rustup for your user to install, switch and pin Rust versions here. ProjectIle shows every command and asks before running it; it never uses sudo.",
                    ),
                );
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!busy && !cfg!(windows), egui::Button::new(RichText::new("Install rustup…").color(p.accent_text)).fill(p.accent))
                        .clicked()
                    {
                        app.confirm_rust(Action::InstallRustup);
                    }
                    ui.hyperlink_to("rustup.rs", "https://rustup.rs/");
                });
                if app.rust_state.rustc_path.as_deref().is_some_and(|path| path.starts_with("/usr")) {
                    plain(ui, RichText::new("Or use your package manager, in a terminal. On Arch Linux: `sudo pacman -S rustup` (it replaces the `rust` package), then `rustup default stable`.").small().color(p.text_dim));
                }
            },
        );
    }

    let pin = app.settings.options.toolchain.clone();
    section(ui, &p, "Toolchains", "Install a version, make it the default, or pin it for the next generated project.", "rustup", |ui| {
        ui.horizontal(|ui| {
            for (view, label) in [(RustView::Installed, "Installed"), (RustView::Releases, "Releases"), (RustView::Channels, "Channels")] {
                if ui.selectable_label(app.rust_view == view, label).clicked() {
                    app.rust_view = view;
                }
            }
            ui.add(egui::TextEdit::singleline(&mut app.rust_filter).hint_text("Search, e.g. 1.95").desired_width(140.0));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Pinned for new projects:").small().color(p.text_dim));
            if pin.is_empty() {
                ui.label(RichText::new("none (projects use your default Rust)").small());
            } else {
                pill(ui, &p, &pin, p.accent);
                if ui.small_button("Clear").clicked() {
                    app.settings.options.toolchain.clear();
                }
            }
        });
        explain(ui, app, "Pinning writes rust-toolchain.toml into the generated project, so rustup uses that exact version there.");
        ui.add_space(4.0);

        let installed_shorts: Vec<String> = app.rust_state.installed.iter().map(|t| t.short().to_owned()).collect();
        let default = app.rust_state.default_short().map(str::to_owned);
        let active_version = app.rust_state.active_version().map(str::to_owned);
        let is_installed = |name: &str| installed_shorts.iter().any(|s| s == name || s.strip_suffix(".0") == Some(name));
        let mut entries: Vec<Entry> = match app.rust_view {
            RustView::Installed if has_rustup => app
                .rust_state
                .installed
                .iter()
                .map(|t| Entry {
                    toolchain: t.short().to_owned(),
                    badge: badge_for(t.short()),
                    title: format!("Rust {}", t.short()),
                    subtitle: t.name.clone(),
                    installed: true,
                    is_default: t.is_default,
                    active: t.is_active,
                })
                .collect(),
            RustView::Installed => active_version
                .iter()
                .map(|v| Entry {
                    toolchain: v.clone(),
                    badge: badge_for(v),
                    title: format!("Rust {v}"),
                    subtitle: app.rust_state.origin(),
                    installed: true,
                    is_default: true,
                    active: true,
                })
                .collect(),
            RustView::Releases => rust::releases(rust::today())
                .into_iter()
                .map(|r| {
                    let version = r.version();
                    let meets = min_minor.is_none_or(|m| r.minor >= m);
                    Entry {
                        badge: format!("1.{}", r.minor),
                        title: format!("Rust {version}"),
                        subtitle: format!("released {}{}", r.date_text(), if meets { "" } else { " · too old for PhotoCraft" }),
                        installed: is_installed(&version),
                        is_default: default.as_deref() == Some(version.as_str()),
                        active: active_version.as_deref() == Some(version.as_str()),
                        toolchain: version,
                    }
                })
                .collect(),
            RustView::Channels => rust::CHANNELS
                .iter()
                .map(|(channel, about)| Entry {
                    toolchain: (*channel).to_owned(),
                    badge: channel.get(..1).unwrap_or("?").to_uppercase(),
                    title: format!("Rust {channel}"),
                    subtitle: (*about).to_owned(),
                    installed: is_installed(channel),
                    is_default: default.as_deref() == Some(*channel),
                    active: false,
                })
                .collect(),
        };
        let filter = app.rust_filter.trim().to_lowercase();
        entries.retain(|e| filter.is_empty() || e.title.to_lowercase().contains(&filter) || e.subtitle.to_lowercase().contains(&filter));
        if entries.is_empty() {
            ui.label(RichText::new("Nothing to show.").color(p.text_dim));
        }
        let height = list_height(app, "rust-list", 460.0);
        egui::ScrollArea::vertical().id_salt("rust-list").min_scrolled_height(height).max_height(height).auto_shrink([false, false]).show(ui, |ui| {
            for entry in &entries {
                rust_row(app, ui, &p, entry, has_rustup, busy);
            }
        });
        resize_handle(app, ui, "rust-list", 460.0);
    });

    draw_android_tools(app, ui);
    draw_useful_commands(app, ui);

    if !app.rust_log.is_empty() || busy {
        section(ui, &p, "rustup output", "", "rustup", |ui| {
            if let Some(task) = app.task_busy() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("{}…", task.title));
                });
            }
            let height = list_height(app, "rust-log", 200.0);
            well(ui, &p, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("rust-log")
                    .min_scrolled_height(height)
                    .max_height(height)
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (kind, line) in &app.rust_log {
                            ui.add(egui::Label::new(RichText::new(line).monospace().color(log_color(&p, *kind))).wrap());
                        }
                    });
            });
            resize_handle(app, ui, "rust-log", 200.0);
            if ui.small_button("Copy").clicked() {
                ctx.copy_text(app.rust_log.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join("\n"));
            }
        });
    }
}

fn draw_android_tools(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let ctx = ui.ctx().clone();
    if app.android_tools.is_none() && !app.checking_android_tools() {
        app.check_android_tools(&ctx);
    }
    section(
        ui,
        &p,
        "Android tools",
        "What building the Android app needs: Java, Gradle, the Android SDK tools, the NDK and Rust's Android targets.",
        "android-tools",
        |ui| {
            ui.horizontal(|ui| {
                if app.checking_android_tools() {
                    ui.spinner();
                    ui.label(RichText::new("Checking… (gradle and sdkmanager take a few seconds)").small().color(p.text_dim));
                } else if ui.button("Check again").clicked() {
                    app.check_android_tools(&ctx);
                }
                if let Some(root) = app.android_tools.as_ref().and_then(|t| t.sdk_root.clone()) {
                    ui.label(RichText::new(format!("SDK: {}", root.display())).small().color(p.text_dim));
                }
            });
            let Some(found) = app.android_tools.clone() else { return };
            let busy = app.task_busy().is_some();
            for check in &found.checks {
                egui::Frame::new()
                    .fill(p.field)
                    .stroke(egui::Stroke::new(1.0, if check.ok { p.card_border } else { p.warning.gamma_multiply(0.6) }))
                    .corner_radius(f32::from(p.radius))
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(if check.ok { "✔" } else { "✖" }).color(if check.ok { p.added } else { p.warning }));
                            ui.label(RichText::new(check.name).strong());
                            ui.label(RichText::new(check.what).small().color(p.text_dim));
                        });
                        if let Some(found) = &check.found {
                            ui.add(egui::Label::new(RichText::new(found).small().monospace()).truncate());
                        }
                        if let Some(problem) = &check.problem {
                            plain(ui, RichText::new(problem).small().color(p.warning));
                        }
                        ui.horizontal_wrapped(|ui| {
                            for fix in &check.fixes {
                                match fix {
                                    crate::tools::Fix::Run { title, program, args, explanation } => {
                                        if ui.add_enabled(!busy, egui::Button::new(RichText::new(format!("{title}…")).small())).clicked() {
                                            app.task_confirm = Some(crate::command::Task {
                                                title: title.clone(),
                                                explanation: explanation.clone(),
                                                program: program.clone(),
                                                args: args.clone(),
                                                cwd: None,
                                                log: crate::command::Log::Rust,
                                                then: crate::command::Then::Nothing,
                                            });
                                        }
                                    }
                                    crate::tools::Fix::Terminal(command) => {
                                        ui.label(RichText::new(command).small().monospace());
                                        if ui.small_button("Copy").on_hover_text("Run it in a terminal; it needs administrator rights").clicked() {
                                            ui.ctx().copy_text(command.clone());
                                        }
                                    }
                                }
                            }
                            ui.hyperlink_to(RichText::new(check.link.0).small(), check.link.1);
                        });
                    });
                ui.add_space(3.0);
            }
        },
    );
}

fn draw_useful_commands(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let project = app.git_project();
    let subtitle = match &project {
        Some(dir) => format!("Copy them into a terminal, or Run the safe ones in {}.", dir.display()),
        None => "Copy them into a terminal opened in your project folder. Generate a project to Run the safe ones from here.".to_owned(),
    };
    section(ui, &p, "Useful commands", &subtitle, "commands", |ui| {
        for (group, list) in [
            ("Everyday Rust (Cargo)", crate::tools::RUST_COMMANDS),
            ("Toolchains (rustup)", crate::tools::RUSTUP_COMMANDS),
            ("Your PhotoCore project", crate::tools::PHOTOCORE_COMMANDS),
            ("Android development", crate::tools::ANDROID_COMMANDS),
        ] {
            egui::CollapsingHeader::new(RichText::new(group).strong()).id_salt(("commands", group)).default_open(group == "Everyday Rust (Cargo)").show(
                ui,
                |ui| {
                    for cmd in list {
                        ui.horizontal(|ui| {
                            if ui.small_button("Copy").clicked() {
                                ui.ctx().copy_text(cmd.command.to_owned());
                            }
                            if cmd.runnable
                                && ui.add_enabled(project.is_some() && app.task_busy().is_none(), egui::Button::new(RichText::new("Run").small())).clicked()
                            {
                                app.run_useful(cmd.command);
                            }
                            ui.label(RichText::new(cmd.command).monospace());
                            ui.label(RichText::new(cmd.what).small().color(p.text_dim));
                        });
                    }
                },
            );
        }
        ui.horizontal_wrapped(|ui| {
            ui.hyperlink_to(RichText::new("All Cargo commands").small(), "https://doc.rust-lang.org/cargo/commands/");
            ui.hyperlink_to(RichText::new("rustup book").small(), "https://rust-lang.github.io/rustup/");
            ui.hyperlink_to(RichText::new("adb").small(), "https://developer.android.com/tools/adb");
            ui.hyperlink_to(RichText::new("sdkmanager").small(), "https://developer.android.com/tools/sdkmanager");
        });
    });
}

fn badge_for(toolchain: &str) -> String {
    match rust::minor_of(toolchain) {
        Some(minor) => format!("1.{minor}"),
        None => toolchain.get(..1).unwrap_or("?").to_uppercase(),
    }
}

fn rust_row(app: &mut ProjectIle, ui: &mut egui::Ui, p: &Palette, entry: &Entry, has_rustup: bool, busy: bool) {
    egui::Frame::new()
        .fill(if entry.active { p.accent.gamma_multiply(0.08) } else { p.field })
        .stroke(egui::Stroke::new(1.0, if entry.is_default { p.accent.gamma_multiply(0.7) } else { p.card_border }))
        .corner_radius(f32::from(p.radius) + 2.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(42.0, 42.0), egui::Sense::hover());
                let fill = if entry.installed { p.accent } else { p.hover };
                ui.painter().rect_filled(rect, f32::from(p.radius) + 2.0, fill);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &entry.badge,
                    egui::FontId::new(13.0, egui::FontFamily::Name("strong".into())),
                    if entry.installed { p.accent_text } else { p.text },
                );
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&entry.title).strong());
                        if entry.is_default {
                            pill(ui, p, "default", p.accent);
                        } else if entry.installed {
                            pill(ui, p, "installed", p.added);
                        }
                        if entry.active && !entry.is_default {
                            pill(ui, p, "active", p.added);
                        }
                        if app.settings.options.toolchain == entry.toolchain {
                            pill(ui, p, "pinned", p.changed);
                        }
                    });
                    ui.add(egui::Label::new(RichText::new(&entry.subtitle).small().color(p.text_dim)).truncate());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let needs = "Needs rustup (see above)";
                    let pinned = app.settings.options.toolchain == entry.toolchain;
                    if ui.selectable_label(pinned, "Pin").on_hover_text("Write rust-toolchain.toml with this version into the next generated project").clicked()
                    {
                        if pinned {
                            app.settings.options.toolchain.clear();
                        } else if rust::valid_toolchain(&entry.toolchain) {
                            app.settings.options.toolchain = entry.toolchain.clone();
                        }
                    }
                    if !entry.installed {
                        let install = egui::Button::new(RichText::new("Install").color(p.accent_text)).fill(p.accent);
                        if ui.add_enabled(has_rustup && !busy, install).on_disabled_hover_text(needs).clicked() {
                            app.confirm_rust(Action::Install(entry.toolchain.clone()));
                        }
                    } else if has_rustup {
                        ui.menu_button("⋯", |ui| {
                            if ui.add_enabled(!busy && !entry.is_default, egui::Button::new("Uninstall…")).clicked() {
                                app.confirm_rust(Action::Uninstall(entry.toolchain.clone()));
                                ui.close();
                            }
                            if ui.button("Copy install command").clicked() {
                                ui.ctx().copy_text(Action::Install(entry.toolchain.clone()).command_text(app.rust_state.rustup.as_deref()));
                                ui.close();
                            }
                        });
                        if !entry.is_default && ui.add_enabled(!busy, egui::Button::new("Use by default")).clicked() {
                            app.confirm_rust(Action::SetDefault(entry.toolchain.clone()));
                        }
                    }
                });
            });
        });
    ui.add_space(4.0);
}

/// The confirmation shown before "Delete it and make it anew" removes an existing folder.
pub fn draw_delete_confirm(app: &mut ProjectIle, ctx: &egui::Context) {
    let Some((path, files, made_by_projectile)) = app.delete_confirm.clone() else { return };
    let p = app.palette;
    let mut decision = None;
    let response = egui::Modal::new(egui::Id::new("delete-confirm")).show(ctx, |ui| {
        ui.set_max_width(440.0);
        ui.label(RichText::new("Delete the existing folder?").size(17.0).family(egui::FontFamily::Name("strong".into())));
        ui.add_space(4.0);
        well(ui, &p, |ui| {
            ui.add(egui::Label::new(RichText::new(path.display().to_string()).monospace()).wrap());
        });
        let count = if files >= 100_000 { "100,000 or more files".to_owned() } else { format!("{files} file(s)") };
        plain(
            ui,
            RichText::new(format!("{count} will be permanently deleted, then a new project is generated in its place. This cannot be undone."))
                .color(p.removed),
        );
        plain(
            ui,
            RichText::new(if made_by_projectile {
                "The folder was made by ProjectIle (it has .projectile.json). Your own changes in it will be lost too."
            } else {
                "This folder was not made by ProjectIle. Make sure nothing in it is needed."
            })
            .small()
            .color(p.text_dim),
        );
        plain(ui, RichText::new("Prefer \"Keep the old folder: rename it aside\" if you are not sure.").small().color(p.text_dim));
        ui.add_space(8.0);
        decision = dialog_buttons(app, ui, RichText::new("Delete and generate").color(Color32::WHITE), p.removed);
    });
    if response.should_close() && decision.is_none() {
        decision = Some(false);
    }
    match decision {
        Some(true) => {
            app.delete_confirm = None;
            app.delete_confirmed = true;
            app.start_generation(ctx);
        }
        Some(false) => {
            app.delete_confirm = None;
            app.announce(LogKind::Info, "Nothing was deleted.".into());
        }
        None => {}
    }
}

/// The Regenerate dialog: what the project records, what to do with its folder, then generate.
pub fn draw_regenerate(app: &mut ProjectIle, ctx: &egui::Context) {
    let Some(mut choice) = app.regen.clone() else { return };
    let p = app.palette;
    let mut decision = None;
    let response = egui::Modal::new(egui::Id::new("regenerate")).show(ctx, |ui| {
        ui.set_max_width(480.0);
        let name = choice.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        ui.label(RichText::new(format!("Regenerate {name}")).size(17.0).family(egui::FontFamily::Name("strong".into())));
        well(ui, &p, |ui| {
            ui.add(egui::Label::new(RichText::new(choice.dir.display().to_string()).monospace()).wrap());
        });
        let then = choice.saved.source_revision.as_deref().unwrap_or("an unknown revision");
        let now = app.inventory.as_ref().ok().and_then(|i| i.revision.clone()).unwrap_or_else(|| "unknown".into());
        plain(
            ui,
            RichText::new(format!(
                "Made as a {} project from {then}. It will be rebuilt from the current source ({now}), backup included.",
                choice.saved.profile
            ))
            .small()
            .color(p.text_dim),
        );
        ui.add_space(6.0);
        ui.checkbox(&mut choice.use_saved, "Keep the profile and options saved in the project")
            .on_hover_text("Untick to use what is set on the Create Project tab now");
        ui.add_space(4.0);
        ui.label(RichText::new("What happens to the current folder").strong());
        for mode in [plan::OnExisting::RenameAside, plan::OnExisting::Merge, plan::OnExisting::Delete] {
            let color = if mode == plan::OnExisting::Delete { p.removed } else { p.text };
            if ui.radio(choice.mode == mode, RichText::new(mode.label()).color(color)).clicked() {
                choice.mode = mode;
            }
            if choice.mode == mode {
                ui.indent(("regen-mode", mode.label()), |ui| plain(ui, RichText::new(mode.detail()).small().color(p.text_dim)));
            }
        }
        ui.add_space(8.0);
        decision = dialog_buttons(app, ui, RichText::new("Regenerate").color(p.accent_text), p.accent);
    });
    if response.should_close() && decision.is_none() {
        decision = Some(false);
    }
    match decision {
        Some(true) => {
            app.regen = None;
            app.regenerate(ctx, choice);
        }
        Some(false) => app.regen = None,
        None => app.regen = Some(choice),
    }
}

/// Two dialog buttons in the order the desktop expects: the main action last (GNOME, macOS) or
/// first (KDE Plasma, Windows). Returns `Some(true)` for the action, `Some(false)` for Cancel.
fn dialog_buttons(app: &ProjectIle, ui: &mut egui::Ui, action: RichText, fill: Color32) -> Option<bool> {
    let mut decision = None;
    let action_last = app.layout().action_last;
    let layout = if action_last { egui::Layout::right_to_left(egui::Align::Center) } else { egui::Layout::left_to_right(egui::Align::Center) };
    ui.with_layout(layout, |ui| {
        if ui.add(egui::Button::new(action).fill(fill)).clicked() {
            decision = Some(true);
        }
        if ui.button("Cancel").clicked() {
            decision = Some(false);
        }
    });
    decision
}

/// The confirmation shown before any external command runs (rustup, downloads, git, gh, cargo).
pub fn draw_task_confirm(app: &mut ProjectIle, ctx: &egui::Context) {
    let Some(task) = app.task_confirm.clone() else { return };
    let p = app.palette;
    let command = task.command_text();
    let mut decision = None;
    let response = egui::Modal::new(egui::Id::new("task-confirm")).show(ctx, |ui| {
        ui.set_max_width(460.0);
        ui.label(RichText::new(&task.title).size(17.0).family(egui::FontFamily::Name("strong".into())));
        ui.add_space(4.0);
        plain(ui, RichText::new(&task.explanation));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("This command will run:").small().color(p.text_dim));
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(command.clone());
            }
        });
        well(ui, &p, |ui| {
            ui.add(egui::Label::new(RichText::new(&command).monospace()).wrap());
        });
        ui.add_space(8.0);
        decision = dialog_buttons(app, ui, RichText::new("Run").color(p.accent_text), p.accent);
    });
    if response.should_close() && decision.is_none() {
        decision = Some(false);
    }
    match decision {
        Some(true) => {
            app.task_confirm = None;
            if task.log == crate::command::Log::Rust {
                app.page = Page::Rust;
            }
            app.start_task(ctx, task);
        }
        Some(false) => app.task_confirm = None,
        None => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Welcome
// ---------------------------------------------------------------------------------------------

/// How to get the offline docs here: rustup's component, or ProjectIle's own download of the
/// official archive when Rust came from the system (no root needed either way).
fn docs_action(app: &ProjectIle) -> Option<Action> {
    if app.rust_state.rustup.is_some() {
        return Some(Action::AddDocs);
    }
    let version = app.rust_state.active_version()?.to_owned();
    let host = app.rust_state.host.clone()?;
    Some(Action::DownloadDocs { version, host })
}

/// The offline Rust book link when the file exists, otherwise the online one and a way to
/// install the docs.
fn rust_book_link(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    match app.rust_state.offline_book.clone() {
        Some(book) => {
            ui.horizontal(|ui| {
                if ui.link("Read the Rust book (offline)").on_hover_text(book.display().to_string()).clicked() {
                    app.open_path(&book);
                }
                ui.hyperlink_to(RichText::new("online").small(), "https://doc.rust-lang.org/book/");
            });
        }
        None => {
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to("Read the Rust book (online)", "https://doc.rust-lang.org/book/");
                ui.label(RichText::new("· offline copy not installed").small().color(p.text_dim));
                let busy = app.task_busy().is_some();
                if let Some(action) = docs_action(app)
                    && ui.add_enabled(!busy, egui::Button::new(RichText::new("Install Rust docs…").small())).on_hover_text(action.explanation()).clicked()
                {
                    app.confirm_rust(action);
                }
                why_link(app, ui, "book");
            });
            why_body(app, ui, "book", "rust-book");
        }
    }
}

pub fn draw_welcome(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    ui.add_space(10.0);
    ui.label(RichText::new("Make PhotoCraft your own").size(24.0).family(egui::FontFamily::Name("strong".into())).color(p.text));
    plain(
        ui,
        RichText::new(
            "ProjectIle is a free, open-source app that generates your own buildable copy of PhotoCraft (by ArtCraft) to learn from, experiment with, fork or mod. It only reads PhotoCraft; it never modifies, builds or launches it.",
        )
        .color(p.text_dim),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(RichText::new("Create a project").strong().color(p.accent_text)).fill(p.accent)).clicked() {
            app.page = Page::Create;
        }
        if ui.button("Rust toolchains").clicked() {
            app.page = Page::Rust;
        }
        if ui.button("Help").clicked() {
            app.page = Page::Help;
        }
        ui.hyperlink_to("Source on GitHub", REPOSITORY);
    });
    plain(ui, RichText::new("Tip: press the mouse wheel (middle-click) on anything to open its help.").small().color(p.text_dim));

    let steps: [(&str, &str, &'static str); 4] = [
        ("1", "Name your project and choose the folder it will be stored in.", "naming"),
        ("2", "Pick a profile. Base (a small editor) and Viewer (one image, no tools) are ready; Full and Minimal show a preview.", "profile"),
        ("3", "Tick components in the tree and read the Summary and Source review: + is enacted, - won't be used, ~ is edited.", "components"),
        ("4", "Press Generate, then open the folder or VS Code and run `cargo run -p photocraft`.", "generate"),
    ];
    section(ui, &p, "How it works", "", "start", |ui| {
        for (number, text, topic) in steps {
            bullet(app, ui, RichText::new(number).strong().color(p.accent), text, &format!("step:{number}"), topic);
        }
        ui.add_space(2.0);
        bullet(
            app,
            ui,
            RichText::new("*").color(p.text_dim),
            "ProjectIle finds the PhotoCraft source it copies from by itself (Create Project › Source). Change it only if you keep more than one PhotoCraft checkout.",
            "step:source",
            "source",
        );
    });
    section(ui, &p, "Works with Claude Code (MCP)", "ProjectIle supports the Model Context Protocol, two ways.", "projectile-mcp", |ui| {
        bullet(
            app,
            ui,
            RichText::new("•").color(p.accent),
            "ProjectIle itself is an MCP server: Claude Code can plan and generate projects for you.",
            "mcp:1",
            "projectile-mcp",
        );
        bullet(
            app,
            ui,
            RichText::new("•").color(p.accent),
            "Generated projects can include agent tools, so Claude Code can drive your running editor.",
            "mcp:2",
            "mcp",
        );
        let command = crate::mcp_command();
        well(ui, &p, |ui| {
            ui.add(egui::Label::new(RichText::new(&command).monospace()).wrap());
        });
        if ui.small_button("Copy command").clicked() {
            ui.ctx().copy_text(command.clone());
        }
    });
    section(ui, &p, "Join the community", "PhotoCraft is made by people like you. Ask, share and help shape it.", "community", |ui| {
        ui.horizontal_wrapped(|ui| {
            let discord = ui.add(egui::Button::new(RichText::new("Join the ArtCraft Discord").strong().color(p.accent_text)).fill(p.accent));
            if discord.on_hover_text(DISCORD).clicked() {
                app.open_url(DISCORD);
            }
            if ui.button("Open Claude Console").on_hover_text(CLAUDE_CONSOLE).clicked() {
                app.open_url(CLAUDE_CONSOLE);
            }
            if ui.button("Source on GitHub").on_hover_text(REPOSITORY).clicked() {
                app.open_url(REPOSITORY);
            }
        });
        plain(
            ui,
            RichText::new("Report bugs, suggest features, show what you made with your PhotoCore project, or help newcomers. The Claude Console is where you manage Claude API keys and usage for agents.").small().color(p.text_dim),
        );
    });
    section(ui, &p, "New to Rust", "", "rust-book", |ui| {
        bullet(
            app,
            ui,
            RichText::new("•").color(p.accent),
            "The Rust tab shows your compiler and installs or switches versions through rustup.",
            "rust:1",
            "rust",
        );
        bullet(
            app,
            ui,
            RichText::new("•").color(p.accent),
            "`cargo run` builds and starts a program; `cargo check` is a fast compile check.",
            "rust:2",
            "after",
        );
        bullet(
            app,
            ui,
            RichText::new("•").color(p.accent),
            "PhotoCraft uses no `unsafe` code outside one isolated tablet crate, and returns errors instead of crashing.",
            "rust:3",
            "crates",
        );
        rust_book_link(app, ui);
    });
    section(ui, &p, "New to PhotoCraft (coming from Photoshop)", "", "profile", |ui| {
        bullet(
            app,
            ui,
            RichText::new("•").color(p.accent),
            "PhotoCraft aims at 1:1 Photoshop parity: same menus, shortcuts and file fidelity.",
            "ps:1",
            "profile",
        );
        bullet(
            app,
            ui,
            RichText::new("•").color(p.accent),
            "Base gives you the familiar workspace with Brush (B), Eraser (E) and Paint Bucket (G).",
            "ps:2",
            "profile",
        );
        bullet(app, ui, RichText::new("•").color(p.accent), "Keep the original source backup to compare your changes with real PhotoCraft.", "ps:3", "backup");
        ui.hyperlink_to("PhotoCraft on GitHub", "https://github.com/storytold/photocraft");
    });
    section(ui, &p, "Tip", "", "start", |ui| {
        ui.add(egui::Label::new(app.tip()).wrap());
        if ui.small_button("Next tip").clicked() {
            app.settings.tip_index = crate::next_tip_index(app.settings.tip_index);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Help
// ---------------------------------------------------------------------------------------------

pub fn draw_help(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    ui.add_space(10.0);
    ui.label(RichText::new("Help").size(24.0).family(egui::FontFamily::Name("strong".into())));
    plain(
        ui,
        RichText::new("How to make a project, and what everything in ProjectIle means. Middle-click anything in ProjectIle to jump to its topic.")
            .color(p.text_dim),
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.help_filter).hint_text("Search help…").desired_width(240.0));
        if !app.help_filter.is_empty() && ui.small_button("Clear").clicked() {
            app.help_filter.clear();
        }
    });
    let filter = app.help_filter.trim().to_lowercase();
    let selected = app.help_topic;
    let scroll = std::mem::take(&mut app.help_scroll);
    for section_name in help::SECTIONS {
        let topics: Vec<&help::Topic> = help::TOPICS
            .iter()
            .filter(|t| t.section == *section_name)
            .filter(|t| filter.is_empty() || t.title.to_lowercase().contains(&filter) || t.body.iter().any(|b| b.to_lowercase().contains(&filter)))
            .collect();
        if topics.is_empty() {
            continue;
        }
        ui.add_space(8.0);
        ui.label(RichText::new(*section_name).size(15.0).family(egui::FontFamily::Name("strong".into())).color(p.accent));
        for topic in topics {
            let is_selected = selected == Some(topic.id);
            let frame = egui::Frame::new()
                .fill(if is_selected { p.accent.gamma_multiply(0.10) } else { p.card })
                .stroke(egui::Stroke::new(1.0, if is_selected { p.accent } else { p.card_border }))
                .corner_radius(f32::from(p.radius) + 2.0)
                .inner_margin(egui::Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let header = egui::CollapsingHeader::new(RichText::new(topic.title).strong())
                        .id_salt(("help", topic.id))
                        .open(((is_selected && scroll) || !filter.is_empty()).then_some(true))
                        .show(ui, |ui| {
                            for paragraph in topic.body {
                                plain(ui, RichText::new(*paragraph));
                                ui.add_space(2.0);
                            }
                            if topic.id == "rust-book" {
                                rust_book_link(app, ui);
                            }
                            if topic.id == "projectile-mcp" {
                                let command = crate::mcp_command();
                                well(ui, &p, |ui| {
                                    ui.add(egui::Label::new(RichText::new(&command).monospace()).wrap());
                                });
                                if ui.small_button("Copy command").clicked() {
                                    ui.ctx().copy_text(command.clone());
                                }
                            }
                            if !topic.links.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Read more:").small().color(p.text_dim));
                                    for (label, url) in topic.links {
                                        ui.hyperlink_to(RichText::new(*label).small(), *url);
                                    }
                                });
                            }
                        });
                    if header.header_response.clicked() {
                        app.help_topic = Some(topic.id);
                    }
                });
            if is_selected && scroll {
                ui.scroll_to_rect(frame.response.rect, Some(egui::Align::TOP));
            }
            ui.add_space(3.0);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Preferences
// ---------------------------------------------------------------------------------------------

pub fn draw_preferences(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    let ctx = ui.ctx().clone();
    section(ui, &p, "Appearance", "How ProjectIle itself looks. Accessibility settings are in the tray icon at the bottom right.", "preferences", |ui| {
        egui::Grid::new("appearance").num_columns(2).spacing([14.0, 10.0]).min_col_width(90.0).show(ui, |ui| {
            row(ui, &p, "Theme", |ui| {
                for theme in AppTheme::ALL {
                    if ui.selectable_label(app.settings.theme == theme, RichText::new(theme.label()).family(egui::FontFamily::Name("medium".into()))).clicked()
                    {
                        app.settings.theme = theme;
                        app.apply_appearance(&ctx);
                    }
                }
            });
            row(ui, &p, "Scale", |ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - 190.0).clamp(100.0, 320.0);
                ui.add(egui::Slider::new(&mut app.pending_scale, crate::SCALE_RANGE).suffix("%").step_by(5.0));
                let changed = app.pending_scale != app.settings.scale_percent;
                let apply = egui::Button::new(RichText::new("Apply").color(if changed { p.accent_text } else { p.text_dim })).fill(if changed {
                    p.accent
                } else {
                    p.field
                });
                if ui.add_enabled(changed, apply).on_hover_text("Use this interface scale now").clicked() {
                    let scale = app.pending_scale;
                    app.set_scale(&ctx, scale);
                }
                if ui.small_button("100%").clicked() {
                    app.pending_scale = 100;
                }
            });
            if app.pending_scale != app.settings.scale_percent {
                ui.label("");
                ui.label(RichText::new(format!("Now {}%; press Apply to use {}%.", app.settings.scale_percent, app.pending_scale)).small().color(p.text_dim));
                ui.end_row();
            }
        });
    });

    section(ui, &p, "System", "What ProjectIle detected here, and the layout it picked for it.", "system", |ui| {
        let system = app.system.clone();
        egui::Grid::new("system").num_columns(2).spacing([14.0, 8.0]).min_col_width(90.0).show(ui, |ui| {
            row(ui, &p, "System", |ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, system.brand_color(&p));
                ui.label(RichText::new(&system.name).strong());
                if system.is_arch() {
                    pill(ui, &p, "Arch-based", system.brand_color(&p));
                }
            });
            row(ui, &p, "Desktop", |ui| {
                ui.label(system.desktop.clone().unwrap_or_else(|| "unknown".into()));
                if system.tiling() {
                    pill(ui, &p, "tiling", p.accent);
                }
            });
            row(ui, &p, "Packages", |ui| {
                ui.label(system.package_manager.map_or("not detected", |pm| pm.name()));
                if let Some(command) = system.package_manager.and_then(|pm| pm.install(crate::system::Tool::Rustup)) {
                    ui.label(RichText::new(format!("e.g. {command}")).small().monospace().color(p.text_dim));
                }
            });
            row(ui, &p, "Layout", |ui| {
                egui::ComboBox::from_id_salt("layout-choice").selected_text(app.settings.layout_choice.label()).width(220.0).show_ui(ui, |ui| {
                    for choice in crate::system::LayoutChoice::ALL {
                        ui.selectable_value(&mut app.settings.layout_choice, choice, choice.label());
                    }
                });
            });
            let (_, why) = system.layout(app.settings.layout_choice);
            ui.label("");
            plain(ui, RichText::new(format!("In use: {why}.")).small().color(p.text_dim));
            ui.end_row();
        });
    });

    section(ui, &p, "Window", "", "preferences", |ui| {
        let size = ctx.input(|input| input.viewport().inner_rect.map(|r| r.size()));
        ui.label(RichText::new(size.map_or_else(|| "Window size unknown".to_owned(), |s| format!("{:.0} × {:.0} points", s.x, s.y))).small().color(p.text_dim));
        ui.horizontal_wrapped(|ui| {
            for (label, width, height) in WINDOW_PRESETS {
                if ui.button(*label).on_hover_text(format!("{width:.0} × {height:.0}")).clicked() {
                    app.resize_window(&ctx, *width, *height);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Maximize / restore").clicked() {
                let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            if ui.button("Full screen").clicked() {
                ProjectIle::toggle_fullscreen(&ctx);
            }
            if ui.button("Take screenshot").on_hover_text(format!("Saves a PNG and copies it ({})", ctx.format_shortcut(&keys::SCREENSHOT))).clicked() {
                app.request_screenshot();
            }
            if ui.button("Screenshots folder").clicked() {
                match crate::screenshot_directory().and_then(|path| std::fs::create_dir_all(&path).map(|()| path).map_err(|error| error.to_string())) {
                    Ok(path) => app.open_path(&path),
                    Err(error) => app.announce(LogKind::Warning, error),
                }
            }
            if ui.button("Reset").clicked() {
                app.settings.theme = AppTheme::default();
                app.set_scale(&ctx, 100);
                app.resize_window(&ctx, crate::DEFAULT_WINDOW[0], crate::DEFAULT_WINDOW[1]);
                app.announce(LogKind::Info, "ProjectIle window settings reset.".into());
            }
        });
    });

    section(ui, &p, "Generated app startup", "Written into the generated editor's main.rs; ProjectIle's own window is unaffected.", "startup", |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 190.0).clamp(120.0, 420.0);
        let o = &mut app.settings.options;
        ui.add(egui::Slider::new(&mut o.width, 640..=3840).text("width (points)"));
        ui.add(egui::Slider::new(&mut o.height, 480..=2160).text("height (points)"));
        ui.add(egui::Slider::new(&mut o.scale_percent, 75..=200).text("UI scale (%)"));
        ui.horizontal(|ui| {
            if ui.button("Reset startup defaults").clicked() {
                let defaults = plan::Options::default();
                o.width = defaults.width;
                o.height = defaults.height;
                o.scale_percent = defaults.scale_percent;
            }
            if ui.button("Reset all project options").on_hover_text("Components ticks, MCP port, toolchain pin and startup size").clicked() {
                *o = plan::Options::default();
            }
        });
    });
}

// ---------------------------------------------------------------------------------------------
// About
// ---------------------------------------------------------------------------------------------

pub fn draw_about(app: &mut ProjectIle, ui: &mut egui::Ui) {
    let p = app.palette;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("ProjectIle").size(24.0).family(egui::FontFamily::Name("strong".into())));
        pill(ui, &p, &format!("v{}", env!("CARGO_PKG_VERSION")), p.accent);
        pill(ui, &p, "MIT OR Apache-2.0", p.added);
    });
    plain(ui, RichText::new("A standalone, open-source generator for PhotoCore projects: modular, reviewable copies of PhotoCraft by ArtCraft. Written in Rust with no unsafe code and no webview.").color(p.text_dim));

    section(ui, &p, "Open source", "ProjectIle is free software. Use it, study it, change it and share it.", "open-source", |ui| {
        plain(
            ui,
            RichText::new(
                "Licensed under the MIT License or the Apache License 2.0, at your option. Contributions are welcome: open an issue, or send a pull request.",
            ),
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button("Source code").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(REPOSITORY));
            }
            if ui.button("Report an issue").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/tiwooz/photocore/issues"));
            }
            let guide = app.inventory.as_ref().ok().map(|inventory| inventory.root.join("docs/contributing.md")).filter(|path| path.is_file());
            if ui.add_enabled(guide.is_some(), egui::Button::new("Contributing guide")).on_disabled_hover_text("Choose a PhotoCraft source first").clicked()
                && let Some(guide) = guide
            {
                app.open_path(&guide);
            }
        });
    });

    section(ui, &p, "Built with", "Open-source libraries ProjectIle depends on, and their licences.", "open-source", |ui| {
        egui::Grid::new("built-with").num_columns(3).spacing([14.0, 4.0]).striped(true).show(ui, |ui| {
            for (name, role, license) in BUILT_WITH {
                ui.label(RichText::new(*name).strong());
                ui.label(RichText::new(*role).small().color(p.text_dim));
                ui.label(RichText::new(*license).small().monospace());
                ui.end_row();
            }
        });
    });

    section(
        ui,
        &p,
        "Typography",
        "Satoshi is used for headings and buttons when a local copy is available; it is never bundled by ProjectIle.",
        "open-source",
        |ui| {
            ui.label(RichText::new("Satoshi — Indian Type Foundry / Fontshare").strong());
            if let Some(path) = &app.satoshi {
                ui.label(RichText::new(format!("Loaded from {}", path.display())).small().color(p.text_dim));
            } else {
                ui.label(RichText::new("No local Satoshi font found; the interface uses its fallback fonts.").small().color(p.text_dim));
            }
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to("Fontshare · Satoshi", "https://www.fontshare.com/fonts/satoshi");
                ui.hyperlink_to("ITF Free Font License", "https://www.fontshare.com/licenses");
            });
            plain(ui, RichText::new("Satoshi is discovered in the selected source's font/ folder, CRAFT_FONTS_DIR, and user/system font folders. Inter and JetBrains Mono are read from the selected PhotoCraft source. Font files are not copied into generated projects.").small().color(p.text_dim));
        },
    );

    section(ui, &p, "Detected tools", "", "rust", |ui| {
        egui::Grid::new("tools").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
            let rows = [
                ("rustc", app.rust.rustc.clone(), app.rust.rustc_path.clone()),
                ("cargo", app.rust.cargo.clone(), app.rust.cargo_path.clone()),
                ("rustup", app.rust.rustup.clone(), app.rust.rustup_path.clone()),
                ("VS Code", app.rust.vscode_path.as_ref().map(|_| "found".to_owned()), app.rust.vscode_path.clone()),
                ("git", app.rust.git_path.as_ref().map(|_| "found".to_owned()), app.rust.git_path.clone()),
            ];
            for (name, version, path) in rows {
                ui.label(RichText::new(name).strong());
                match version {
                    Some(version) => {
                        let path = path.map(|p| p.display().to_string()).unwrap_or_default();
                        ui.add(egui::Label::new(RichText::new(version).monospace()).truncate()).on_hover_text(path);
                    }
                    None => {
                        ui.label(RichText::new("not found").color(p.warning));
                    }
                }
                ui.end_row();
            }
        });
    });

    section(ui, &p, "Use ProjectIle from Claude Code", "ProjectIle is an MCP server: agents can plan and generate projects for you.", "projectile-mcp", |ui| {
        let command = crate::mcp_command();
        well(ui, &p, |ui| {
            ui.add(egui::Label::new(RichText::new(&command).monospace()).wrap());
        });
        ui.horizontal(|ui| {
            if ui.small_button("Copy command").clicked() {
                ui.ctx().copy_text(command.clone());
            }
            ui.label(RichText::new("Tools: projectile_status, projectile_plan, projectile_generate").small().color(p.text_dim));
        });
    });

    section(ui, &p, "Links", "", "open-source", |ui| {
        for (label, url) in LINKS {
            ui.hyperlink_to(*label, *url);
        }
    });
    plain(
        ui,
        RichText::new("Layout ideas from CMake GUI, the VST3 SDK Project Generator, Windows installers, launcher apps and Photoshop: interaction patterns only, no code or artwork reused.").small().color(p.text_dim),
    );
}
