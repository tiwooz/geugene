//! The Components tree: a Windows-installer-style component list with expand arrows, checkboxes
//! and a `git`-style sign column (`+` enacted/added, `~` edited, `·` dormant, `-` left out).
//! Component folders expand into the real source files; clicking a file shows it in Source review.

use crate::plan::{self, Options, Plan, Status};
use crate::theme::Palette;
use eframe::egui::{self, Color32, RichText, Sense};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_LISTED: usize = 400;
const INDENT: f32 = 18.0;

/// An option a checkbox in the tree controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    Backup,
    ProjectileSnapshot,
    Docs,
    VsCode,
    Mcp,
    Git,
    Android,
}

impl Toggle {
    fn value(self, o: &mut Options) -> &mut bool {
        match self {
            Self::Backup => &mut o.backup_source,
            Self::ProjectileSnapshot => &mut o.backup_projectile,
            Self::Docs => &mut o.include_docs,
            Self::VsCode => &mut o.vscode_config,
            Self::Mcp => &mut o.mcp,
            Self::Git => &mut o.git_init,
            Self::Android => &mut o.android,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// Shown, but decided by the profile (required or impossible).
    Locked(bool),
    Toggle(Toggle),
    MenuBar,
    VisualDebug,
    TopMenu(&'static str),
    Submenu {
        path: String,
        parent: &'static str,
    },
    None,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub sign: char,
    pub detail: String,
    pub note: String,
    pub bytes: Option<u64>,
    pub check: Check,
    /// Source-relative folder this node can be expanded into.
    pub browse: Option<String>,
    /// Source-relative (or generated) file this node shows in Source review.
    pub file: Option<String>,
    pub children: Vec<Node>,
}

impl Node {
    fn group(id: &str, label: &str, detail: &str, check: Check, children: Vec<Node>) -> Self {
        let mut node = Node {
            id: format!("g:{id}"),
            label: label.into(),
            sign: '·',
            detail: detail.into(),
            note: String::new(),
            bytes: None,
            check,
            browse: None,
            file: None,
            children,
        };
        node.sign = summarize(&node.children);
        let total: u64 = node.children.iter().filter_map(|c| c.bytes).sum();
        node.bytes = (total > 0).then_some(total);
        node
    }

    fn leaf(id: &str, label: &str, sign: char, detail: &str, note: &str) -> Self {
        Node {
            id: id.into(),
            label: label.into(),
            sign,
            detail: detail.into(),
            note: note.into(),
            bytes: None,
            check: Check::None,
            browse: None,
            file: Some(id.into()),
            children: Vec::new(),
        }
    }

    fn matches(&self, filter: &str) -> bool {
        filter.is_empty()
            || self.label.to_lowercase().contains(filter)
            || self.detail.to_lowercase().contains(filter)
            || self.children.iter().any(|c| c.matches(filter))
    }
}

fn summarize(children: &[Node]) -> char {
    let signs: Vec<char> = children.iter().map(|c| c.sign).collect();
    if signs.is_empty() || signs.iter().all(|s| *s == '-') {
        '-'
    } else if signs.contains(&'~') {
        '~'
    } else if signs.contains(&'+') {
        '+'
    } else {
        '·'
    }
}

fn component_node(c: &plan::Component) -> Node {
    let is_dir_like = c.path.starts_with("crates/") || c.path.starts_with("apps/") || !c.path.contains('.') || c.path == ".github" || c.path == ".cargo";
    Node {
        id: format!("c:{}", c.path),
        label: c.path.rsplit('/').next().unwrap_or(&c.path).to_owned(),
        sign: c.status.sign(),
        detail: c.role.to_owned(),
        note: format!("{}: {}", c.status.label(), c.note),
        bytes: (c.bytes > 0).then_some(c.bytes),
        check: Check::Locked(c.status != Status::Omitted),
        browse: (is_dir_like && c.files > 1 && c.group != "generated").then(|| c.path.clone()),
        file: (!is_dir_like || c.files == 1).then(|| c.path.clone()),
        children: Vec::new(),
    }
}

const LAYERS: &[&str] =
    &["L6 interface", "L5 engine", "L4 files", "L3 render", "L2 editing", "L1 document", "L0 input", "L0 formats", "L0 foundation", "crate", "tests"];

/// Build the tree for the current plan. Toggle groups reflect the options, so unticking one turns
/// its whole branch into `-` lines immediately.
pub fn build(plan: &Plan, options: &Options, ready: bool) -> Vec<Node> {
    let find = |path: &str| plan.components.iter().find(|c| c.path == path);
    let mut used = HashSet::new();
    let mut take = |path: &str| -> Option<Node> {
        let c = find(path)?;
        used.insert(path.to_owned());
        Some(component_node(c))
    };
    let mut tree = Vec::new();

    let app: Vec<Node> = ["apps/photocraft"].iter().filter_map(|p| take(p)).collect();
    tree.push(Node::group("app", "Editor application", "the program you build and run", Check::Locked(true), app));

    let mut layer_groups = Vec::new();
    for layer in LAYERS {
        let crates: Vec<Node> = plan
            .components
            .iter()
            .filter(|c| c.path.starts_with("crates/") && c.group == *layer && !(ready && c.path == "crates/automation"))
            .map(|c| c.path.clone())
            .collect::<Vec<_>>()
            .iter()
            .filter_map(|p| take(p))
            .collect();
        if !crates.is_empty() {
            layer_groups.push(Node::group(&format!("layer:{layer}"), layer, &format!("{} crate(s)", crates.len()), Check::Locked(true), crates));
        }
    }
    let crates_group = Node::group("crates", "Crates", "the libraries the editor is made of", Check::Locked(true), layer_groups);

    if ready {
        let mut backup = Vec::new();
        if let Some(mut node) = take(plan::BACKUP_SOURCE_DIR) {
            node.browse = Some(String::new());
            node.file = None;
            backup.push(node);
            backup.push(Node::leaf(plan::BACKUP_NOTE, "ORIGINAL-SOURCE.md", '+', "says what the backup is", "added"));
        }
        let mut group = Node::group("backup", "Original source backup", "untouched PhotoCraft source in backup/", Check::Toggle(Toggle::Backup), backup);
        if !options.backup_source {
            group.sign = '-';
        }
        tree.push(group);

        let snapshot = plan
            .components
            .iter()
            .find(|component| component.path == plan::PROJECTILE_SNAPSHOT_DIR)
            .map(component_node)
            .into_iter()
            .collect();
        let mut group = Node::group(
            "projectile-snapshot",
            "ProjectIle source snapshot",
            "the generator source archive included for recovery",
            Check::Toggle(Toggle::ProjectileSnapshot),
            snapshot,
        );
        if !options.backup_projectile {
            group.sign = '-';
        }
        tree.push(group);

        let mut agent: Vec<Node> = ["apps/photocraft-cli", "crates/automation"].iter().filter_map(|p| take(p)).collect();
        if options.mcp {
            agent.push(Node::leaf(".mcp.json", ".mcp.json", '+', "Claude Code server list", "added: MCP bridge to the running editor"));
            agent.push(Node::leaf(".projectile/README.md", ".projectile/", '+', "local control token folder", "added: the token itself is gitignored"));
        }
        let mut group = Node::group("mcp", "Agent tools (Claude Code MCP)", "drive the running editor from Claude Code", Check::Toggle(Toggle::Mcp), agent);
        if !options.mcp {
            group.sign = '-';
        }
        tree.push(group);

        let docs: Vec<Node> = ["docs"].iter().filter_map(|p| take(p)).collect();
        tree.push(Node::group("docs", "Documentation", "architecture and contributor guides", Check::Toggle(Toggle::Docs), docs));

        let vscode: Vec<Node> = if options.vscode_config {
            ["extensions.json", "settings.json", "tasks.json"]
                .iter()
                .map(|f| Node::leaf(&format!(".vscode/{f}"), f, '+', "VS Code project settings", "added"))
                .collect()
        } else {
            Vec::new()
        };
        let mut group = Node::group("vscode", "VS Code setup", "rust-analyzer, tasks, search excludes", Check::Toggle(Toggle::VsCode), vscode);
        if !options.vscode_config {
            group.sign = '-';
        }
        tree.push(group);

        let git = vec![Node {
            file: None,
            ..Node::leaf("git:repo", ".git", if options.git_init { '+' } else { '-' }, "first commit = the generated state", "git init, add, commit")
        }];
        tree.push(Node::group("git", "Git repository", "so `git diff` shows your own changes", Check::Toggle(Toggle::Git), git));

        let android: Vec<Node> = plan
            .edits
            .iter()
            .filter(|e| e.path.starts_with("android/") || e.path.starts_with("apps/photocraft-android/"))
            .map(|e| Node::leaf(&e.path, &e.path, '+', "", &format!("added: {}", e.why)))
            .collect();
        // Mark the plan's Android components as shown here, not under Workspace files.
        let _ = take("android");
        let _ = take("apps/photocraft-android");
        let mut group =
            Node::group("android", "Android build (experimental)", "Gradle (Java) project and a Rust entry crate", Check::Toggle(Toggle::Android), android);
        if !options.android {
            group.sign = '-';
        }
        tree.push(group);

        let pinned = !options.toolchain.is_empty();
        let pin = if pinned { take("rust-toolchain.toml").into_iter().collect() } else { Vec::new() };
        let mut group = Node::group(
            "pin",
            if pinned { "Rust toolchain pin" } else { "Rust toolchain pin (none)" },
            if pinned { "set on the Rust tab" } else { "choose a version on the Rust tab" },
            Check::Locked(pinned),
            pin,
        );
        if !pinned {
            group.sign = '-';
        }
        tree.push(group);
    }

    tree.push(crates_group);

    let mut left_out = Vec::new();
    let mut workspace = Vec::new();
    for c in &plan.components {
        if used.contains(&c.path) || c.path.starts_with("crates/") && c.status != Status::Omitted {
            continue;
        }
        if matches!(c.path.as_str(), ".mcp.json" | "PROJECTILE.md" | ".projectile.json" | ".vscode") && c.status == Status::Added {
            if c.path != ".mcp.json" && c.path != ".vscode" {
                let mut node = component_node(c);
                node.file = Some(c.path.clone());
                workspace.push(node);
            }
            continue;
        }
        if c.status == Status::Omitted {
            left_out.push(component_node(c));
        } else {
            workspace.push(component_node(c));
        }
    }
    tree.push(Node::group("workspace", "Workspace files", "build configuration, licences, assets", Check::Locked(true), workspace));
    if !left_out.is_empty() {
        let mut group = Node::group("left-out", "Not included", "left out of this profile", Check::Locked(false), left_out);
        group.sign = '-';
        tree.push(group);
    }
    tree
}

/// One folder's entries: name, is-folder, size in bytes.
type Listing = Result<Vec<(String, bool, u64)>, String>;

/// Lazily read source folders, cached by relative path.
#[derive(Default)]
pub struct Listings {
    cache: HashMap<String, Listing>,
}

impl Listings {
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    fn get(&mut self, root: &Path, relative: &str) -> &Listing {
        self.cache.entry(relative.to_owned()).or_insert_with(|| {
            let dir = if relative.is_empty() { root.to_path_buf() } else { root.join(relative) };
            let entries = std::fs::read_dir(&dir).map_err(|error| format!("could not list {}: {error}", dir.display()))?;
            let mut out: Vec<(String, bool, u64)> = entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let rel = if relative.is_empty() { name.clone() } else { format!("{relative}/{name}") };
                    if plan::excluded_from_snapshot(Path::new(&rel)) {
                        return None;
                    }
                    let meta = entry.metadata().ok()?;
                    Some((name, meta.is_dir(), meta.len()))
                })
                .collect();
            out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
            Ok(out)
        })
    }
}

/// Mutable state the tree reads and writes.
pub struct TreeState<'a> {
    pub options: &'a mut Options,
    pub expanded: &'a mut HashSet<String>,
    pub listings: &'a mut Listings,
    pub selected: &'a mut Option<String>,
    pub root: Option<&'a Path>,
    pub edited: &'a HashSet<String>,
    pub filter: String,
    pub toggles_enabled: bool,
}

/// Help topic for a tree row (used by middle-click).
fn topic_for(id: &str) -> &'static str {
    match id {
        "g:app" | "c:apps/photocraft" => "profile",
        "g:backup" | "backup/ORIGINAL-SOURCE.md" => "backup",
        "g:mcp" | ".mcp.json" | ".projectile/README.md" | "c:apps/photocraft-cli" | "c:crates/automation" => "mcp",
        "g:docs" | "c:docs" => "docs",
        "g:vscode" => "vscode",
        "g:git" | "git:repo" => "git",
        "g:android" => "android",
        _ if id.starts_with("android/") || id.starts_with("apps/photocraft-android") => "android",
        "g:pin" | "c:rust-toolchain.toml" => "pin",
        "g:workspace" => "workspace",
        "g:menu-bar" | "g:menu-debug" => "generated-menu",
        _ if id.starts_with("g:menu-top:") || id.starts_with("g:menu-sub:") => "generated-menu",
        "g:left-out" => "left-out",
        _ if id.starts_with(".vscode/") => "vscode",
        _ if id == "c:backup" => "backup",
        _ if id.starts_with("g:layer") || id == "g:crates" || id.starts_with("c:crates/") => "crates",
        _ if id.starts_with("fs:") => "review",
        _ => "components",
    }
}

pub fn sign_color(p: &Palette, sign: char) -> Color32 {
    match sign {
        '+' => p.added,
        '-' => p.removed,
        '~' => p.changed,
        _ => p.text_dim,
    }
}

pub fn show(ui: &mut egui::Ui, p: &Palette, nodes: &[Node], state: &mut TreeState<'_>) {
    let filter = state.filter.clone();
    for node in nodes.iter().filter(|n| n.matches(&filter)) {
        show_node(ui, p, node, 0, state);
    }
}

pub fn menu_node(options: &Options, submenu_paths: Result<Vec<Vec<String>>, String>) -> Node {
    let mut children = vec![Node {
        id: "g:menu-debug".into(),
        label: "egui Visual Debug menu and inspection window".into(),
        sign: if options.visual_debug_menu { '+' } else { '-' },
        detail: "built-in inspection and visual-style tools".into(),
        note: String::new(),
        bytes: None,
        check: Check::VisualDebug,
        browse: None,
        file: None,
        children: Vec::new(),
    }];
    let submenu_paths = match submenu_paths {
        Ok(paths) => paths,
        Err(error) => {
            children.push(Node {
                id: "g:menu-error".into(),
                label: error,
                sign: '!',
                detail: String::new(),
                note: String::new(),
                bytes: None,
                check: Check::None,
                browse: None,
                file: None,
                children: Vec::new(),
            });
            Vec::new()
        }
    };
    for top in plan::PROJECT_MENUS {
        let submenus = submenu_paths
            .iter()
            .filter(|path| path.first().is_some_and(|head| head == top))
            .filter_map(|path| {
                let label = path.last()?;
                let path_key = path.join(" > ");
                Some(Node {
                    id: format!("g:menu-sub:{path_key}"),
                    label: label.clone(),
                    sign: if options.hidden_submenus.contains(&path_key) { '-' } else { '·' },
                    detail: String::new(),
                    note: path_key.clone(),
                    bytes: None,
                    check: Check::Submenu { path: path_key, parent: top },
                    browse: None,
                    file: None,
                    children: Vec::new(),
                })
            })
            .collect();
        let mut node = Node::group(&format!("menu-top:{top}"), top, "top-level menu", Check::TopMenu(top), submenus);
        if options.hidden_menus.iter().any(|hidden| hidden == top) {
            node.sign = '-';
        }
        children.push(node);
    }
    let mut node = Node::group(
        "menu-bar",
        "Main menu bar, tabs and submenus",
        "include the app's top menu bar and choose which tabs and nested groups appear",
        Check::MenuBar,
        children,
    );
    if !options.show_menu_bar {
        node.sign = '-';
    }
    node
}

fn expander(ui: &mut egui::Ui, p: &Palette, open: bool, enabled: bool) -> egui::Response {
    let size = egui::vec2(14.0, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    if enabled {
        let c = rect.center();
        let color = if response.hovered() { p.text } else { p.text_dim };
        let points = if open {
            vec![c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 3.0)]
        } else {
            vec![c + egui::vec2(-2.0, -4.0), c + egui::vec2(3.0, 0.0), c + egui::vec2(-2.0, 4.0)]
        };
        ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
    }
    response
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut egui::Ui,
    p: &Palette,
    depth: usize,
    sign: char,
    expandable: bool,
    open: bool,
    check: Check,
    label: RichText,
    detail: &str,
    bytes: Option<u64>,
    selected: bool,
    topic: &'static str,
    state: &mut TreeState<'_>,
) -> (bool, egui::Response) {
    let mut toggled = false;
    let inner = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_sized([14.0, ui.spacing().interact_size.y], egui::Label::new(RichText::new(sign.to_string()).monospace().strong().color(sign_color(p, sign))));
        ui.add_space(depth as f32 * INDENT);
        if expander(ui, p, open, expandable).clicked() {
            toggled = true;
        }
        match check {
            Check::Locked(value) => {
                let mut v = value;
                ui.add_enabled(false, egui::Checkbox::without_text(&mut v));
            }
            Check::Toggle(toggle) => {
                let value = toggle.value(state.options);
                ui.add_enabled(state.toggles_enabled, egui::Checkbox::without_text(value));
            }
            Check::MenuBar => {
                let response = ui.add_enabled(state.toggles_enabled, egui::Checkbox::without_text(&mut state.options.show_menu_bar));
                if response.changed() && !state.options.show_menu_bar {
                    state.options.visual_debug_menu = false;
                }
            }
            Check::VisualDebug => {
                ui.add_enabled(state.toggles_enabled && state.options.show_menu_bar, egui::Checkbox::without_text(&mut state.options.visual_debug_menu));
            }
            Check::TopMenu(top) => {
                let mut visible = !state.options.hidden_menus.iter().any(|hidden| hidden == top);
                if ui.add_enabled(state.toggles_enabled && state.options.show_menu_bar, egui::Checkbox::without_text(&mut visible)).changed() {
                    if visible {
                        state.options.hidden_menus.retain(|hidden| hidden != top);
                    } else if !state.options.hidden_menus.iter().any(|hidden| hidden == top) {
                        state.options.hidden_menus.push((*top).to_owned());
                    }
                }
            }
            Check::Submenu { path, parent } => {
                let parent_hidden = state.options.hidden_menus.iter().any(|hidden| hidden == parent);
                let mut visible = !state.options.hidden_submenus.contains(&path);
                if ui.add_enabled(state.toggles_enabled && state.options.show_menu_bar && !parent_hidden, egui::Checkbox::without_text(&mut visible)).changed()
                {
                    if visible {
                        state.options.hidden_submenus.retain(|hidden| hidden != &path);
                    } else if !state.options.hidden_submenus.contains(&path) {
                        state.options.hidden_submenus.push(path.clone());
                    }
                }
            }
            Check::None => ui.add_space(ui.spacing().icon_width + 2.0),
        }
        let label = ui.add(egui::Label::new(label).sense(Sense::click()).selectable(false).truncate());
        if !detail.is_empty() {
            ui.add(egui::Label::new(RichText::new(detail).small().color(p.text_dim)).truncate());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(bytes) = bytes {
                ui.label(RichText::new(plan::human_bytes(bytes)).small().color(p.text_dim));
            }
        });
        label
    });
    if selected {
        ui.painter().rect_filled(inner.response.rect, 2.0, p.accent.gamma_multiply(0.18));
    }
    crate::help::target(ui, inner.response.rect, topic);
    (toggled, inner.inner)
}

fn show_node(ui: &mut egui::Ui, p: &Palette, node: &Node, depth: usize, state: &mut TreeState<'_>) {
    let has_children = !node.children.is_empty() || node.browse.is_some();
    let open = has_children && (state.expanded.contains(&node.id) || !state.filter.is_empty() && !node.children.is_empty());
    let dim = node.sign == '-';
    let mut label = RichText::new(&node.label).color(if dim { p.text_dim } else { p.text });
    if node.id.starts_with("g:") {
        label = label.strong();
    } else if node.id.starts_with("c:") {
        label = label.monospace();
    }
    let selected = node.file.is_some() && *state.selected == node.file;
    let (toggled, response) =
        row(ui, p, depth, node.sign, has_children, open, node.check.clone(), label, &node.detail, node.bytes, selected, topic_for(&node.id), state);
    let response = if node.note.is_empty() { response } else { response.on_hover_text(&node.note) };
    if toggled || (response.clicked() && has_children && node.file.is_none()) {
        if open {
            state.expanded.remove(&node.id);
        } else {
            state.expanded.insert(node.id.clone());
        }
    }
    if response.clicked()
        && let Some(file) = &node.file
    {
        *state.selected = Some(file.clone());
    }
    if !open {
        return;
    }
    let filter = state.filter.clone();
    for child in node.children.iter().filter(|c| c.matches(&filter)) {
        show_node(ui, p, child, depth + 1, state);
    }
    if let Some(folder) = &node.browse {
        show_folder(ui, p, folder, node.sign, depth + 1, state);
    }
}

fn show_folder(ui: &mut egui::Ui, p: &Palette, folder: &str, sign: char, depth: usize, state: &mut TreeState<'_>) {
    if depth > plan::MAX_DEPTH {
        return;
    }
    let Some(root) = state.root else { return };
    let entries = match state.listings.get(root, folder) {
        Ok(entries) => entries.clone(),
        Err(error) => {
            ui.label(RichText::new(error).small().color(p.removed));
            return;
        }
    };
    for (name, is_dir, bytes) in entries.iter().take(MAX_LISTED) {
        let path = if folder.is_empty() { name.clone() } else { format!("{folder}/{name}") };
        let id = format!("fs:{path}");
        let file_sign = if state.edited.contains(&path) { '~' } else { sign };
        if *is_dir {
            let open = state.expanded.contains(&id);
            let label = RichText::new(format!("{name}/")).monospace().color(if sign == '-' { p.text_dim } else { p.text });
            let (toggled, response) = row(ui, p, depth, file_sign, true, open, Check::None, label, "", None, false, "review", state);
            if toggled || response.clicked() {
                if open {
                    state.expanded.remove(&id);
                } else {
                    state.expanded.insert(id.clone());
                }
            }
            if open {
                show_folder(ui, p, &path, sign, depth + 1, state);
            }
        } else {
            let selected = state.selected.as_deref() == Some(path.as_str());
            let label = RichText::new(name).monospace().color(if sign == '-' { p.text_dim } else { p.text });
            let (_, response) = row(ui, p, depth, file_sign, false, false, Check::None, label, "", Some(*bytes), selected, "review", state);
            if response.on_hover_text("Show in Source review").clicked() {
                *state.selected = Some(path);
            }
        }
    }
    if entries.len() > MAX_LISTED {
        ui.label(RichText::new(format!("… {} more", entries.len() - MAX_LISTED)).small().color(p.text_dim));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{Inventory, Profile, ProjectSpec};

    fn plan_for(options: Options) -> Result<Plan, String> {
        let root = crate::source::detect("").ok_or("no PhotoCraft source")?;
        let inventory: Inventory = plan::scan(&root)?;
        let spec = ProjectSpec {
            source: root,
            name: "tree-check".into(),
            destination: std::env::temp_dir().join("projectile-tree-check-missing"),
            profile: Profile::Base,
            options,
        };
        Ok(plan::build(&spec, &inventory))
    }

    fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
        nodes.iter().find_map(|n| if n.id == id { Some(n) } else { find(&n.children, id) })
    }

    #[test]
    fn every_component_appears_once_and_toggles_change_signs() -> Result<(), String> {
        let plan = plan_for(Options::default())?;
        let tree = build(&plan, &Options::default(), true);
        for c in &plan.components {
            if c.status == Status::Added && c.path != plan::BACKUP_SOURCE_DIR {
                continue;
            }
            if find(&tree, &format!("c:{}", c.path)).is_none() {
                return Err(format!("{} is missing from the tree", c.path));
            }
        }
        if find(&tree, "g:mcp").map(|n| n.sign) != Some('-') || find(&tree, "c:apps/photocraft").map(|n| n.sign) != Some('~') {
            return Err("MCP should be '-' when off and the editor app '~'".into());
        }
        let with_mcp = Options { mcp: true, ..Options::default() };
        let tree = build(&plan_for(with_mcp.clone())?, &with_mcp, true);
        if find(&tree, "g:mcp").map(|n| n.sign) != Some('+') || find(&tree, ".mcp.json").is_none() {
            return Err("MCP group should be '+' with .mcp.json when on".into());
        }
        Ok(())
    }

    #[test]
    fn menu_settings_follow_the_component_tree_hierarchy() -> Result<(), String> {
        let options = Options { show_menu_bar: true, hidden_menus: vec!["File".into()], hidden_submenus: vec!["Edit > Undo".into()], ..Options::default() };
        let menu = menu_node(&options, Ok(vec![vec!["Edit".into(), "Undo".into()]]));
        if menu.id != "g:menu-bar" || menu.children.first().map(|node| node.id.as_str()) != Some("g:menu-debug") {
            return Err("menu settings should be one tree row with Visual Debug as its first child".into());
        }
        let file = find(&menu.children, "g:menu-top:File").ok_or("File menu is missing")?;
        if file.sign != '-' || !matches!(file.check, Check::TopMenu("File")) {
            return Err("hidden top-level menu should be represented by an unchecked tree node".into());
        }
        let undo = find(&menu.children, "g:menu-sub:Edit > Undo").ok_or("nested Undo submenu is missing")?;
        if undo.sign != '-' || !matches!(&undo.check, Check::Submenu { path, parent } if path == "Edit > Undo" && *parent == "Edit") {
            return Err("hidden submenu should be nested beneath its top-level menu".into());
        }
        Ok(())
    }
}
