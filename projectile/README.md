# ProjectIle

ProjectIle is a **standalone** native Rust app (egui/eframe on wgpu, no webview, no `unsafe`) that
generates PhotoCore projects: your own buildable copy of PhotoCraft (by ArtCraft), trimmed to a
profile, for learning, experiments, forks and mods.

It is **not part of PhotoCraft**. It has its own Cargo workspace, version and lockfile; PhotoCraft
does not build, depend on, or launch it, and ProjectIle never builds or launches PhotoCraft. It
only **reads** a PhotoCraft source folder you choose and **writes** a new project folder.

```sh
cargo projectile              # from the PhotoCraft repo root (alias in .cargo/config.toml)
cd projectile && cargo run    # the same, from inside projectile/
cargo projectile --mcp        # MCP server on stdio (for Claude Code)
cargo install --path projectile # then just `projectile`
```

`cargo run -p projectile` does **not** work from the PhotoCraft root (ProjectIle is not in that
workspace), and `cargo run <anything>` there starts PhotoCraft, not ProjectIle.

Set `PROJECTILE_SOURCE=/path/to/photocraft` to preselect the source folder.

## Features

Every feature below is defined by what it does, where it lives and what it touches. "Project" means
the folder ProjectIle generates; "ProjectIle" means this app.

### Inputs

| Feature | Definition | Where | Notes |
|---|---|---|---|
| **Source** | The PhotoCraft checkout to copy from. Valid when its root `Cargo.toml` is a workspace and it has `apps/photocraft`, `crates/engine` and `crates/ui-egui`. | Create › Source (text + `…`), MCP `source` | Detected on start: saved choice → `$PROJECTILE_SOURCE` → folders around the executable and working directory (and sibling `photocraft`/`photocore`) → the folder ProjectIle was compiled next to. Read-only. |
| **Name** | Folder and package name of the project: an ASCII letter, then letters, digits, `-` or `_`, at most 64 characters; Windows device names rejected. Lower-cased, `_` → `-`. | Create › Name | |
| **Location** | Parent folder; the project is created inside it. Must be outside the source. | Create › Location | |
| **When the folder exists** | Under Creates, choose: *Stop* (default; pick another name), *Keep the old folder* (renamed to `<name>-old-<YYYYMMDD>-<HHMMSS>`, restored if generation fails), *Replace files inside it* (same-named files overwritten, other files kept; never deleted on failure), or *Delete it and make it anew* (asks for confirmation with the file count). Replace and Delete are refused for the root, its direct children, the home folder and anything containing it or the PhotoCraft source. MCP accepts `on_existing`: Stop, RenameAside or Merge; Delete is window-only. | under Creates | Stop |
| **Profile** | What the project's editor can do. **Base** (ready): PhotoCraft's UI with Brush, Eraser, Paint Bucket, Fill, file basics, undo/redo, colours, navigation, enforced by the engine's command allowlist. **Viewer** (ready): opens one chosen image at startup, skipping the Home screen; no tools, no editing, no panels, only About in the menus (PhotoCraft's `viewer-profile` feature). **Learning demos** (ready): adds a small PhotoCraft UI host and `LEARNING.md`, a crate-by-crate guided demo index. **Full** and **Minimal**: previews only. | Create › Profile | Profiles are capability restrictions, not security sandboxes. |
| **Image** (Viewer) | The picture the viewer shows; copied into the project as `assets/viewer/startup.<ext>` and opened when no file is given. PNG, JPEG, WebP, GIF, BMP, TIFF, PSD/PSB or .pcraft, up to 1 GB. | Create › Image | |

### Options (each changes the project only)

The file-producing options are checkboxes in the **Components** tree (Agent tools, Documentation,
Original source backup, VS Code setup, Git repository); the rest are where noted.

| Option | Definition | Default |
|---|---|---|
| **Original source backup** | Copies the untouched source into `backup/` (a complete source folder inside the project, plus a short `ORIGINAL-SOURCE.md` note), adds `exclude = ["backup"]` to the project's workspace and hides it from rust-analyzer. Lets you and agents compare or restore without finding the repository. | on |
| **ProjectIle source snapshot** | Includes the verified ProjectIle source archive that generated the project under `projectile-source-snapshot/`, with an extraction note. Use it to inspect or recover the generator version. | on |
| **PhotoCraft docs/** | Copies `docs/`. Off for a smaller project. | on |
| **VS Code setup** | Writes `.vscode/extensions.json` (rust-analyzer, Even Better TOML), `settings.json` (excludes `target/` and `backup/`) and `tasks.json` (run, check, test the Base allowlist, and run with MCP control when MCP is on). | on |
| **Initialise git** | `git init` + `git add -A` + a first commit, so `git diff` shows what you changed. A failed commit (for example no `user.email`) is a warning; the project is still complete. | on |
| **Claude Code MCP** | Keeps `apps/photocraft-cli`, writes `.mcp.json` registering a `photocore` server that bridges to the *running* editor on the chosen port, creates `.projectile/` and gitignores `.projectile/control.token` (the editor creates the token on first run). Bridge calls go through the Base allowlist; the CLI's headless mode does not, and the plan warns about it. | off |
| **MCP port** | Control port used by `.mcp.json` and the VS Code task, 1024–65535. | 7878 |
| **Generated app startup** | Initial window size (640×480 to 3840×2160) and UI scale (75–200%) written into the project's `apps/photocraft/src/main.rs`. | 1200×800, 100% (Preferences) |
| **When done** | Open the project folder, open it in VS Code (`code`, `codium` or `code-oss`), **Build** (`cargo build -p photocraft` in the project, streamed to the output) and **Run** (build, then start `target/debug/photocraft`). *Generate, build & run* does all of it in one click. | folder on |
| **Android build** | Experimental. Writes `android/` (Gradle, Java: `settings.gradle`, `app/build.gradle`, a `NativeActivity` subclass, manifest, `build-rust.sh`, README) and `apps/photocraft-android` (a Rust `cdylib` with the `android_main` entry point, the project's only unsafe attribute). Choose JDK 17/21/25 and Gradle 9.6.0/9.8.1 (both meet the pinned Android Gradle Plugin minimum); the Rust build includes arm64 phone and x86_64 emulator libraries. The generated guide links to the selected downloads, pins Gradle, and explains how to install an API-level system image, create and launch an emulator, and install the APK. Minimum and target API are Automatic (26 and 36: Google Play requires 36 since 31 Aug 2026) or chosen by you; compileSdk stays at least 36. PhotoCraft itself has not been verified on Android. | Components › Android | off |
| **Main menu bar, tabs and submenus** | A component checkbox in Create › Components includes or removes the whole top menu bar. When included, choose which top-level tabs and catalogue-derived submenus to show, plus optional egui Visual Debug tools. Menu visibility is presentation-only, not a command/security restriction. | Create › Components | on |
| **Rust toolchain pin** | Writes `rust-toolchain.toml` (`channel` + rustfmt, clippy) so rustup uses that version inside the project. Accepts `stable`, `beta`, `nightly` or `1.N[.N]`. Set with *Pin* on the Rust tab; blocked if the source already has the file. | none |

### Review before generating

| Feature | Definition |
|---|---|
| **Configure** (F5) | Rescans the source and Rust/VS Code/git, rebuilds the plan and reports problems, like CMake's Configure. The plan also refreshes on every input change. |
| **Summary** | Two `git`-style lists. *Compared with full PhotoCraft*: capabilities the profile adds (`+`) or switches off (`-`). *What goes into the project*: crates folded per status, then apps, folders and generated files: `+` enacted/added, `~` enacted with ProjectIle edits, `·` compiled but dormant, `-` won't be used. Problems that block generation are listed first in red. |
| **Components** | An installer-style tree. Each row: sign (`+` enacted/added, `~` edited, `·` compiled but dormant, `-` won't be used), expand arrow, checkbox, name, role, size. Groups: Editor application, Crates (by architecture layer), Agent tools, Documentation, Original source backup, VS Code setup, Git repository, Rust toolchain pin, Workspace files, Not included. Greyed checkboxes are decided by the profile; live ones are the options, and unticking one turns its branch into `-`. Components expand into their real folders and files (up to 400 entries per folder); clicking a file shows it in Source review. Search, Expand all, Collapse all. |
| **Source review** | Every edit ProjectIle will make, as a diff (patches, appended lines, new files), or the file picked in the tree with line numbers (512 KB cap; binaries described, not shown). *Copy* copies it. The generator applies exactly these edits; if PhotoCraft changed so an edit no longer applies, generation is blocked up front. |

### Generating

| Feature | Definition |
|---|---|
| **Generate** (Ctrl+G) | Enabled only when the plan has no blockers. Runs on a worker thread with a progress bar: copy source (skipping `.git`, `target`, and top-level `projectile`, `corpus`, `plan`, `log`, `refs`, `temp`, `dist`, `secrets`, `craft-fonts`; symlinks are refused), apply edits, write `.projectile.json` (profile, options, PhotoCraft version and revision, component statuses), copy the backup, run git. On failure the incomplete folder is removed. |
| **Regenerate** | Rebuilds a project ProjectIle made from the current source, with the name, folder, profile and options saved in its `.projectile.json` (or the current Create settings, if you untick that), refreshing `backup/` too. You choose what happens to the current folder: rename it aside (default), replace files inside it, or delete it (confirmed). *Regenerate…* sits next to Generate; the Creates hint offers it when the folder is a ProjectIle project; MCP: `projectile_regenerate {project, on_existing?}` (RenameAside or Merge). A Viewer whose original picture moved reuses the copy inside the project. |
| **Output** | Colour-coded log of everything ProjectIle did, at most 2000 lines. *Copy Output to Clipboard* (Ctrl+Shift+C), *Clear*. |
| **Open Folder / VS Code** | Open the last generated project in the file manager or VS Code. |

### Git and GitHub

| Feature | Definition |
|---|---|
| **Status** | Branch, uncommitted changes and remote of the last generated project (or the existing Creates folder). |
| **Commit** | `git init` if needed, `git add -A`, `git commit -m <message>`, shown and confirmed first; the message is passed as an argument, never through a shell. Local only. |
| **Publish / Push** | With the GitHub CLI signed in: `gh repo create <name> --private|--public --source . --remote origin --push`, confirmed first (outward-facing). Afterwards *Push* runs `git push -u origin HEAD`. Without `gh`, ProjectIle shows how to install it with your package manager and how to sign in (`gh auth login --web`). |

### Rust tab

| Feature | Definition |
|---|---|
| **Active toolchain** | The version `rustc` runs now, where it comes from (rustup or a system package), and whether it meets the source's `rust-version` (PhotoCraft needs 1.95). *Refresh*; *Update all* (`rustup update`). |
| **Installed / Releases / Channels** | Launcher-style rows: a version badge, title, status pills (default, installed, active, pinned) and actions. *Installed* lists `rustup toolchain list` (or the system compiler without rustup). *Releases* lists stable 1.70 up to the latest, with dates derived from Rust's fixed six-week schedule (no network needed) and a note when a version is too old for PhotoCraft. *Channels*: stable, beta, nightly. Search box. |
| **Actions** | *Install* (`rustup toolchain install X --profile default`), *Use by default* (`rustup default X`), *Uninstall* (⋯ menu), *Pin* (for the next generated project). Every rustup command is shown in a confirmation dialog first, runs on a worker thread, and streams its output to *rustup output*. Names are validated before reaching rustup. |
| **Rust docs** | *Install Rust docs…*: with rustup, `rustup component add rust-docs`; without it, ProjectIle downloads the official `rust-docs-<version>-<host>.tar.xz` (about 25 MB) for your compiler from static.rust-lang.org and unpacks it into `~/.local/share/projectile/rust-docs` (no root). The offline book link appears once `index.html` exists. |
| **Without rustup** | Explains that switching needs rustup. *Install rustup…* runs the official installer (`curl … sh.rustup.rs | sh -s -- -y --no-modify-path`) for your user only after confirmation; it never uses sudo or edits shell profiles. A package-manager alternative is shown as text (on Arch: `sudo pacman -S rustup`) for you to run yourself. rustup in `~/.cargo/bin` is found even when it is not on PATH. |

### ProjectIle itself

| Feature | Definition |
|---|---|
| **Window** | Portrait 720×960 by default (minimum 560×640); presets Portrait compact, Portrait, Portrait tall, Landscape; shrunk to fit 92% of a smaller screen. Size and position are remembered by eframe. Maximize, full screen (F11). |
| **Themes** | Pro Dark, Pro Medium (Photoshop-like charcoal, blue accent), Studio, Studio Light (violet), matching PhotoCraft's palettes. Fonts: Inter and JetBrains Mono read from the chosen source's `assets/fonts` when present, otherwise egui's built-in fonts (ProjectIle bundles no font files). |
| **Scale** | 80–160%: pick it in Preferences and press **Apply** (or Ctrl+= / Ctrl+− / Ctrl+0), saved. |
| **System** | Detects the OS, Linux distribution, desktop and package manager (shown in the status bar, e.g. *Arch Linux · Hyprland*, and in Preferences › System). Install hints use your package manager (`sudo pacman -S …` on Arch); ProjectIle never runs sudo. **Layout** (Automatic or overridden): dialogs put the main action last on GNOME/macOS/most Linux desktops and first on KDE/Windows; tiling window managers get compact spacing. |
| **Resizable lists** | Components, Source review, the Rust list and rustup output have a grip underneath: drag to resize, double-click to reset; sizes are saved. |
| **Fonts** | Inter Regular (text), Inter Medium (buttons, tabs), Inter SemiBold (headings) and JetBrains Mono (code), read from the PhotoCraft source's `assets/fonts`. |
| **Community** | Welcome › Join the community: the official ArtCraft Discord (`discord.gg/artcraft`), the Claude Console (`platform.claude.com`) and the source on GitHub. |
| **Language and clock** | Top right, as two equal-height chips: the interface language (English only for now, saved) and the local time (closest to the edge; hover for the full date). |
| **Layout** | Underlined tabs at the top (Welcome, Create Project, Rust, Preferences, About), the page, and a status bar: Rust version (opens the Rust tab), PhotoCraft source version (opens Create), profile, the latest notice, ProjectIle's version and the ease-of-access tray icon. On Create, the Configure/Generate bar and output sit above the status bar. |
| **Ease of access** | Only in the tray: the icon at the bottom right of the status bar (or Ctrl+Alt+U). Not on any tab or menu. Magnifier (+25%), High contrast, Larger text (+20%), Bigger click targets, Reduce motion, Strong focus outline, Beginner explanations (plain-language help under every field). Screen readers via AccessKit. Saved. |
| **No menu bar** | Every action is on a tab or button. Shortcuts: Ctrl+1–6 tabs, Ctrl+G Generate, F5 Configure, Ctrl+N Create, F1 Help, Ctrl+= / Ctrl+− / Ctrl+0 scale, F11 full screen, F12 screenshot, Ctrl+Shift+C copy output, Ctrl+Alt+U ease-of-access tray, Ctrl+Q quit. |
| **Screenshot** | Preferences › Window › *Take screenshot* (or F12) saves a PNG to `Pictures/ProjectIle/Screenshots` (or `$XDG_PICTURES_DIR/ProjectIle/Screenshots`) and copies it to the clipboard when the desktop allows. |
| **Tips** | One per launch, rotating; *Next tip* on Welcome. |
| **Settings** | Saved by eframe under the app id `ai.storytold.projectile`; settings from the first ProjectIle release are migrated, out-of-range values clamped. |

### Help

| Feature | Definition |
|---|---|
| **Help tab** | A guide to making a project and a glossary of everything in ProjectIle, in seven sections (Making a project, Reading the plan, Project options, Rust and tools, Concepts, Using ProjectIle, When something goes wrong), with search. F1 opens it. *Concepts* explains 28 ideas (source code, fork, crate, toolchain, version control, diff, MCP, open source…), each with its Wikipedia article. |
| **Links** | Every link opens in the system browser through ProjectIle itself (the `open` crate); only http(s) links are followed. |
| **Middle-click for help** | Middle-click (press the mouse wheel on) a field, card, button, tree row, tab or status item to open its Help topic. The smallest registered area under the pointer wins. |
| **Why?** | Next to steps, bullets and form hints. Unfolds the short answer in place, with *More in Help →* and up to two reference links. Help topics link official docs (Cargo, Microsoft, rustup) and Wikipedia. |
| **Status** | The last row of the Project card: ready to generate, the first blocking problem, or progress while generating. |
| **Rust book** | Links the offline copy when its `index.html` exists (active sysroot, any rustup toolchain, `/usr` or `/usr/local`); otherwise the online book and how to get the offline one (with rustup: *Install the offline Rust book…* = `rustup component add rust-docs`). |
| **MCP indicator** | "MCP ready" in the status bar: ProjectIle is an MCP server. Click it for the Help topic and the `claude mcp add` command; Welcome has the same. |

### MCP server (`projectile --mcp`)

JSON-RPC over stdio (one message per line, 1 MiB limit). Tools:

- `projectile_status`: ProjectIle version, source folder, PhotoCraft version, revision, size, profiles.
- `projectile_plan {source?, name, destination, profile?, options?}`: the Summary, edits, warnings
  and blockers. Writes nothing.
- `projectile_generate {…same…}`: generates; never overwrites unless `options.on_existing` says how. `destination` must be absolute.
- `projectile_regenerate {project, on_existing?, source?}`: rebuilds a generated project with its saved settings.

Register: About shows the exact `claude mcp add projectile -- <path to projectile> --mcp` line.

## Code map

| File | Role |
|---|---|
| `src/source.rs` | find and validate the PhotoCraft source, its version and fonts |
| `src/plan.rs` | inventory, profiles, options, components, summary, file edits and diff preview |
| `src/generator.rs` | executes a plan (copy, edits, metadata, backup, git) with progress events |
| `src/main.rs` | app state, settings, window, menus, tabs, footer, shortcuts, worker thread |
| `src/pages.rs` | Welcome, Create Project, Preferences, About and the output pane |
| `src/tree.rs` | the Components tree and its lazy folder listings |
| `src/rust.rs` | rustup toolchains, the release schedule, rustup commands |
| `src/theme.rs`, `src/access.rs` | palettes and the ease-of-access tray |
| `src/mcp.rs` | stdio MCP server |
| `src/toolchain.rs` | rustc/cargo/rustup, VS Code and git detection; source revision |

## Checks

```sh
cargo test                                  # 59 tests, incl. offscreen renders of every page
cargo clippy --all-targets -- -D warnings
PROJECTILE_SNAPSHOT_DIR=/tmp/shots cargo test renders   # save the renders as PNGs
```

Non-test code never panics (`clippy::unwrap_used` etc. are denied). Layout references (CMake GUI,
VST3 SDK Project Generator, Photoshop, the Windows sign-in accessibility menu) informed interaction
patterns only; no code or artwork is reused.
