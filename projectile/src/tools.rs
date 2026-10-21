//! Android development tools: what is installed (JDK, Gradle, Android SDK command-line tools,
//! platform tools, NDK, cargo-ndk, Rust's Android targets), how to install what's missing, and the
//! commands people use most. Detection runs on a worker thread: `gradle --version` and
//! `sdkmanager --list` take seconds.

use crate::system::{PackageManager, System};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Android Gradle Plugin 9 needs JDK 17 or newer.
pub const MIN_JDK: u32 = 17;
/// Rust's Android targets need NDK r25 or newer.
pub const MIN_NDK: u32 = 25;
pub const ANDROID_TARGETS: [&str; 3] = ["aarch64-linux-android", "armv7-linux-androideabi", "x86_64-linux-android"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// A command ProjectIle can run for the user (shown and confirmed first).
    Run { title: String, program: PathBuf, args: Vec<String>, explanation: String },
    /// A command for the user's terminal (it needs sudo or a shell setup).
    Terminal(String),
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub what: &'static str,
    /// Version or location when found.
    pub found: Option<String>,
    pub ok: bool,
    /// Why it isn't OK (too old, missing).
    pub problem: Option<String>,
    pub fixes: Vec<Fix>,
    pub link: (&'static str, &'static str),
}

#[derive(Debug, Clone, Default)]
pub struct AndroidTools {
    pub checks: Vec<Check>,
    pub sdk_root: Option<PathBuf>,
}

fn run(program: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    out.status.success().then_some(text)
}

/// `17` from `openjdk version "17.0.20.1" …` or `javac 21.0.1`.
pub fn java_major(text: &str) -> Option<u32> {
    let version = text.split(['"', ' ', '\n']).find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = version.split('.');
    let first: u32 = parts.next()?.parse().ok()?;
    // Java 8 and older report "1.8.0".
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

/// Newest stable `ndk;<version>` in `sdkmanager --list` output (release candidates skipped).
pub fn latest_stable_ndk(list: &str) -> Option<String> {
    let mut best: Option<(Vec<u32>, String)> = None;
    for line in list.lines() {
        let mut columns = line.split('|').map(str::trim);
        let (Some(id), Some(version)) = (columns.next(), columns.next()) else { continue };
        let Some(number) = id.strip_prefix("ndk;") else { continue };
        if version.contains("rc") || version.contains("beta") || version.contains("alpha") {
            continue;
        }
        let key: Vec<u32> = number.split('.').filter_map(|p| p.parse().ok()).collect();
        if key.is_empty() {
            continue;
        }
        if best.as_ref().is_none_or(|(k, _)| key > *k) {
            best = Some((key, id.to_owned()));
        }
    }
    best.map(|(_, id)| id)
}

fn sdk_tool(root: Option<&Path>, relative: &str, name: &str) -> Option<PathBuf> {
    let suffix = if cfg!(windows) { ".bat" } else { "" };
    root.map(|r| r.join(relative).join(format!("{name}{suffix}"))).filter(|p| p.is_file()).or_else(|| crate::toolchain::find_program(name))
}

fn sdkmanager_tool(root: Option<&Path>) -> Option<PathBuf> {
    let suffix = if cfg!(windows) { ".bat" } else { "" };
    if let Some(root) = root {
        let tools = root.join("cmdline-tools");
        if let Ok(entries) = std::fs::read_dir(&tools) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()) && entry.file_name() != std::ffi::OsStr::new("latest"))
                .map(|entry| entry.path())
                .collect();
            versions.sort();
            versions.reverse();
            let latest = tools.join("latest");
            let candidates = std::iter::once(latest)
                .chain(versions)
                .chain(std::iter::once(root.join("tools")))
                .map(|dir| dir.join("bin").join(format!("sdkmanager{suffix}")));
            if let Some(program) = candidates.into_iter().find(|path| path.is_file()) {
                return Some(program);
            }
        }
    }
    crate::toolchain::find_program("sdkmanager")
}

fn terminal(pm: Option<PackageManager>, packages: &[(PackageManager, &str)]) -> Vec<Fix> {
    let Some(pm) = pm else { return Vec::new() };
    packages.iter().find(|(p, _)| *p == pm).map(|(_, cmd)| Fix::Terminal((*cmd).to_owned())).into_iter().collect()
}

pub fn detect(system: &System, rustup: Option<&Path>, rustc: Option<&Path>, cargo: Option<&Path>, target_sdk: u32) -> AndroidTools {
    let pm = system.package_manager;
    let sdk = crate::android::Sdk::detect();
    let root = sdk.root.clone();
    let mut checks = Vec::new();

    // JDK
    let javac = crate::toolchain::find_program("javac");
    let java_text = javac.as_deref().and_then(|j| run(j, &["-version"]));
    let major = java_text.as_deref().and_then(java_major);
    checks.push(Check {
        name: "Java (JDK)",
        what: "compiles the Java side; the Android Gradle Plugin needs JDK 17+",
        found: java_text.as_deref().map(|t| t.trim().to_owned()),
        ok: major.is_some_and(|m| m >= MIN_JDK),
        problem: match major {
            None => Some("no JDK found (javac is missing)".into()),
            Some(m) if m < MIN_JDK => Some(format!("JDK {m} is too old; install {MIN_JDK} or newer")),
            Some(_) => None,
        },
        fixes: terminal(
            pm,
            &[
                (PackageManager::Pacman, "sudo pacman -S jdk17-openjdk"),
                (PackageManager::Apt, "sudo apt install openjdk-17-jdk"),
                (PackageManager::Dnf, "sudo dnf install java-17-openjdk-devel"),
                (PackageManager::Zypper, "sudo zypper install java-17-openjdk-devel"),
                (PackageManager::Brew, "brew install openjdk@17"),
                (PackageManager::Winget, "winget install EclipseAdoptium.Temurin.17.JDK"),
            ],
        ),
        link: ("Eclipse Temurin (OpenJDK)", "https://adoptium.net/temurin/releases/"),
    });

    // Gradle
    let gradle = crate::toolchain::find_program("gradle");
    let gradle_version = gradle
        .as_deref()
        .and_then(|g| run(g, &["--version", "--quiet"]).or_else(|| run(g, &["--version"])))
        .and_then(|t| t.lines().find(|l| l.starts_with("Gradle ")).map(|l| l.trim().to_owned()));
    checks.push(Check {
        name: "Gradle",
        what: "builds the Android app (android/ is a Gradle project)",
        found: gradle_version.clone(),
        ok: gradle_version.is_some(),
        problem: gradle_version.is_none().then(|| "gradle was not found (Android Studio bundles one too)".to_owned()),
        fixes: terminal(
            pm,
            &[
                (PackageManager::Pacman, "sudo pacman -S gradle"),
                (PackageManager::Apt, "sudo apt install gradle"),
                (PackageManager::Dnf, "sudo dnf install gradle"),
                (PackageManager::Zypper, "sudo zypper install gradle"),
                (PackageManager::Brew, "brew install gradle"),
            ],
        ),
        link: ("Install Gradle", "https://gradle.org/install/"),
    });

    // SDK command-line tools (sdkmanager)
    let sdkmanager = sdkmanager_tool(root.as_deref());
    let sdkmanager_version = sdkmanager.as_deref().and_then(|s| run(s, &["--version"])).map(|t| t.trim().lines().last().unwrap_or_default().to_owned());
    checks.push(Check {
        name: "Android SDK command-line tools",
        what: "sdkmanager installs SDK platforms, build tools and the NDK",
        found: sdkmanager_version
            .as_ref()
            .map(|v| format!("sdkmanager {v} · SDK at {}", root.as_ref().map_or_else(|| "?".into(), |r| r.display().to_string()))),
        ok: sdkmanager.is_some(),
        problem: sdkmanager.is_none().then(|| "sdkmanager was not found; install the command-line tools into your SDK".to_owned()),
        fixes: Vec::new(),
        link: ("Android command-line tools", "https://developer.android.com/studio#command-line-tools-only"),
    });

    // Platform tools (adb)
    let adb = sdk_tool(root.as_deref(), "platform-tools", "adb");
    let adb_version = adb.as_deref().and_then(|a| run(a, &["version"])).and_then(|t| t.lines().next().map(str::to_owned));
    let mut sdk_packages = Vec::new();
    if adb.is_none() {
        sdk_packages.push("platform-tools".to_owned());
    }
    checks.push(Check {
        name: "Android platform tools (adb)",
        what: "installs the app on a phone or emulator and shows its logs",
        found: adb_version.clone(),
        ok: adb.is_some(),
        problem: adb.is_none().then(|| "adb was not found".to_owned()),
        fixes: Vec::new(),
        link: ("adb", "https://developer.android.com/tools/adb"),
    });

    // SDK platform for the target API, build tools
    let platform_ok = sdk.has_platform(target_sdk);
    if !platform_ok {
        sdk_packages.push(format!("platforms;android-{target_sdk}"));
    }
    let build_tools_ok = sdk.build_tools.iter().any(|b| b.split('.').next().and_then(|m| m.parse::<u32>().ok()).is_some_and(|m| m >= target_sdk));
    if !build_tools_ok {
        sdk_packages.push(format!("build-tools;{target_sdk}.0.0"));
    }
    checks.push(Check {
        name: "SDK platform and build tools",
        what: "the Android API the app is built against",
        found: Some(format!(
            "platforms {} · build tools {}",
            if sdk.platforms.is_empty() { "none".into() } else { sdk.platforms.iter().map(u32::to_string).collect::<Vec<_>>().join(", ") },
            if sdk.build_tools.is_empty() { "none".into() } else { sdk.build_tools.join(", ") }
        )),
        ok: platform_ok && build_tools_ok,
        problem: (!(platform_ok && build_tools_ok)).then(|| format!("API {target_sdk} platform or build tools are missing")),
        fixes: Vec::new(),
        link: ("sdkmanager", "https://developer.android.com/tools/sdkmanager"),
    });

    // NDK
    let ndk_major = sdk.ndks.iter().filter_map(|n| n.split('.').next()?.parse::<u32>().ok()).max();
    let latest_ndk = sdkmanager.as_deref().and_then(|s| run(s, &["--list"])).as_deref().and_then(latest_stable_ndk);
    let ndk_ok = ndk_major.is_some_and(|m| m >= MIN_NDK);
    if !ndk_ok && let Some(ndk) = &latest_ndk {
        sdk_packages.push(ndk.clone());
    }
    checks.push(Check {
        name: "Android NDK",
        what: "compiles Rust code for Android; Rust needs NDK r25 or newer",
        found: (!sdk.ndks.is_empty()).then(|| sdk.ndks.join(", ")),
        ok: ndk_ok,
        problem: match ndk_major {
            None => Some(format!(
                "no NDK installed{}",
                latest_ndk.as_ref().map_or(String::new(), |n| format!("; newest stable is {}", n.trim_start_matches("ndk;")))
            )),
            Some(m) if m < MIN_NDK => Some(format!("NDK {m} is too old for Rust (needs r{MIN_NDK}+)")),
            Some(_) => None,
        },
        fixes: Vec::new(),
        link: ("Android NDK", "https://developer.android.com/ndk/downloads"),
    });
    // One sdkmanager run installs everything missing above.
    if let (Some(sdkmanager), false) = (&sdkmanager, sdk_packages.is_empty()) {
        let mut args = vec!["--install".to_owned()];
        args.extend(sdk_packages.iter().cloned());
        let fix = Fix::Run {
            title: "Install Android SDK packages".into(),
            program: sdkmanager.clone(),
            args,
            explanation: format!(
                "Downloads {} into your Android SDK with sdkmanager. Google's licences must already be accepted (`sdkmanager --licenses`).",
                sdk_packages.join(", ")
            ),
        };
        for check in checks.iter_mut().filter(|c| !c.ok && matches!(c.name, "Android platform tools (adb)" | "SDK platform and build tools" | "Android NDK")) {
            check.fixes.push(fix.clone());
        }
    }

    // cargo-ndk
    let cargo_ndk = crate::toolchain::find_program("cargo-ndk");
    let cargo_ndk_version = cargo_ndk
        .as_deref()
        .and_then(|c| run(c, &["ndk", "--version"]).or_else(|| run(c, &["--version"])))
        .map(|t| t.trim().lines().next().unwrap_or_default().to_owned());
    checks.push(Check {
        name: "cargo-ndk",
        what: "builds Rust crates for Android with the NDK",
        found: cargo_ndk_version.clone().or_else(|| cargo_ndk.as_ref().map(|p| p.display().to_string())),
        ok: cargo_ndk.is_some(),
        problem: cargo_ndk.is_none().then(|| "cargo-ndk is not installed".to_owned()),
        fixes: cargo
            .map(|cargo| Fix::Run {
                title: "Install cargo-ndk".into(),
                program: cargo.to_path_buf(),
                args: vec!["install".into(), "cargo-ndk".into()],
                explanation: "Builds and installs cargo-ndk from crates.io into ~/.cargo/bin for your user.".into(),
            })
            .into_iter()
            .collect(),
        link: ("cargo-ndk", "https://github.com/bbqsrc/cargo-ndk"),
    });

    // Rust Android targets
    let installed: Vec<String> = match rustup {
        Some(rustup) => run(rustup, &["target", "list", "--installed"]).map(|t| t.lines().map(|l| l.trim().to_owned()).collect()).unwrap_or_default(),
        None => {
            let sysroot = rustc.and_then(|r| run(r, &["--print", "sysroot"])).map(|t| PathBuf::from(t.trim()));
            ANDROID_TARGETS.iter().filter(|t| sysroot.as_ref().is_some_and(|s| s.join("lib/rustlib").join(t).is_dir())).map(|t| (*t).to_owned()).collect()
        }
    };
    let has_arm64 = installed.iter().any(|t| t == "aarch64-linux-android");
    checks.push(Check {
        name: "Rust Android targets",
        what: "the Rust standard library for Android (arm64 phones need aarch64-linux-android)",
        found: (!installed.is_empty())
            .then(|| installed.iter().filter(|t| t.contains("android")).cloned().collect::<Vec<_>>().join(", "))
            .filter(|s| !s.is_empty()),
        ok: has_arm64,
        problem: (!has_arm64).then(|| {
            if rustup.is_some() {
                "aarch64-linux-android is not installed".to_owned()
            } else {
                "needs rustup: a system Rust package can't add targets (Rust tab › Install rustup)".to_owned()
            }
        }),
        fixes: rustup
            .map(|rustup| Fix::Run {
                title: "Add Rust's Android targets".into(),
                program: rustup.to_path_buf(),
                args: ["target", "add"].iter().map(|s| (*s).to_owned()).chain(ANDROID_TARGETS.iter().map(|t| (*t).to_owned())).collect(),
                explanation: "Downloads the Rust standard library for Android phones (arm64, armv7) and emulators (x86_64).".into(),
            })
            .into_iter()
            .collect(),
        link: ("Cross-compiling with rustup", "https://rust-lang.github.io/rustup/cross-compilation.html"),
    });

    AndroidTools { checks, sdk_root: root }
}

/// A command for the Useful commands list.
pub struct UsefulCommand {
    pub command: &'static str,
    pub what: &'static str,
    /// Safe to run in the generated project from ProjectIle (read-only or build-only).
    pub runnable: bool,
}

pub const RUST_COMMANDS: &[UsefulCommand] = &[
    UsefulCommand { command: "cargo check", what: "fast compile check, no binary", runnable: true },
    UsefulCommand { command: "cargo build", what: "build in debug mode (target/debug)", runnable: true },
    UsefulCommand { command: "cargo build --release", what: "optimised build (target/release)", runnable: true },
    UsefulCommand { command: "cargo run -p photocraft", what: "build and start the editor", runnable: false },
    UsefulCommand { command: "cargo test", what: "run the tests", runnable: true },
    UsefulCommand { command: "cargo fmt", what: "format all code the standard way", runnable: true },
    UsefulCommand { command: "cargo clippy --all-targets", what: "lint for common mistakes", runnable: true },
    UsefulCommand { command: "cargo doc --open", what: "build and open the project's API docs", runnable: false },
    UsefulCommand { command: "cargo tree", what: "show the dependency tree", runnable: true },
    UsefulCommand { command: "cargo update", what: "update Cargo.lock within allowed versions", runnable: false },
    UsefulCommand { command: "cargo clean", what: "delete target/ to free space", runnable: false },
];

pub const RUSTUP_COMMANDS: &[UsefulCommand] = &[
    UsefulCommand { command: "rustup show", what: "active toolchain and installed targets", runnable: false },
    UsefulCommand { command: "rustup update", what: "update stable, beta, nightly", runnable: false },
    UsefulCommand { command: "rustup component add rust-src rust-analyzer", what: "editor support (go to definition, completion)", runnable: false },
    UsefulCommand { command: "rustup doc --book", what: "open the offline Rust book", runnable: false },
];

pub const PHOTOCORE_COMMANDS: &[UsefulCommand] = &[
    UsefulCommand { command: "cargo run -p photocraft -- path/to/image.png", what: "open an image at startup", runnable: false },
    UsefulCommand { command: "cargo run -p photocraft --no-default-features", what: "unrestricted PhotoCraft instead of the profile", runnable: false },
    UsefulCommand { command: "cargo test -p photocraft-engine photocore_", what: "test the profile's command allowlist", runnable: true },
    UsefulCommand {
        command: "cargo run -p photocraft -- --control 7878 --control-token-file .projectile/control.token",
        what: "start with the MCP control port for Claude Code",
        runnable: false,
    },
    UsefulCommand { command: "diff -ru backup/crates crates", what: "compare your crates with the original backup", runnable: true },
];

pub const ANDROID_COMMANDS: &[UsefulCommand] = &[
    UsefulCommand { command: "javac -version", what: "check that the Java JDK is installed", runnable: false },
    UsefulCommand { command: "gradle --version", what: "check Gradle and the JVM it uses", runnable: false },
    UsefulCommand { command: "sdkmanager --version", what: "check the Android SDK command-line tools", runnable: false },
    UsefulCommand {
        command: "rustup target add aarch64-linux-android x86_64-linux-android",
        what: "Rust for Android arm64 phones and x86_64 emulators",
        runnable: false,
    },
    UsefulCommand { command: "cargo install cargo-ndk", what: "the Rust-for-Android build helper", runnable: false },
    UsefulCommand {
        command: "cargo check -p photocraft-android --target aarch64-linux-android",
        what: "cross-check the Rust Android entry crate",
        runnable: false,
    },
    UsefulCommand { command: "sdkmanager --licenses", what: "accept Google's SDK licences (once)", runnable: false },
    UsefulCommand { command: "sdkmanager --list_installed", what: "installed SDK packages", runnable: false },
    UsefulCommand { command: "sh android/build-rust.sh", what: "build the Rust library into android/app/src/main/jniLibs", runnable: false },
    UsefulCommand {
        command: "cd android && gradle wrapper --gradle-version 9.6.0 --distribution-type bin",
        what: "bootstrap the selected Gradle wrapper (replace with the version chosen in ProjectIle)",
        runnable: false,
    },
    UsefulCommand { command: "cd android && ./gradlew assembleDebug", what: "build the debug APK with the pinned Gradle", runnable: false },
    UsefulCommand { command: "adb devices", what: "list connected phones and emulators", runnable: false },
    UsefulCommand { command: "adb install -r android/app/build/outputs/apk/debug/app-debug.apk", what: "install the APK", runnable: false },
    UsefulCommand { command: "adb logcat -s RustStdoutStderr:V", what: "show the app's Rust output", runnable: false },
    UsefulCommand { command: "emulator -list-avds", what: "list Android emulators", runnable: false },
    UsefulCommand {
        command: "sdkmanager --install \"system-images;android-36;google_apis;x86_64\"",
        what: "install an x86_64 API 36 emulator image",
        runnable: false,
    },
    UsefulCommand {
        command: "avdmanager create avd --name PhotoCore_API_36 --package \"system-images;android-36;google_apis;x86_64\"",
        what: "create an API 36 x86_64 virtual device",
        runnable: false,
    },
    UsefulCommand { command: "emulator -avd PhotoCore_API_36", what: "launch the PhotoCore Android emulator", runnable: false },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_java_versions() {
        assert_eq!(java_major("openjdk version \"17.0.20.1\" 2026-08-18"), Some(17));
        assert_eq!(java_major("javac 21.0.1"), Some(21));
        assert_eq!(java_major("java version \"1.8.0_402\""), Some(8));
        assert_eq!(java_major("nothing"), None);
    }

    #[test]
    fn picks_the_newest_stable_ndk() {
        let list = "  ndk;29.0.14206865 | 29.0.14206865     | NDK\n  ndk;30.0.16138531 | 30.0.16138531 rc3 | NDK\n  ndk;30.0.16248370 | 30.0.16248370     | NDK\n  ndk;23.1.7779620 | 23.1.7779620 | NDK\n";
        assert_eq!(latest_stable_ndk(list).as_deref(), Some("ndk;30.0.16248370"));
        assert_eq!(latest_stable_ndk("platforms;android-36 | 1 | x"), None);
    }

    #[test]
    fn finds_versioned_sdkmanager_installations() -> Result<(), String> {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos();
        let root = std::env::temp_dir().join(format!("projectile-sdkmanager-test-{unique}"));
        let suffix = if cfg!(windows) { ".bat" } else { "" };
        let program = root.join("cmdline-tools/15.0/bin").join(format!("sdkmanager{suffix}"));
        std::fs::create_dir_all(program.parent().ok_or("sdkmanager path has no parent")?).map_err(|error| error.to_string())?;
        std::fs::write(&program, b"").map_err(|error| error.to_string())?;
        let found = sdkmanager_tool(Some(&root));
        let _ = std::fs::remove_dir_all(&root);
        if found.as_deref() != Some(program.as_path()) {
            return Err("versioned command-line tools sdkmanager was not found".into());
        }
        Ok(())
    }

    #[test]
    fn useful_commands_are_documented() {
        for list in [RUST_COMMANDS, RUSTUP_COMMANDS, PHOTOCORE_COMMANDS, ANDROID_COMMANDS] {
            assert!(list.iter().all(|c| !c.command.is_empty() && !c.what.is_empty()));
        }
        for command in [
            "javac -version",
            "gradle --version",
            "sdkmanager --version",
            "cargo install cargo-ndk",
            "sh android/build-rust.sh",
            "adb install",
            "sdkmanager --install",
            "avdmanager create avd",
            "emulator -avd",
        ] {
            assert!(ANDROID_COMMANDS.iter().any(|item| item.command.starts_with(command)), "missing Android command: {command}");
        }
    }
}
