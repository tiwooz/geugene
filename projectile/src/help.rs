//! The Help tab's topics, the "Why?" explanations, and middle-click-for-help.
//!
//! Widgets register the screen area they cover with [`target`]; when the user middle-clicks, the
//! smallest registered area under the pointer decides which topic the Help tab opens.

use eframe::egui;

pub struct Topic {
    pub id: &'static str,
    pub section: &'static str,
    pub title: &'static str,
    /// Paragraphs. The first one is the short answer shown by "Why?".
    pub body: &'static [&'static str],
    /// Further reading: official docs and Wikipedia.
    pub links: &'static [(&'static str, &'static str)],
}

pub const SECTIONS: &[&str] =
    &["Making a project", "Reading the plan", "Project options", "Rust and tools", "Concepts", "Using ProjectIle", "When something goes wrong"];

pub const TOPICS: &[Topic] = &[
    // Making a project
    Topic {
        id: "start",
        section: "Making a project",
        title: "How a project is made",
        body: &[
            "ProjectIle copies the PhotoCraft source into a new folder, applies a few small, reviewable edits for the profile you chose, and writes some helper files. You then build that folder with Cargo like any Rust project.",
            "The steps: check the source folder, name the project and pick a location, choose a profile, tick components, read the Summary and Source review, press Generate, then run `cargo run -p photocraft` inside the new folder.",
            "Nothing is written until you press Generate, and an existing folder is never overwritten.",
        ],
        links: &[
            ("Source code (Wikipedia)", "https://en.wikipedia.org/wiki/Source_code"),
            ("Fork (Wikipedia)", "https://en.wikipedia.org/wiki/Fork_(software_development)"),
        ],
    },
    Topic {
        id: "naming",
        section: "Making a project",
        title: "Naming the project and choosing where to store it",
        body: &[
            "The name becomes the project's folder and its Cargo package name, so it follows rules both understand: ASCII letters, digits, - and _, starting with a letter, at most 64 characters (crates.io's limit). Lower case with hyphens, like my-photocore, is the usual style; ProjectIle converts _ to - and capitals to lower case for you.",
            "Windows reserves device names such as CON, PRN, AUX, NUL, COM1–COM9 and LPT1–LPT9 in every folder, so those are refused even on Linux and macOS: the project stays shareable.",
            "For the location, pick a folder you own, such as Projects in your home folder (the default). Avoid folders synced by cloud storage (Dropbox, OneDrive, iCloud): a Rust build writes gigabytes into target/ and changes it constantly. Avoid system folders that need administrator rights.",
            "If a project with that name is already there, ProjectIle asks what to do with it: keep it renamed aside, replace files inside it, or delete it and start fresh.",
        ],
        links: &[
            ("Cargo: the package name field", "https://doc.rust-lang.org/cargo/reference/manifest.html#the-name-field"),
            ("Microsoft: naming files and folders", "https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file"),
            ("Filename (Wikipedia)", "https://en.wikipedia.org/wiki/Filename"),
            ("Directory (Wikipedia)", "https://en.wikipedia.org/wiki/Directory_(computing)"),
        ],
    },
    Topic {
        id: "regenerate",
        section: "Making a project",
        title: "Regenerating a project",
        body: &[
            "Regenerate rebuilds a project ProjectIle made, from the current PhotoCraft source: the same name, folder, profile and options, read from the project's .projectile.json. Use it to pick up PhotoCraft updates, to refresh the backup folder, or to start over after experiments.",
            "You choose what happens to the current folder: keep it renamed aside (the safe default), replace files inside it (your extra files stay, your edits to generated files are overwritten), or delete it first (asks to confirm). Untick \"Keep the profile and options\" to regenerate with what the Create Project tab is set to now.",
        ],
        links: &[],
    },
    Topic {
        id: "replace",
        section: "Making a project",
        title: "When the project folder already exists",
        body: &[
            "ProjectIle never overwrites anything unless you say so. Under Creates it asks what to do: stop (and choose another name), keep the old folder by renaming it aside, replace files inside it, or delete it and make it anew.",
            "Keep the old folder: the old one becomes <name>-old-<date>-<time> next to the new project. Nothing is deleted, and if generation fails it is renamed back. The safest choice.",
            "Replace files inside it: ProjectIle generates into the folder. Files it writes overwrite files with the same names; files only you have (notes, experiments) stay. Useful to refresh a project while keeping extras, but your edits to PhotoCraft files are overwritten.",
            "Delete it and make it anew: the folder and everything in it is permanently deleted first, after a confirmation that shows how many files go. ProjectIle refuses this for your home folder, anything containing it or the PhotoCraft source, and folders right under the root of the disk. Agents using MCP can't choose it.",
        ],
        links: &[],
    },
    Topic {
        id: "source",
        section: "Making a project",
        title: "Source folder",
        body: &[
            "The PhotoCraft checkout ProjectIle copies from. ProjectIle is a separate app, so it needs to be told where PhotoCraft is; it only reads that folder and never changes, builds or launches it.",
            "A valid source has a workspace Cargo.toml plus apps/photocraft, crates/engine and crates/ui-egui. ProjectIle shows the PhotoCraft version and git revision it found.",
            "Set PROJECTILE_SOURCE to preselect it.",
        ],
        links: &[("Source code (Wikipedia)", "https://en.wikipedia.org/wiki/Source_code")],
    },
    Topic {
        id: "name",
        section: "Making a project",
        title: "Project name",
        body: &[
            "The name becomes the folder name and part of the generated app's id, so it must be portable: an ASCII letter first, then letters, digits, - or _ (at most 64 characters). It is lower-cased and _ becomes -.",
            "Windows device names such as CON or COM1 are refused so the project can be shared with Windows users.",
        ],
        links: &[
            ("Cargo: the package name field", "https://doc.rust-lang.org/cargo/reference/manifest.html#the-name-field"),
            ("Microsoft: naming files and folders", "https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file"),
            ("Filename (Wikipedia)", "https://en.wikipedia.org/wiki/Filename"),
        ],
    },
    Topic {
        id: "location",
        section: "Making a project",
        title: "Location",
        body: &[
            "The parent folder. ProjectIle creates a new folder named after the project inside it, shown on the Creates line.",
            "It must be outside the PhotoCraft source (a project inside its own source would copy itself), and the project folder must not exist yet: ProjectIle never overwrites anything.",
        ],
        links: &[("Directory (Wikipedia)", "https://en.wikipedia.org/wiki/Directory_(computing)")],
    },
    Topic {
        id: "profile",
        section: "Making a project",
        title: "Profiles: Base, Viewer, Full, Minimal",
        body: &[
            "A profile decides what the generated editor can do. Base keeps PhotoCraft's real interface but allows only Brush, Eraser, Paint Bucket, Fill, file basics, undo/redo, colours and navigation: a small, understandable starting point.",
            "Viewer opens the image you choose at startup, has no editing tools, and always includes the untouched PhotoCraft source in backup/ so it is available in every Viewer project.",
            "Base works by switching on PhotoCraft's `base-profile` feature, which makes the engine refuse every other command. It is a learning restriction, not a security sandbox: all the source is still there to read and extend.",
            "Full (the whole of PhotoCraft) and Minimal (a pruned build) are previews: ProjectIle shows what they would contain, but cannot generate them yet.",
        ],
        links: &[("Adobe Photoshop (Wikipedia)", "https://en.wikipedia.org/wiki/Adobe_Photoshop")],
    },
    Topic {
        id: "configure",
        section: "Making a project",
        title: "Configure",
        body: &[
            "Rescans the source folder and your tools, rebuilds the plan and reports problems, like CMake's Configure button. The plan also refreshes by itself whenever you change something; Configure is for when files changed outside ProjectIle.",
        ],
        links: &[],
    },
    Topic {
        id: "status",
        section: "Making a project",
        title: "Ready to generate / blocked",
        body: &[
            "The line at the bottom of the Project card says whether the plan can be generated. When it can't, it names the first problem; the Summary lists all of them in red.",
            "Common causes: the project folder already exists, the name has characters it can't use, or the source folder is not PhotoCraft.",
        ],
        links: &[],
    },
    Topic {
        id: "generate",
        section: "Making a project",
        title: "Generate",
        body: &[
            "Copies the source (without .git, build output and local-only folders), applies the edits shown in Source review, writes the helper files and metadata, copies the backup, and runs git if you asked for it. Progress and every step appear in the output log.",
            "If anything fails, the half-made folder is removed, so you never end up with a broken project that looks finished.",
        ],
        links: &[("Software build (Wikipedia)", "https://en.wikipedia.org/wiki/Software_build")],
    },
    Topic {
        id: "after",
        section: "Making a project",
        title: "After generating",
        body: &[
            "Open the folder (or VS Code), then build and run the editor with `cargo run -p photocraft`. The first build downloads and compiles dependencies and takes several minutes; later builds are fast.",
            "PROJECTILE.md in the project explains its layout and useful commands; .projectile.json records how it was made.",
        ],
        links: &[],
    },
    // Reading the plan
    Topic {
        id: "signs",
        section: "Reading the plan",
        title: "The + ~ · - signs",
        body: &[
            "They read like `git diff`: + is enacted (copied and used) or added by ProjectIle, ~ is copied and edited by ProjectIle, · is compiled but dormant, - won't be used (left out or switched off).",
        ],
        links: &[("Diff (Wikipedia)", "https://en.wikipedia.org/wiki/Diff")],
    },
    Topic {
        id: "summary",
        section: "Reading the plan",
        title: "Summary",
        body: &[
            "Two short lists. The first compares the profile with full PhotoCraft: which capabilities you get (+) and which are switched off (-). The second says what goes into the project folder, crates folded together and apps, folders and generated files listed one by one.",
        ],
        links: &[],
    },
    Topic {
        id: "components",
        section: "Reading the plan",
        title: "Components tree",
        body: &[
            "Every part of the project, like an installer's component list. Greyed checkboxes are decided by the profile; the others are options you can untick. Unticking turns that branch into - lines at once.",
            "Expand a component to browse its real folders and files, and click a file to read it in Source review. Hover a row for the reason behind its sign.",
        ],
        links: &[],
    },
    Topic {
        id: "review",
        section: "Reading the plan",
        title: "Source review",
        body: &[
            "Shows the exact edits ProjectIle will make, as a diff (red lines go, green lines arrive), or the file you clicked in the tree. The generator applies these very edits, so what you review is what you get.",
            "If PhotoCraft changed so an edit no longer fits, generation is blocked up front instead of producing a broken project.",
        ],
        links: &[("Diff (Wikipedia)", "https://en.wikipedia.org/wiki/Diff")],
    },
    Topic {
        id: "dormant",
        section: "Reading the plan",
        title: "Dormant crates",
        body: &[
            "Crates such as text, vector or plugins are still compiled because the engine depends on them, but Base switches their tools off. Removing them needs PhotoCraft's engine features to become optional first; that is what the Minimal profile is waiting for.",
        ],
        links: &[],
    },
    Topic {
        id: "crates",
        section: "Reading the plan",
        title: "Crates and layers",
        body: &[
            "A crate is a Rust library or program. PhotoCraft is split into layers: L0 foundation (geometry, colour, pixels, file formats), L1 the document model, L2 editing (history, brushes, filters, text, vectors), L3 rendering, L4 files and plug-ins, L5 the engine, L6 the interface. A crate may only use lower layers.",
        ],
        links: &[
            ("Library (Wikipedia)", "https://en.wikipedia.org/wiki/Library_(computing)"),
            ("Rust (Wikipedia)", "https://en.wikipedia.org/wiki/Rust_(programming_language)"),
        ],
    },
    Topic {
        id: "workspace",
        section: "Reading the plan",
        title: "Workspace files",
        body: &[
            "Cargo.toml lists the workspace's members and shared dependencies (ProjectIle trims it); Cargo.lock pins exact dependency versions so the project builds the same way everywhere. Licences, assets and configuration are copied as-is.",
        ],
        links: &[],
    },
    Topic {
        id: "left-out",
        section: "Reading the plan",
        title: "Not included",
        body: &[
            "Parts that would not belong in this profile: the browser build, PhotoCraft's maintenance tooling (xtask), and the headless CLI, which could run commands without the Base restriction. Tick Agent tools to bring the CLI back for MCP.",
        ],
        links: &[],
    },
    // Project options
    Topic {
        id: "backup",
        section: "Project options",
        title: "Original source backup",
        body: &[
            "Copies the untouched PhotoCraft source into a backup folder inside your project: a complete source tree of its own (backup/Cargo.toml, backup/crates, …). Viewer projects always include it; Base projects can turn it off. It is a fallback and a reference: compare your changes with the original, restore a file you broke, or let an AI agent read PhotoCraft without searching for the repository.",
            "It is excluded from the Cargo workspace and from rust-analyzer, so it never slows builds or confuses your editor.",
        ],
        links: &[("Backup (Wikipedia)", "https://en.wikipedia.org/wiki/Backup")],
    },
    Topic {
        id: "mcp",
        section: "Project options",
        title: "Agent tools (Claude Code MCP)",
        body: &[
            "MCP (Model Context Protocol) lets an AI agent such as Claude Code use tools. With this ticked, the project gets a .mcp.json that lets Claude Code drive your running editor: open files, paint, inspect layers, take screenshots.",
            "Start the editor with `cargo run -p photocraft -- --control 7878 --control-token-file .projectile/control.token`, then run `claude` in the project folder. Agents get the same Base restrictions you have. The token file is created by the editor and is gitignored.",
        ],
        links: &[
            ("Claude Code: MCP servers", "https://docs.anthropic.com/en/docs/claude-code/mcp"),
            ("Model Context Protocol (Wikipedia)", "https://en.wikipedia.org/wiki/Model_Context_Protocol"),
        ],
    },
    Topic {
        id: "viewer",
        section: "Making a project",
        title: "The Viewer profile",
        body: &[
            "A purely visual PhotoCore: the editor opens the image you choose at startup and shows nothing else. There are no tools, no painting, erasing or filling, no panels or toolbar, and the menu bar holds only About. You can still pan and zoom with the mouse.",
            "ProjectIle copies your image into the project (assets/viewer/startup.<ext>) and turns on PhotoCraft's viewer-profile feature, so `cargo run -p photocraft` shows it straight away, skipping the Home screen. Pass another file to show that one instead.",
        ],
        links: &[("Image viewer (Wikipedia)", "https://en.wikipedia.org/wiki/Image_viewer")],
    },
    Topic {
        id: "generated-menu",
        section: "Project options",
        title: "Generated app menus and visual debugging",
        body: &[
            "In Create > Components, include or omit the entire main menu bar with its component checkbox. When included, select the top-level tabs and nested submenu groups to show. Viewer keeps Help/About; use Base to customize editor menus.",
            "The optional Visual Debug menu opens egui's built-in inspection, memory, settings and style tools, plus widget diagnostics on hover. Menu visibility is presentation-only: it does not remove commands or restrict shortcuts. The debug menu requires the main menu bar.",
        ],
        links: &[("egui documentation", "https://docs.rs/egui/latest/egui/struct.Context.html")],
    },
    Topic {
        id: "quick-run",
        section: "Making a project",
        title: "Generate, build & run",
        body: &[
            "Skips straight to the result: generates the project, compiles it with `cargo build -p photocraft`, then starts the editor. The When done row has the same steps as checkboxes (Build, Run). The first build compiles every dependency and takes several minutes; the output log shows its progress.",
        ],
        links: &[("Software build (Wikipedia)", "https://en.wikipedia.org/wiki/Software_build")],
    },
    Topic {
        id: "android",
        section: "Project options",
        title: "Android build (experimental)",
        body: &[
            "Adds an android/ folder with a Gradle project in Java and an apps/photocraft-android Rust crate. Choose the JDK (17, 21 or 25) and Gradle (9.6.0 or 9.8.1, both compatible with the pinned Android Gradle Plugin); generated Android docs link to the selected installers and pin the Gradle wrapper. The Java NativeActivity is locked to portrait orientation; the Rust build includes arm64 phone and x86_64 emulator libraries. android/README.md lists build, emulator creation, launch and install steps.",
            "ProjectIle picks the API levels: target 36, which Google Play requires for new apps and updates since 31 August 2026, and minimum 26 (Android 8.0), since PhotoCraft's GPU renderer needs Vulkan from API 24 on. Untick Automatic to choose your own.",
            "PhotoCraft is a desktop editor first and has not been verified on Android yet; expect porting work. The Android entry point is one exported symbol, which Rust counts as unsafe code; it is the only such exception and is documented in place.",
        ],
        links: &[
            ("Google Play target API requirements", "https://support.google.com/googleplay/android-developer/answer/11926878"),
            ("Android (Wikipedia)", "https://en.wikipedia.org/wiki/Android_(operating_system)"),
            ("Gradle (Wikipedia)", "https://en.wikipedia.org/wiki/Gradle"),
        ],
    },
    Topic {
        id: "github",
        section: "Project options",
        title: "Git and GitHub",
        body: &[
            "Commit records the project's current files in its local git history (starting a repository if needed). Nothing leaves your computer.",
            "Publish creates a repository on your GitHub account with the GitHub CLI (`gh repo create --source . --push`) and uploads the commits; you choose private or public, and ProjectIle shows the exact command and asks first. After that, Push uploads new commits. Sign in once with `gh auth login` in a terminal.",
        ],
        links: &[("GitHub CLI", "https://cli.github.com/"), ("GitHub (Wikipedia)", "https://en.wikipedia.org/wiki/GitHub")],
    },
    Topic {
        id: "docs",
        section: "Project options",
        title: "Documentation",
        body: &[
            "Copies PhotoCraft's docs/ folder: architecture, contributing rules, parity checklists. Untick it for a smaller project; the backup (if ticked) still has them.",
        ],
        links: &[],
    },
    Topic {
        id: "vscode",
        section: "Project options",
        title: "VS Code setup",
        body: &[
            "Writes .vscode/ with recommended extensions (rust-analyzer for code intelligence, Even Better TOML), settings that keep target/ and the backup out of search, and tasks to run, check and test from Terminal › Run Task.",
        ],
        links: &[("Visual Studio Code (Wikipedia)", "https://en.wikipedia.org/wiki/Visual_Studio_Code")],
    },
    Topic {
        id: "git",
        section: "Project options",
        title: "Git repository",
        body: &[
            "Runs `git init` and makes a first commit of the generated files, so `git diff` later shows exactly what you changed. If git has no user name or email configured, the commit is skipped with a warning and the project is still complete.",
        ],
        links: &[("Git (Wikipedia)", "https://en.wikipedia.org/wiki/Git"), ("Pro Git book", "https://git-scm.com/book/en/v2")],
    },
    Topic {
        id: "pin",
        section: "Project options",
        title: "Rust toolchain pin",
        body: &[
            "Writes rust-toolchain.toml so rustup builds this project with one exact Rust version, whatever your default is. Set it with Pin on the Rust tab. It has no effect without rustup.",
        ],
        links: &[],
    },
    Topic {
        id: "startup",
        section: "Project options",
        title: "Generated app startup",
        body: &["The window size and interface scale the generated editor starts with, written into its main.rs. They don't change ProjectIle's own window."],
        links: &[],
    },
    // Rust and tools
    Topic {
        id: "rust",
        section: "Rust and tools",
        title: "The Rust tab",
        body: &[
            "Shows the Rust compiler you have, where it comes from, and whether it is new enough for PhotoCraft (it needs the version in its Cargo.toml `rust-version`). With rustup you can install other versions, choose the default, uninstall, and pin a version for new projects.",
        ],
        links: &[("Rust (Wikipedia)", "https://en.wikipedia.org/wiki/Rust_(programming_language)")],
    },
    Topic {
        id: "rustup",
        section: "Rust and tools",
        title: "rustup and toolchains",
        body: &[
            "rustup is Rust's official version manager. A toolchain is one Rust version with its compiler and Cargo. stable is the newest release (every six weeks), beta the next one, nightly a daily build.",
            "ProjectIle shows every rustup command and asks before it runs; it never uses sudo.",
        ],
        links: &[
            ("rustup.rs", "https://rustup.rs/"),
            ("Toolchain (Wikipedia)", "https://en.wikipedia.org/wiki/Toolchain"),
            ("Package manager (Wikipedia)", "https://en.wikipedia.org/wiki/Package_manager"),
        ],
    },
    Topic {
        id: "rust-book",
        section: "Rust and tools",
        title: "The Rust book",
        body: &[
            "\"The Rust Programming Language\" is the free official introduction. ProjectIle links the offline copy when it is installed (it comes with the rust-docs component), and the online one otherwise.",
            "Get the offline copy with `rustup component add rust-docs` (then `rustup doc --book`), or your distribution's Rust documentation package.",
        ],
        links: &[("Read online", "https://doc.rust-lang.org/book/")],
    },
    Topic {
        id: "android-tools",
        section: "Rust and tools",
        title: "Android tools",
        body: &[
            "Building the Android app takes the selected JDK and Gradle versions, the Android SDK command-line tools (sdkmanager), platform tools (adb), the SDK platform and build tools for the target API, an NDK (r25 or newer for Rust), cargo-ndk, and Rust's aarch64-linux-android target (which needs rustup).",
            "The Toolchains tab checks each one. What ProjectIle can install for your user (SDK packages with sdkmanager, cargo-ndk, Rust targets) it offers as a confirmed button; what needs administrator rights (the JDK, Gradle from your package manager) it shows as a command to copy, plus the official download page.",
        ],
        links: &[
            ("Android command-line tools", "https://developer.android.com/studio#command-line-tools-only"),
            ("Android NDK", "https://developer.android.com/ndk/downloads"),
            ("Eclipse Temurin (OpenJDK)", "https://adoptium.net/temurin/releases/"),
            ("Install Gradle", "https://gradle.org/install/"),
        ],
    },
    Topic {
        id: "commands",
        section: "Rust and tools",
        title: "Useful commands",
        body: &[
            "The Toolchains tab lists the commands people use most: everyday Cargo (check, build, run, test, fmt, clippy, doc, tree), rustup, ones for your PhotoCore project (open an image, unrestricted build, allowlist tests, MCP control, compare with the backup), and Android development (targets, cargo-ndk, sdkmanager, building the APK, adb install and logcat). Copy any of them; the safe ones also have Run, which runs them inside your generated project.",
        ],
        links: &[("Cargo commands", "https://doc.rust-lang.org/cargo/commands/"), ("rustup book", "https://rust-lang.github.io/rustup/")],
    },
    Topic {
        id: "projectile-mcp",
        section: "Rust and tools",
        title: "ProjectIle supports MCP",
        body: &[
            "Yes: ProjectIle is itself an MCP server. Run `projectile --mcp` (or register it with the command on the About tab) and Claude Code can inspect the source (projectile_status), preview a plan (projectile_plan) and generate a project (projectile_generate), with the same rules as the window: nothing is overwritten.",
        ],
        links: &[
            ("Claude Code: MCP servers", "https://docs.anthropic.com/en/docs/claude-code/mcp"),
            ("Model Context Protocol (Wikipedia)", "https://en.wikipedia.org/wiki/Model_Context_Protocol"),
        ],
    },
    // Concepts
    Topic {
        id: "c-source-code",
        section: "Concepts",
        title: "Source code",
        body: &[
            "The human-readable text a program is written in. Compiling turns it into a program you can run. PhotoCraft's source is what ProjectIle copies into your project, so you can read and change every part of the editor.",
        ],
        links: &[("Source code (Wikipedia)", "https://en.wikipedia.org/wiki/Source_code")],
    },
    Topic {
        id: "c-fork",
        section: "Concepts",
        title: "Fork",
        body: &[
            "A copy of a project that you develop independently. A generated PhotoCore project is a fork of PhotoCraft: you can change it freely, and the original source backup lets you compare it with where it started.",
        ],
        links: &[("Fork (software development) (Wikipedia)", "https://en.wikipedia.org/wiki/Fork_(software_development)")],
    },
    Topic {
        id: "c-raster-editor",
        section: "Concepts",
        title: "Raster graphics editor",
        body: &["A program for editing images made of pixels, like Adobe Photoshop, GIMP or Krita. PhotoCraft is one, aiming to behave like Photoshop."],
        links: &[("Raster graphics editor (Wikipedia)", "https://en.wikipedia.org/wiki/Raster_graphics_editor")],
    },
    Topic {
        id: "c-rust",
        section: "Concepts",
        title: "Rust",
        body: &[
            "A programming language focused on speed and memory safety. Its compiler checks at build time that a program can't read freed memory or race between threads, so many crashes simply can't be written. PhotoCraft and ProjectIle are written entirely in Rust.",
        ],
        links: &[("Rust (programming language) (Wikipedia)", "https://en.wikipedia.org/wiki/Rust_(programming_language)")],
    },
    Topic {
        id: "c-memory-safety",
        section: "Concepts",
        title: "Memory safety",
        body: &[
            "Protection against bugs that read or write the wrong memory, a classic cause of crashes and security holes. Rust guarantees it unless code is marked `unsafe`; PhotoCraft allows `unsafe` in only one small, isolated crate.",
        ],
        links: &[("Memory safety (Wikipedia)", "https://en.wikipedia.org/wiki/Memory_safety")],
    },
    Topic {
        id: "c-compiler",
        section: "Concepts",
        title: "Compiler",
        body: &[
            "The program that translates source code into a runnable program. For Rust it is `rustc`, which Cargo runs for you. The first build of a project compiles every dependency, which is why it takes minutes.",
        ],
        links: &[("Compiler (Wikipedia)", "https://en.wikipedia.org/wiki/Compiler")],
    },
    Topic {
        id: "c-toolchain",
        section: "Concepts",
        title: "Toolchain",
        body: &[
            "The set of tools used to build software: for Rust, the compiler, Cargo, the standard library, rustfmt and clippy, all of one version. The Rust tab installs and switches toolchains.",
        ],
        links: &[("Toolchain (Wikipedia)", "https://en.wikipedia.org/wiki/Toolchain")],
    },
    Topic {
        id: "c-library",
        section: "Concepts",
        title: "Library (crate)",
        body: &[
            "Reusable code other programs build on. In Rust a library is called a crate. PhotoCraft is made of about twenty-five crates, each with one job, listed in the Components tree.",
        ],
        links: &[("Library (computing) (Wikipedia)", "https://en.wikipedia.org/wiki/Library_(computing)")],
    },
    Topic {
        id: "c-modular",
        section: "Concepts",
        title: "Modular programming",
        body: &[
            "Splitting a program into independent parts with clear interfaces. PhotoCraft's crates are layered so lower ones never depend on higher ones, which is what makes trimmed profiles possible.",
        ],
        links: &[("Modular programming (Wikipedia)", "https://en.wikipedia.org/wiki/Modular_programming")],
    },
    Topic {
        id: "c-package-manager",
        section: "Concepts",
        title: "Package manager and dependencies",
        body: &[
            "A tool that downloads the libraries a program depends on, in compatible versions. Rust's is Cargo: Cargo.toml lists dependencies and Cargo.lock pins their exact versions.",
        ],
        links: &[("Package manager (Wikipedia)", "https://en.wikipedia.org/wiki/Package_manager"), ("Cargo Book", "https://doc.rust-lang.org/cargo/")],
    },
    Topic {
        id: "c-monorepo",
        section: "Concepts",
        title: "Workspace (monorepo)",
        body: &[
            "One repository holding several related packages that are built together. PhotoCraft is a Cargo workspace; ProjectIle trims the workspace's member list for your profile.",
        ],
        links: &[("Monorepo (Wikipedia)", "https://en.wikipedia.org/wiki/Monorepo")],
    },
    Topic {
        id: "c-feature-toggle",
        section: "Concepts",
        title: "Feature flag",
        body: &[
            "A switch in the code that turns a capability on or off without removing it. The Base profile is the `base-profile` Cargo feature: the code stays, the engine refuses commands outside it.",
        ],
        links: &[("Feature toggle (Wikipedia)", "https://en.wikipedia.org/wiki/Feature_toggle")],
    },
    Topic {
        id: "c-build",
        section: "Concepts",
        title: "Software build",
        body: &[
            "Turning source code into a runnable program: compiling, linking and copying resources. `cargo build` builds, `cargo run` builds and starts, and output goes into the target/ folder.",
        ],
        links: &[("Software build (Wikipedia)", "https://en.wikipedia.org/wiki/Software_build")],
    },
    Topic {
        id: "c-version-control",
        section: "Concepts",
        title: "Version control and Git",
        body: &[
            "A system that records every change to files so you can compare, undo and share them. Git is the most used one. ProjectIle can start a Git repository in your project with the generated files as the first commit.",
        ],
        links: &[("Version control (Wikipedia)", "https://en.wikipedia.org/wiki/Version_control"), ("Git (Wikipedia)", "https://en.wikipedia.org/wiki/Git")],
    },
    Topic {
        id: "c-diff",
        section: "Concepts",
        title: "Diff",
        body: &[
            "A listing of the differences between two versions of a text, with removed lines marked - and added lines +. Source review shows ProjectIle's edits this way, and the signs in the Summary borrow the same idea.",
        ],
        links: &[("Diff (Wikipedia)", "https://en.wikipedia.org/wiki/Diff")],
    },
    Topic {
        id: "c-backup",
        section: "Concepts",
        title: "Backup",
        body: &["A copy kept so the original can be restored or consulted. The original source backup is a read-only copy of PhotoCraft inside your project."],
        links: &[("Backup (Wikipedia)", "https://en.wikipedia.org/wiki/Backup")],
    },
    Topic {
        id: "c-snapshot",
        section: "Concepts",
        title: "Snapshot",
        body: &[
            "The state of files at one moment. A generated project is a snapshot of the PhotoCraft source as it was when you pressed Generate; .projectile.json records which revision.",
        ],
        links: &[("Snapshot (computer storage) (Wikipedia)", "https://en.wikipedia.org/wiki/Snapshot_(computer_storage)")],
    },
    Topic {
        id: "c-directory",
        section: "Concepts",
        title: "Directory and path",
        body: &[
            "A directory (folder) holds files and other folders; a path like /home/you/Projects/my-photocore says where something is. Location is a path to the parent directory of your project.",
        ],
        links: &[("Path (computing) (Wikipedia)", "https://en.wikipedia.org/wiki/Path_(computing)")],
    },
    Topic {
        id: "c-naming",
        section: "Concepts",
        title: "Naming conventions",
        body: &[
            "Agreed ways to write names. Rust packages usually use lower-case words joined by hyphens (kebab case), such as my-photocore; ProjectIle normalises names to that style.",
        ],
        links: &[
            ("Naming convention (programming) (Wikipedia)", "https://en.wikipedia.org/wiki/Naming_convention_(programming)"),
            ("Kebab case (Wikipedia)", "https://en.wikipedia.org/wiki/Kebab_case"),
        ],
    },
    Topic {
        id: "c-ascii",
        section: "Concepts",
        title: "ASCII",
        body: &[
            "The basic 128-character set of English letters, digits and punctuation. Project names are limited to ASCII letters, digits, - and _ so every file system and tool accepts them.",
        ],
        links: &[("ASCII (Wikipedia)", "https://en.wikipedia.org/wiki/ASCII")],
    },
    Topic {
        id: "c-ide",
        section: "Concepts",
        title: "IDE and VS Code",
        body: &[
            "An integrated development environment combines an editor, build tools and a debugger. Visual Studio Code with the rust-analyzer extension is a popular choice for Rust; ProjectIle can prepare its settings.",
        ],
        links: &[("Integrated development environment (Wikipedia)", "https://en.wikipedia.org/wiki/Integrated_development_environment")],
    },
    Topic {
        id: "c-cli",
        section: "Concepts",
        title: "Command line",
        body: &[
            "Typing commands such as `cargo run` into a terminal instead of clicking. Every action in ProjectIle shows the command it stands for, so you can learn and repeat it.",
        ],
        links: &[("Command-line interface (Wikipedia)", "https://en.wikipedia.org/wiki/Command-line_interface")],
    },
    Topic {
        id: "c-env-var",
        section: "Concepts",
        title: "Environment variable",
        body: &[
            "A named setting a program reads from its environment. PROJECTILE_SOURCE preselects the source folder; PATH tells the system where programs such as cargo live.",
        ],
        links: &[("Environment variable (Wikipedia)", "https://en.wikipedia.org/wiki/Environment_variable")],
    },
    Topic {
        id: "c-ai-agent",
        section: "Concepts",
        title: "AI agent",
        body: &[
            "An AI system that can take actions, not only answer: read files, run tools, edit code. Claude Code is one; MCP is how it reaches tools such as ProjectIle or a running PhotoCraft.",
        ],
        links: &[("AI agent (Wikipedia)", "https://en.wikipedia.org/wiki/AI_agent")],
    },
    Topic {
        id: "c-mcp",
        section: "Concepts",
        title: "Model Context Protocol",
        body: &[
            "An open standard for connecting AI agents to tools and data. ProjectIle speaks it with `projectile --mcp`, and generated projects can include a .mcp.json for the editor.",
        ],
        links: &[("Model Context Protocol (Wikipedia)", "https://en.wikipedia.org/wiki/Model_Context_Protocol")],
    },
    Topic {
        id: "c-open-source",
        section: "Concepts",
        title: "Open source and licences",
        body: &[
            "Software whose source anyone may read, change and share under its licence. ProjectIle is available under the MIT or Apache 2.0 licence, both permissive: you may reuse it, keeping the notices.",
        ],
        links: &[
            ("Free and open-source software (Wikipedia)", "https://en.wikipedia.org/wiki/Free_and_open-source_software"),
            ("MIT License (Wikipedia)", "https://en.wikipedia.org/wiki/MIT_License"),
            ("Apache License (Wikipedia)", "https://en.wikipedia.org/wiki/Apache_License"),
        ],
    },
    Topic {
        id: "c-clean-room",
        section: "Concepts",
        title: "Clean-room design",
        body: &[
            "Recreating how something behaves without copying its code. PhotoCraft matches Photoshop by studying behaviour and public specifications only, never Adobe's code or assets.",
        ],
        links: &[("Clean-room design (Wikipedia)", "https://en.wikipedia.org/wiki/Clean-room_design")],
    },
    Topic {
        id: "c-accessibility",
        section: "Concepts",
        title: "Accessibility",
        body: &[
            "Designing software usable by everyone, including people with low vision or limited mobility. ProjectIle's tray offers high contrast, larger text and more, and supports screen readers.",
        ],
        links: &[
            ("Computer accessibility (Wikipedia)", "https://en.wikipedia.org/wiki/Computer_accessibility"),
            ("Screen reader (Wikipedia)", "https://en.wikipedia.org/wiki/Screen_reader"),
        ],
    },
    // Using ProjectIle
    Topic {
        id: "help",
        section: "Using ProjectIle",
        title: "Getting help",
        body: &[
            "Middle-click (press the mouse wheel on) almost anything in ProjectIle to jump to its topic here. Why? links open a short explanation in place.",
        ],
        links: &[("Mouse button (Wikipedia)", "https://en.wikipedia.org/wiki/Mouse_button")],
    },
    Topic {
        id: "community",
        section: "Using ProjectIle",
        title: "Community: Discord and Claude Console",
        body: &[
            "The official ArtCraft Discord is where PhotoCraft's makers and users talk: ask questions, report bugs, suggest features, share your PhotoCore projects and help newcomers. Every contribution makes a difference.",
            "The Claude Console (platform.claude.com) is Anthropic's site for API keys, usage and billing, which agents such as Claude Code can use.",
        ],
        links: &[
            ("ArtCraft Discord", "https://discord.gg/artcraft"),
            ("Claude Console", "https://platform.claude.com/"),
            ("Discord (Wikipedia)", "https://en.wikipedia.org/wiki/Discord"),
        ],
    },
    Topic {
        id: "system",
        section: "Using ProjectIle",
        title: "System and layout",
        body: &[
            "ProjectIle detects your operating system, Linux distribution, desktop and package manager. The status bar shows it (for example Arch Linux · Hyprland), install hints use your package manager (sudo pacman -S … on Arch), and ProjectIle never runs sudo itself.",
            "The layout follows your desktop: dialogs put the main action last on GNOME, macOS and most Linux desktops, and first on KDE Plasma and Windows; tiling window managers (Hyprland, Sway, i3) get compact spacing. Preferences › System lets you override it.",
        ],
        links: &[
            ("Arch Linux (Wikipedia)", "https://en.wikipedia.org/wiki/Arch_Linux"),
            ("Tiling window manager (Wikipedia)", "https://en.wikipedia.org/wiki/Tiling_window_manager"),
        ],
    },
    Topic {
        id: "access",
        section: "Using ProjectIle",
        title: "Ease of access",
        body: &[
            "The icon at the bottom right opens the ease-of-access tray: magnifier, high contrast, larger text, bigger click targets, reduced motion, a strong focus outline, and beginner explanations under every field. Ctrl+Alt+U opens it too. Screen readers work through AccessKit.",
        ],
        links: &[],
    },
    Topic {
        id: "language",
        section: "Using ProjectIle",
        title: "Language and clock",
        body: &[
            "The selector at the top right chooses ProjectIle's interface language. English is the only language for now; translations can be contributed. The clock next to it shows your local time.",
        ],
        links: &[],
    },
    Topic {
        id: "shortcuts",
        section: "Using ProjectIle",
        title: "Keyboard shortcuts",
        body: &[
            "Ctrl+1–6 tabs · Ctrl+G Generate · F5 Configure · F1 Help · Ctrl+= / Ctrl+− / Ctrl+0 scale · F11 full screen · F12 screenshot · Ctrl+Shift+C copy output · Ctrl+Alt+U ease of access · Ctrl+Q quit.",
        ],
        links: &[],
    },
    Topic {
        id: "preferences",
        section: "Using ProjectIle",
        title: "Preferences",
        body: &[
            "Theme, interface scale, window size and screenshots for ProjectIle itself, and the generated app's startup size. Everything is saved between launches.",
        ],
        links: &[],
    },
    Topic {
        id: "open-source",
        section: "Using ProjectIle",
        title: "Open source",
        body: &[
            "ProjectIle is free software under the MIT License or the Apache License 2.0, at your option. Read the source, report issues, and send improvements; the About tab has the links and the licences of every library it uses.",
        ],
        links: &[
            ("Source on GitHub", "https://github.com/tiwooz/photocore"),
            ("Free and open-source software (Wikipedia)", "https://en.wikipedia.org/wiki/Free_and_open-source_software"),
        ],
    },
    // Troubleshooting
    Topic {
        id: "no-source",
        section: "When something goes wrong",
        title: "\"No PhotoCraft source folder found\"",
        body: &[
            "Choose the root of a PhotoCraft checkout in Create Project › Source (the folder with Cargo.toml, apps/ and crates/). Clone it from GitHub if you don't have one.",
        ],
        links: &[("PhotoCraft on GitHub", "https://github.com/storytold/photocraft")],
    },
    Topic {
        id: "cant-generate",
        section: "When something goes wrong",
        title: "Generate is greyed out",
        body: &[
            "Hover it, or read the status line in the Project card: it names the problem. Full and Minimal are previews and can't be generated; switch to Base.",
        ],
        links: &[],
    },
    Topic {
        id: "git-commit",
        section: "When something goes wrong",
        title: "\"the first commit failed\"",
        body: &[
            "git needs your name and email once: `git config --global user.name \"Your Name\"` and `git config --global user.email you@example.com`, then commit in the project folder.",
        ],
        links: &[],
    },
    Topic {
        id: "no-rustup",
        section: "When something goes wrong",
        title: "\"Switching versions needs rustup\"",
        body: &[
            "Your Rust came from your system's package manager, which installs one version. Install rustup (the Rust tab can do it for your user, after asking) to manage several versions. Your system Rust is left alone.",
        ],
        links: &[],
    },
];

pub fn find(id: &str) -> Option<&'static Topic> {
    TOPICS.iter().find(|topic| topic.id == id)
}

/// The short answer for "Why?": the topic's first paragraph.
pub fn short(id: &str) -> &'static str {
    find(id).and_then(|topic| topic.body.first().copied()).unwrap_or("")
}

#[derive(Clone, Default)]
struct Targets(Vec<(egui::Rect, &'static str)>);

fn targets_id() -> egui::Id {
    egui::Id::new("projectile-help-targets")
}

/// Register `rect` (clipped to what is visible) as showing `topic` on middle-click.
pub fn target(ui: &egui::Ui, rect: egui::Rect, topic: &'static str) {
    let visible = rect.intersect(ui.clip_rect());
    if visible.is_positive() {
        ui.ctx().data_mut(|data| data.get_temp_mut_or_default::<Targets>(targets_id()).0.push((visible, topic)));
    }
}

/// The topic under a middle-click this frame, if any. Clears the registrations either way.
pub fn take_middle_click(ctx: &egui::Context) -> Option<&'static str> {
    let targets = ctx.data_mut(|data| std::mem::take(&mut data.get_temp_mut_or_default::<Targets>(targets_id()).0));
    let pos = ctx.input(|input| input.pointer.button_clicked(egui::PointerButton::Middle).then(|| input.pointer.interact_pos()).flatten())?;
    targets.into_iter().filter(|(rect, _)| rect.contains(pos)).min_by(|a, b| a.0.area().total_cmp(&b.0.area())).map(|(_, topic)| topic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_are_unique_complete_and_sectioned() {
        let mut ids = std::collections::HashSet::new();
        for topic in TOPICS {
            assert!(ids.insert(topic.id), "duplicate topic {}", topic.id);
            assert!(SECTIONS.contains(&topic.section), "{} has unknown section", topic.id);
            assert!(!topic.body.is_empty() && !short(topic.id).is_empty());
        }
    }

    #[test]
    fn links_are_well_formed() {
        for topic in TOPICS {
            for (label, url) in topic.links {
                assert!(!label.is_empty() && url.starts_with("https://") && !url.contains(' '), "{}: {url}", topic.id);
            }
        }
        assert!(TOPICS.iter().filter(|t| t.section == "Concepts").all(|t| t.links.iter().any(|(_, url)| url.contains("wikipedia.org"))));
    }

    #[test]
    fn smallest_target_under_the_pointer_wins() {
        let ctx = egui::Context::default();
        let mut picked = None;
        let pos = egui::pos2(50.0, 50.0);
        let input = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton { pos, button: egui::PointerButton::Middle, pressed: true, modifiers: Default::default() },
                egui::Event::PointerButton { pos, button: egui::PointerButton::Middle, pressed: false, modifiers: Default::default() },
            ],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            target(ui, egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 400.0)), "start");
            target(ui, egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(20.0, 20.0)), "backup");
            target(ui, egui::Rect::from_min_size(egui::pos2(200.0, 200.0), egui::vec2(5.0, 5.0)), "git");
            picked = take_middle_click(ui.ctx());
        });
        output.textures_delta.clear();
        assert_eq!(picked, Some("backup"));
    }
}
