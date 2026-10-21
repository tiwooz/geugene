# ProjectIle and generated PhotoCore projects

**Status:** PhotoCraft-derived Base source snapshots implemented on 2026-10-11. On 2026-10-12
ProjectIle became a separate app in `projectile/` (its own Cargo workspace; the source folder is
chosen at run time), and the portrait redesign (plan preview, `+`/`-` summary, source review, backup/VS Code/MCP
options, background generation, ease of access, `projectile --mcp`) landed on 2026-10-12. Full
unrestricted snapshots and Minimal feature selection remain previews. This document records the
architecture and current limitations.

**Revision history:** 2026-10-11 first Base generator in `apps/projectile` · 2026-10-12 redesign;
moved out of the PhotoCraft workspace into standalone `projectile/`; header removed for a status
bar, installer-style Components tree, Rust toolchains tab, ease of access only in the tray; Viewer profile, Android scaffold,
Git/GitHub, build & run, Rust docs download, system detection (this revision).
The user-facing feature definitions live in [`projectile/README.md`](../projectile/README.md).

## Product boundaries

Keep three things separate:

1. **ProjectIle** is the generator, a separate app in `projectile/` with its own Cargo workspace,
   version and lockfile. PhotoCraft does not build, depend on or launch it; it reads a PhotoCraft
   checkout chosen at run time (validated, remembered, overridable with `PROJECTILE_SOURCE`) and
   never builds or launches PhotoCraft. It owns the project wizard, source/version selection,
   dependency and component previews, Rust/toolchain checks, generation logs, and its own
   preferences.
2. **Generated project** is a copied source workspace with its own source tree and generator
   metadata. It must build without a path back into the PhotoCraft checkout or ProjectIle's build
   tree. The current Base snapshot omits `.git`, build outputs, downloaded corpora, local plans,
   ProjectIle, xtask, and the CLI/browser app entry points.
3. **Generated application** is what the user builds from that project. Its enabled editor
   features, launch arguments, appearance defaults, and update behavior are distinct from
   ProjectIle's own window settings.

The source is the existing PhotoCraft workspace, not an empty template:
[`../AGENTS.md`](../AGENTS.md) describes its product and workspace rules, and
[`architecture.md`](architecture.md) describes its current crate boundaries. Do not mass-rename
PhotoCraft source files or strings to make a derivative. Full should preserve the selected
upstream source snapshot; a generated project's own name/identity should be supplied through
explicit project metadata and a reviewed branding layer.

## Source profiles

| Profile | Intended result | Safe implementation |
|---|---|---|
| **Full** | A complete PhotoCraft source snapshot in a new project directory. | Copy/archive an exact upstream commit, record its tag and commit, and keep upstream files intact. The user-selected project name is metadata, not a blanket text replacement. |
| **Minimal** | A buildable editor limited to painting, layers, brushes, colour changes, file actions, and About. | Still requires a curated capability manifest and optional dependency closures; not yet safe to build by deleting files or omitting workspace members. |
| **Base** | PhotoCraft's original native UI and editor source, with Brush, Paint Bucket, Eraser, basic document/file operations, undo/redo, colour selection, and essential navigation. | Snapshot the local source, omit alternate CLI/browser app entry points, and enable `base-profile` by default in the generated desktop app. The engine session enforces an explicit command allowlist; the UI limits its toolbar and menus. This is a learning profile, not a security sandbox: the editor source and dependency graph are retained. |

Base preserves the PhotoCraft product identity in the application. The chosen project name is
recorded in `.projectile.json` and the generated `PROJECTILE.md`; source identifiers and branding
are not mass-renamed. ProjectIle applies the selected window size and scale to the copied app.

### Why Minimal is not currently a Cargo switch

Some useful low-level crates are already reasonably focused:

- `geom`, `color`, `cms`, and `raster` provide geometry, colour data/conversion, colour
  management, and pixel/tile storage.
- `doc` supplies the document and layer data model.
- `paint` contains brush/paint behavior; `ops` contains history/operations; `algo` contains image
  algorithms.
- `compose` renders documents on the CPU; `gpu` provides the wgpu renderer; `vector` and `text`
  support vector and type content.
- `psd`, `codecs`, `raw`, and `affinity` decode or encode formats; `format` handles native
  `.pcraft` files; `io` coordinates document import/export.
- `engine` owns commands and dispatch; `ui-egui` provides the editor UI; `automation` provides
  MCP/JSON-RPC interfaces; `plugins` hosts sandboxed Wasm plug-ins.

Those roles do **not** imply that each feature can currently be selected independently. In the
current manifests, `engine` has required dependencies on broad operation, text, vector, I/O,
format, and plug-in crates. `ui-egui` also directly depends on the engine, GPU, text, vector, and
plug-in crates. `compose` requires `vector`. The desktop app enables the broad UI and has only a
small app-level `heif` feature; `heif` itself is optional in `codecs`. Therefore a Minimal project
cannot be made reliable simply by choosing a subset of `crates/*`.

Before shipping Minimal, introduce explicit capability features/registries through the engine and
UI, make their dependencies optional only where the code can genuinely compile without them, and
add profile-specific build, command-catalog, UI, and output tests. Keep the selected source
allowlist generated from reviewed metadata, not hand-maintained shell deletion rules. The first
capability closure to prove should include the layer/document model, paint/brush path, colour
handling, composition, and the chosen file formats.

## Generation pipeline

Represent a wizard submission as a validated `ProjectSpec`, containing at least:

- a filesystem-safe project name and destination;
- source release/tag and immutable commit (Full/Minimal), or template revision (Base);
- profile and explicitly selected capabilities;
- required Rust toolchain and target platforms;
- generated-app startup defaults and any validated argument preset;
- whether to retain a separate source backup.

Use a staged pipeline:

1. Validate names, destination permissions, profile compatibility, and available disk space.
2. Resolve the selected source ref to a commit. Present the resolved commit before generation.
3. Acquire the source into a ProjectIle-owned cache; do not modify the user's existing checkout.
4. For Base, copy the current local workspace into a newly reserved output directory; for future
   remote profiles, materialize the selected immutable source snapshot.
5. Generate profile metadata, apply Base startup settings, and write an educational
   `PROJECTILE.md` source overview.
6. Run metadata checks and the selected profile's build/test gates; report every failure in the
   log viewer without labeling an incomplete generation successful.
7. On failure, remove the incomplete output and report cleanup errors; never overwrite an existing
   project silently. If backup is enabled, make it an explicit, separate copy.

Record the source revision, selected profile/options, generator version, resolved dependencies, and
validation results in generated metadata. This makes source review, later rebuilds, updates, and
bug reports reproducible.

## ProjectIle UI and platform behavior

Keep the initial window centered on the project workflow: project name, destination picker,
profile, source version, feature checklist, generated-directory preview, source/dependency review,
Rust status, and a visible generation log. The broader requested window size, locale, clock,
keyboard-layout indicator, screenshot, tray/extra menu, tips, and About information belong to
ProjectIle, not to the generated editor.

### Generator UI references

The initial ProjectIle layout was informed by the CMake GUI and Steinberg's VST3 SDK Project
Generator, inspected on 2026-10-11:

- CMake's [user interaction guide](https://cmake.org/cmake/help/latest/guide/user-interaction/index.html)
  separates source/build locations, Configure, cache-variable review, and Generate. Its
  [Qt dialog definition](https://github.com/Kitware/CMake/blob/d852457f0bb953a730da52350cab0a12566f7209/Source/QtDialog/CMakeSetupDialog.ui)
  shows the grouped/advanced options and output pane.
- Steinberg's [Project Generator documentation](https://steinbergmedia.github.io/vst3_dev_portal/pages/What+is+the+VST+3+SDK/Project+Generator.html)
  describes Welcome, Create Project, and Preferences pages; explicit project identity/location;
  common and advanced options; and inspectable script output with a copy action.

ProjectIle follows those observed workflows with explicit name/destination fields, separate
Advanced/About pages, a visible output-file preview, and a copyable generation log. These are
interaction patterns only: no proprietary code or artwork is reused. Full will need a source/ref
selector as well as its destination; Base snapshots the local checkout chosen in Create › Source.

### Implemented ProjectIle controls (2026-10-12)

Code map (in `projectile/src/`): `source.rs` (find/validate the PhotoCraft checkout, its version and
fonts), `plan.rs` (source inventory, profiles, options, components, `+`/`-` summary, file edits
and their diff preview), `generator.rs` (executes a plan on a worker thread), `pages.rs` (tabs),
`theme.rs` (PhotoCraft palettes), `access.rs` (ease of access), `mcp.rs` (stdio MCP server).

- **Window.** Portrait by default (720 × 960, minimum 560 × 640), presets Portrait compact,
  Portrait, Portrait tall and Landscape. eframe persists size and position; ProjectIle no longer
  sends its own startup resize (the old window jumped from the default to the saved size on every
  launch, and the saved size never followed manual resizes). Upgraders get the portrait size once
  (`layout_version`), and the window is shrunk to fit 92% of a smaller monitor.
- **Layout.** No menu bar (removed 2026-10-12; every action is on a tab or button, shortcuts
  kept), underlined tabs (Welcome, Create Project, Rust, Preferences, About), and a CMake-style bottom pane on Create:
  Configure / Generate / Open Folder / VS Code, progress and the colour-coded output log. Status
  bar: Rust version, PhotoCraft source version, profile, latest notice, ProjectIle version and the
  ease-of-access tray icon. (The earlier title/chip header was removed as redundant.)
- **Themes.** PhotoCraft's own palettes: Pro Dark, Pro Medium (Photoshop brightnesses, blue
  accent), Studio and Studio Light (violet). Ctrl+= / Ctrl+− / Ctrl+0 scale ProjectIle and are
  saved (egui's built-in keyboard zoom, which bypassed the saved scale, is off).
- **Create page.** Form (source checkout and revision, name, location, profile, resulting folder,
  when done); a Summary that compares the profile with full PhotoCraft (`+` enacted, `-` won't be
  used, `~` changed) and lists what goes into the project; the Components tree (below); and a
  Source review box showing every patch as a `git diff`, or a picked file. The same `FileEdit`s are applied by the generator, so the preview cannot drift; a
  patch whose anchor text vanished from the source blocks generation up front.
- **Options.** Original source backup (`upstream/photocraft`, untouched, excluded from the
  workspace and rust-analyzer), PhotoCraft `docs/`, VS Code setup (`.vscode/` extensions, settings,
  tasks), `git init` with a first commit (a failed commit is a warning), and Claude Code MCP: keeps
  `photocraft-cli` and writes `.mcp.json` for the bridge to the running editor
  (`--control <port> --control-token-file .projectile/control.token`; the editor creates the token,
  which is gitignored). The bridge goes through `PhotocraftApp::run`, so the Base allowlist holds;
  the CLI's *headless* mode is not restricted, and the plan says so. After generating: open the
  folder and/or VS Code (`code`, `codium` or `code-oss` on PATH).
- **Generation** runs on a worker thread with progress; failures remove the incomplete folder.
  Local-only folders (`corpus`, `plan`, `log`, `refs`, …) are skipped only at the top level, so a
  nested `src/log/` module is still copied.
- **Components tree** replaces the table: installer-style rows with `+`/`~`/`·`/`-` signs, checkboxes
  for the file-producing options, and lazy browsing into real source files shown in Source review.
- **Rust tab** (`rust.rs`): active toolchain vs PhotoCraft's `rust-version`; installed toolchains,
  stable releases (from the fixed six-week schedule) and channels; confirmed rustup install, default,
  uninstall, update and rustup self-install (user-only, no sudo); *Pin* writes `rust-toolchain.toml`.
- **Ease of access** (only the status-bar tray icon, or Ctrl+Alt+U): Magnifier, High contrast,
  Larger text, Bigger click targets, Reduce motion, Strong focus outline and Beginner explanations
  (plain-language help under every field). AccessKit is enabled for screen readers.
- **Agents.** `projectile --mcp` serves `projectile_status`, `projectile_plan` and
  `projectile_generate` over stdio:
  `claude mcp add projectile -- <path to projectile> --mcp` (About shows the exact line); tools
  take an optional `source`.

Not yet: tray, keyboard-layout indicator, localization, automatic updates, Full and Minimal
generation. Screenshots (Preferences › Window, or F12) go to `Pictures/ProjectIle/Screenshots`.

Use the existing native Rust GUI stack as prior art: [`apps/photocraft/Cargo.toml`](../apps/photocraft/Cargo.toml)
uses eframe/egui on wgpu (not a webview). ProjectIle reuses that ecosystem but does not depend on
the editor's UI crate to render its own forms.

### Viewer profile and other 2026-10-12 additions

- **Viewer profile.** PhotoCraft gained `CapabilityProfile::PhotoCoreViewer` in `crates/engine`
  (`use_photocore_viewer_profile`, allowlist: `help.about`, `file.open`, view zoom/fit;
  `Session::menu_visible` limits menus to About), UI trimming in `crates/ui-egui` (no toolbar,
  options bar, dock or status bar; Hand tool; no Home screen) and the `viewer-profile` feature in
  `apps/photocraft`. ProjectIle's Viewer profile turns that feature on, copies the chosen image to
  `assets/viewer/startup.<ext>` and patches `main.rs` to open it when no file is given. Tests:
  `photocore_viewer_profile_allows_only_viewing` (engine), `photocore_viewer_menu_has_only_about`
  (ui-egui).
- **Android (experimental).** `projectile/src/android.rs` writes a Gradle (Java, AGP 9.4.1)
  project with a `NativeActivity` subclass and an `apps/photocraft-android` `cdylib`; API levels
  default to minSdk 26 / targetSdk 36 (Google Play's requirement since 2026-08-31). The Gradle
  side was built with `gradle assembleDebug`; PhotoCraft itself has not been built for Android.
- **Commands** (`command.rs`): rustup, the docs download, git, gh and cargo build run as confirmed,
  streamed tasks; `git.rs` commits and publishes with the GitHub CLI.
- **System** (`system.rs`): OS/distro/desktop/package-manager detection drives the status-bar
  indicator, install hints and the dialog layout.

### Rust/toolchain handling

- Detect `rustc`, `cargo`, and `rustup` independently and display their paths and versions. A
  missing `rustup` does not necessarily mean Rust is absent.
- Compare the detected compiler with the generated source's `rust-version` and selected release's
  toolchain file. A version list must show which entries meet that requirement.
- Never run `sudo`, a shell-piped installer, or an installation without explicit user confirmation.
  Prefer the official rustup flow; show the exact command/link and explain when administrator
  action is required.
- On Arch Linux, the documented native development libraries are `libxkbcommon`, `wayland`,
  `libx11`, `libxrandr`, `libxi`, `mesa`, and `gtk3` (see
  [`development.md`](development.md)). wgpu uses the available graphics backend/driver at runtime;
  an AMD x64 machine does not need an AMD-only Cargo dependency. Show adapter/backend diagnostics
  and retain the application's existing safe fallback behavior.

### Advanced options, caches, and updates

Model argument presets as typed, versioned settings, not arbitrary unvalidated command strings.
Validate every flag against the selected generated-app version; clearly separate options that
control ProjectIle's own window from options written into the generated app's startup config.

Give cache clean/rebuild operations narrow ownership: clear only ProjectIle's named source/build
cache, or invoke a rebuild/clean for the selected generated project. Never erase Cargo's global
registry or Git cache as a convenience action. Auto-update must be opt-in, resolve and display the
new source revision, and never silently rewrite an existing project.

## Dependency map and audit

The editor's dependency direction is deliberately layered: foundation and document crates feed
operations/compositing/I/O, which feed the engine and UI, and the desktop app wires those layers
together. The layer rules are checked by `cargo xtask layers`. ProjectIle should consume source
metadata to present this graph; it should not maintain a second manually copied dependency list.

Important native desktop dependencies in the current app:

| Dependency/group | What it does | Relevance to ProjectIle |
|---|---|---|
| `eframe`, `egui` | Native immediate-mode UI and window shell; eframe uses wgpu here. | Suitable prior art for a Rust-only native wizard. Versions are aligned at workspace version 0.36. |
| `rfd` | Native file/directory dialogs. | Useful for destination and source-backup selection. |
| `arboard` | Clipboard integration. | Relevant to copying screenshots/log excerpts; platform behavior must be checked. |
| `winit`, `x11rb`, `zbus`, `async-io`, `futures-lite` | Linux window/event and portal/X11 integrations used by the desktop application. | Linux platform integration, not PhotoCore image features. |
| `photocraft-engine`, `photocraft-ui-egui`, `photocraft-io`, `photocraft-format` | Editor behavior, UI, file orchestration, and native project format. | Included in Base snapshots with the rest of the workspace; their dependencies are not yet feature-pruned. |
| `photocraft-tablet` | Isolated pen/tablet input helper, including platform-specific integration. | Not a generator dependency; only include it in editor outputs that need tablet input. |

The standalone CLI is a useful example of the separation: it depends on engine/I/O/format/codecs
but not on the desktop UI. [`apps/photocraft-cli/Cargo.toml`](../apps/photocraft-cli/Cargo.toml)
shows that headless dependency boundary.

### Audit result on 2026-10-11

`cargo-machete` 0.9.2 identified six direct dependency declarations with no source use. Their
declarations were removed in this change:

- `photocraft` → `photocraft-raster`;
- `photocraft-automation` → `photocraft-geom`;
- `photocraft-testkit` → `photocraft-color`, `photocraft-geom`, and `photocraft-raster`;
- `photocraft-raster` → `serde`.

The workspace lockfile contains multiple versions of several transitive crates (for example
`thiserror`, `png`, `zune-jpeg`, `getrandom`, and platform-specific `objc2` crates). This is not
itself a conflict: Cargo has resolved them together, and the native editor aligns its direct
eframe/egui versions. Treat duplicates as candidates for investigation only when they cross an
important ABI boundary, materially increase output size, or prevent a selected profile from
building. The audit found unused declarations; it does not prove that every optional profile is
buildable or that no missing feature gate exists.

## Suggested implementation order

1. **Done:** standalone native ProjectIle app and Base-profile wizard, with persisted settings,
   profile/file previews, toolchain status, live window theme/scale/size/fullscreen controls,
   copyable logs, screenshots, About links, and output validation tests.
2. **Done:** snapshot PhotoCraft locally for Base; retain its original UI, apply the runtime
   command/UI allowlist, embed startup dimensions/scale, document the source, validate input,
   preserve existing destinations, and test the generator.
3. **Partly done:** read-only source inventory (top-level components, sizes, revision) and plan
   preview. Still to do: the crate dependency graph from `cargo metadata`.
4. Add Full as an unrestricted source snapshot with immutable remote source selection and
   source-revision recording.
5. Modularize PhotoCraft dependencies and prove an actually-pruned Minimal closure in CI.
6. Add convenience surfaces (toolchain installation, presets, backup, cache actions, preview,
   localization, screenshots, tips, and update flow) behind tested operations.

Run the generator with `cd projectile && cargo run`. Base snapshots the
current local workspace; disabled Full and Minimal cards are previews, not generation options.

Do not advertise Minimal as supported until its requested features are both present and excluded
features are demonstrably absent from the generated build and UI.
