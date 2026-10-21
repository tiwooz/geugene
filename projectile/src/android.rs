//! The Android build option: finds the Android SDK, picks API levels, and writes a Gradle (Java)
//! project plus a Rust `cdylib` crate into the generated project.

use std::path::{Path, PathBuf};

/// Google Play requires new apps and updates to target this API level (Android 16) since
/// 31 August 2026 (Play Console Help, "Target API level requirements").
pub const PLAY_TARGET_SDK: u32 = 36;
/// Android 8.0. wgpu's Vulkan backend needs API 24 or newer; 26 leaves headroom and covers
/// nearly all devices in use.
pub const DEFAULT_MIN_SDK: u32 = 26;
pub const LOWEST_MIN_SDK: u32 = 24;
pub const HIGHEST_SDK: u32 = 40;
pub const JAVA_VERSIONS: &[u32] = &[17, 21, 25];
pub const GRADLE_VERSIONS: &[&str] = &["9.6.0", "9.8.1"];

fn has_sdk_contents(path: &Path) -> bool {
    ["platforms", "platform-tools", "build-tools", "ndk", "cmdline-tools"].iter().any(|name| path.join(name).is_dir())
}

#[derive(Debug, Clone, Default)]
pub struct Sdk {
    pub root: Option<PathBuf>,
    /// Installed `platforms/android-N`, ascending.
    pub platforms: Vec<u32>,
    /// Installed `ndk/<version>` folder names.
    pub ndks: Vec<String>,
    pub build_tools: Vec<String>,
}

impl Sdk {
    pub fn detect() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let candidates = [
            std::env::var_os("ANDROID_HOME").map(PathBuf::from),
            std::env::var_os("ANDROID_SDK_ROOT").map(PathBuf::from),
            home.as_ref().map(|h| h.join("Android/Sdk")),
            home.as_ref().map(|h| h.join("Library/Android/sdk")),
            std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("Android/Sdk")),
        ];
        let root = candidates.into_iter().flatten().find(|p| has_sdk_contents(p));
        match root {
            Some(root) => Self::scan(&root),
            None => Self::default(),
        }
    }

    pub fn scan(root: &Path) -> Self {
        let names = |dir: &str| -> Vec<String> {
            let mut out: Vec<String> =
                std::fs::read_dir(root.join(dir)).into_iter().flatten().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            out.sort();
            out
        };
        let mut platforms: Vec<u32> = names("platforms").iter().filter_map(|n| n.strip_prefix("android-")?.parse().ok()).collect();
        platforms.sort_unstable();
        Self { root: Some(root.to_path_buf()), platforms, ndks: names("ndk"), build_tools: names("build-tools") }
    }

    /// The target ProjectIle recommends: Google Play's required level, or the newest installed
    /// platform when that is newer.
    pub fn recommended_target(&self) -> u32 {
        self.platforms.last().copied().unwrap_or(PLAY_TARGET_SDK).clamp(PLAY_TARGET_SDK, HIGHEST_SDK)
    }

    pub fn has_platform(&self, level: u32) -> bool {
        self.platforms.contains(&level)
    }
}

/// The levels to use: the user's, or ProjectIle's picks when left at 0 ("Automatic").
pub fn resolve(min: u32, target: u32, sdk: &Sdk) -> (u32, u32) {
    let target = if target == 0 { sdk.recommended_target() } else { target };
    let min = if min == 0 { DEFAULT_MIN_SDK } else { min };
    (min, target)
}

pub fn validate(min: u32, target: u32) -> Result<(), String> {
    if !(LOWEST_MIN_SDK..=HIGHEST_SDK).contains(&min) {
        return Err(format!("Android minimum API must be between {LOWEST_MIN_SDK} and {HIGHEST_SDK} (wgpu's Vulkan backend needs {LOWEST_MIN_SDK}+)."));
    }
    if !(LOWEST_MIN_SDK..=HIGHEST_SDK).contains(&target) || target < min {
        return Err(format!("Android target API must be between the minimum ({min}) and {HIGHEST_SDK}."));
    }
    Ok(())
}

/// The Java package for the app: `ai.storyteller.photocore.<name without dashes>`.
pub fn java_package(package: &str) -> String {
    let clean: String = package.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    let clean = if clean.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) { clean } else { format!("app{clean}") };
    format!("ai.storyteller.photocore.{clean}")
}

/// Files of the Android build, relative to the project root.
pub fn files(package: &str, min: u32, target: u32, java_version: u32, gradle: &str) -> Vec<(String, String)> {
    let java = java_package(package);
    let java_path = java.replace('.', "/");
    let compile_sdk = target.max(PLAY_TARGET_SDK);
    vec![
        (
            "android/settings.gradle".into(),
            format!(
                "pluginManagement {{\n    repositories {{\n        google()\n        mavenCentral()\n        gradlePluginPortal()\n    }}\n}}\ndependencyResolutionManagement {{\n    repositories {{\n        google()\n        mavenCentral()\n    }}\n}}\nrootProject.name = \"{package}\"\ninclude ':app'\n"
            ),
        ),
        ("android/build.gradle".into(), "plugins {\n    id 'com.android.application' version '9.4.1' apply false\n}\n".into()),
        ("android/gradle.properties".into(), "org.gradle.jvmargs=-Xmx2g\nandroid.useAndroidX=true\n".into()),
        (
            "android/gradle/wrapper/gradle-wrapper.properties".into(),
            format!(
                "distributionBase=GRADLE_USER_HOME\ndistributionPath=wrapper/dists\nzipStoreBase=GRADLE_USER_HOME\nzipStorePath=wrapper/dists\ndistributionUrl=https\\://services.gradle.org/distributions/gradle-{gradle}-bin.zip\n"
            ),
        ),
        (
            "android/app/build.gradle".into(),
            format!(
                "plugins {{\n    id 'com.android.application'\n}}\n\nandroid {{\n    namespace '{java}'\n    compileSdk {compile_sdk}\n\n    defaultConfig {{\n        applicationId '{java}'\n        minSdk {min}\n        targetSdk {target}\n        versionCode 1\n        versionName '0.1'\n        ndk {{\n            abiFilters 'arm64-v8a', 'x86_64'\n        }}\n    }}\n\n    compileOptions {{\n        sourceCompatibility JavaVersion.VERSION_17\n        targetCompatibility JavaVersion.VERSION_17\n    }}\n\n    // The Rust libraries built by build-rust.sh land here.\n    sourceSets {{\n        main {{\n            jniLibs.srcDirs = ['src/main/jniLibs']\n        }}\n    }}\n}}\n"
            ),
        ),
        (
            "android/app/src/main/AndroidManifest.xml".into(),
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">\n    <uses-feature android:name=\"android.hardware.vulkan.version\" android:version=\"0x400003\" android:required=\"false\" />\n    <application android:label=\"PhotoCore\" android:hasCode=\"true\" android:theme=\"@style/PhotoCoreTheme\">\n        <activity android:name=\".MainActivity\" android:exported=\"true\" android:screenOrientation=\"portrait\" android:configChanges=\"orientation|screenSize|screenLayout|keyboardHidden|uiMode\">\n            <meta-data android:name=\"android.app.lib_name\" android:value=\"photocraft_android\" />\n            <intent-filter>\n                <action android:name=\"android.intent.action.MAIN\" />\n                <category android:name=\"android.intent.category.LAUNCHER\" />\n            </intent-filter>\n        </activity>\n    </application>\n</manifest>\n".into(),
        ),
        (
            format!("android/app/src/main/java/{java_path}/MainActivity.java"),
            format!(
                "package {java};\n\nimport android.app.NativeActivity;\nimport android.os.Bundle;\nimport android.view.View;\nimport android.view.WindowInsets;\nimport android.view.WindowInsetsController;\n\n/**\n * Starts the Rust editor: NativeActivity loads libphotocraft_android.so (named in\n * AndroidManifest.xml) and calls its android_main. System bars start hidden, but Android\n * reveals them transiently when the user swipes from the corresponding screen edge.\n */\npublic class MainActivity extends NativeActivity {{\n    static {{\n        System.loadLibrary(\"photocraft_android\");\n    }}\n\n    @Override\n    public void onCreate(Bundle state) {{\n        super.onCreate(state);\n        hideSystemBars();\n    }}\n\n    @Override\n    public void onWindowFocusChanged(boolean focused) {{\n        super.onWindowFocusChanged(focused);\n        if (focused) {{\n            hideSystemBars();\n        }}\n    }}\n\n    private void hideSystemBars() {{\n        WindowInsetsController controller = getWindow().getInsetsController();\n        if (controller != null) {{\n            controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);\n            controller.hide(WindowInsets.Type.statusBars() | WindowInsets.Type.navigationBars());\n        }} else {{\n            getWindow().getDecorView().setSystemUiVisibility(\n                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY\n                    | View.SYSTEM_UI_FLAG_FULLSCREEN\n                    | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION\n                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN\n                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION\n                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE);\n        }}\n    }}\n}}\n"
            ),
        ),
        (
            "android/app/src/main/res/values/styles.xml".into(),
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n    <style name=\"PhotoCoreTheme\" parent=\"android:style/Theme.Material.NoActionBar\">\n        <item name=\"android:windowActionModeOverlay\">true</item>\n        <item name=\"android:windowNoTitle\">true</item>\n        <item name=\"android:fontFamily\">sans</item>\n    </style>\n</resources>\n".into(),
        ),
        (
            "android/build-rust.sh".into(),
            "#!/bin/sh\n# Builds the Rust editor for Android phones (arm64) and x86_64 emulators.\n# Needs: rustup targets aarch64-linux-android and x86_64-linux-android; cargo-ndk; Android NDK.\nset -e\ncd \"$(dirname \"$0\")/..\"\ncargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build -p photocraft-android --release\n".into(),
        ),
        (
            "android/README.md".into(),
            format!(
                r#"# Android build (experimental)

A Gradle (Java) project that wraps the Rust editor in a `NativeActivity`.
minSdk {min}, targetSdk {target}, compileSdk {compile_sdk}; JDK {java_version}; Gradle {gradle}.
The Android activity is locked to portrait orientation; the emulator/device sets the actual pixel resolution.

## Install and check tools

Install [Eclipse Temurin JDK {java_version}](https://adoptium.net/temurin/releases/?version={java_version}) and
[Gradle {gradle}](https://gradle.org/releases/). Install the Android SDK
[command-line tools](https://developer.android.com/studio#command-line-tools-only), then accept
SDK licences with `sdkmanager --licenses`. Install the API {compile_sdk} platform and build tools,
Android platform tools (`adb`), and an NDK r25 or newer with `sdkmanager` or Android Studio's
SDK Manager. Set `ANDROID_HOME`/`ANDROID_SDK_ROOT` and `ANDROID_NDK_HOME` if they are not found
automatically.

Check the installations with `javac -version`, `gradle --version`, `sdkmanager --list`,
`adb version`, and `rustup show`. The Gradle distribution is pinned in
`android/gradle/wrapper/gradle-wrapper.properties`. Bootstrap the wrapper scripts with the
installed Gradle, then build with the pinned wrapper:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
sh android/build-rust.sh
cd android
gradle wrapper --gradle-version {gradle} --distribution-type bin
./gradlew assembleDebug
```

The selected JDK is the Gradle runtime; the Android Java source/bytecode target stays at 17.
The debug APK is in `android/app/build/outputs/apk/debug/`. See the Android
[SDK manager guide](https://developer.android.com/tools/sdkmanager),
[NDK downloads](https://developer.android.com/ndk/downloads), and [adb guide](https://developer.android.com/tools/adb).
PhotoCraft's automatic UI scale fits to the Android display width with a readability floor;
choosing a fixed UI scale in Preferences overrides the automatic fit.

## Create and launch an emulator

This APK includes arm64 libraries for phones and x86_64 libraries for the standard Linux emulator.
Install an API {compile_sdk} Google APIs x86_64 image and create an AVD once:

```sh
sdkmanager --install "system-images;android-{compile_sdk};google_apis;x86_64"
avdmanager create avd --name PhotoCore_API_{compile_sdk} --package "system-images;android-{compile_sdk};google_apis;x86_64"
```

Launch the emulator in another terminal, wait for Android to finish booting, then install and start
PhotoCore from the project root:

```sh
emulator -avd PhotoCore_API_{compile_sdk}
```

```sh
adb wait-for-device
adb shell getprop sys.boot_completed
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
adb shell monkey -p {java} 1
adb logcat -s RustStdoutStderr:V
```

If `sys.boot_completed` is not `1` yet, wait and run it again. To list or stop emulators, use
`emulator -list-avds` and `adb emu kill`. The old Lenovo_S850_Clone AVD runs Android 4.4 (API 19),
below this app's minimum API 24; create and use the new API {compile_sdk} AVD instead.

PhotoCraft is a desktop editor first: it has not been verified on Android yet, so the Rust
step may need porting work (file dialogs, window handling). `apps/photocraft-android` is where
that starts.
"#
            ),
        ),
        (
            "apps/photocraft-android/Cargo.toml".into(),
            "[package]\nname = \"photocraft-android\"\ndescription = \"Android entry point for this PhotoCore project (experimental)\"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\npublish = false\n\n# The Android entry point must be an exported symbol, which Rust counts as unsafe code.\n# It is the only exception, so this crate denies (rather than forbids) unsafe code.\n[lints.rust]\nunsafe_code = \"deny\"\n\n[lib]\nname = \"photocraft_android\"\ncrate-type = [\"cdylib\"]\n\n[target.'cfg(target_os = \"android\")'.dependencies]\neframe = { workspace = true, features = [\"wgpu\", \"default_fonts\", \"android-native-activity\"] }\nwinit = { version = \"0.30\", default-features = false, features = [\"android-native-activity\"] }\nphotocraft-engine = { workspace = true }\nphotocraft-ui-egui = { workspace = true }\n".into(),
        ),
        (
            "apps/photocraft-android/src/lib.rs".into(),
            "//! Android entry point. On other platforms this crate is empty.\n\n#[cfg(target_os = \"android\")]\nuse winit::platform::android::activity::AndroidApp;\n\n// SAFETY: `android_main` must be exported unmangled so Android's NativeActivity glue can call\n// it; nothing else in the program defines a symbol with this name.\n#[cfg(target_os = \"android\")]\n#[allow(unsafe_code)]\n#[unsafe(no_mangle)]\nfn android_main(app: AndroidApp) {\n    let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };\n    let _ = eframe::run_native(\n        \"PhotoCore\",\n        options,\n        Box::new(|_cc| {\n            let mut editor = photocraft_ui_egui::PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());\n            // Keep the document view untouched. The icon rail remains available to reveal the\n            // dock on demand, while the wide dock starts closed on a phone-sized display.\n            editor.ui.panels.dock = false;\n            editor.ui.panels.rail = true;\n            Ok(Box::new(editor))\n        }),\n    );\n}\n".into(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_play_target_and_safe_minimum() {
        let sdk = Sdk { platforms: vec![19, 33, 34, 36], ..Sdk::default() };
        assert_eq!(resolve(0, 0, &sdk), (DEFAULT_MIN_SDK, 36));
        assert_eq!(resolve(28, 35, &sdk), (28, 35));
        let old = Sdk { platforms: vec![30], ..Sdk::default() };
        assert_eq!(old.recommended_target(), PLAY_TARGET_SDK, "never recommends below Play's requirement");
        assert!(validate(26, 36).is_ok());
        assert!(validate(10, 36).is_err() && validate(30, 28).is_err() && validate(26, 99).is_err());
    }

    #[test]
    fn java_packages_are_valid_identifiers() {
        assert_eq!(java_package("my-photocore"), "ai.storyteller.photocore.myphotocore");
        assert_eq!(java_package("9-lives"), "ai.storyteller.photocore.app9lives");
        let files = files("my-photocore", 26, 36, 17, "9.6.0");
        assert!(files.iter().any(|(path, text)| path == "android/app/build.gradle" && text.contains("minSdk 26") && text.contains("targetSdk 36")));
        assert!(files.iter().any(|(path, text)| path == "android/app/src/main/AndroidManifest.xml" && text.contains("android:screenOrientation=\"portrait\"")));
        assert!(files.iter().any(|(path, _)| path.ends_with("myphotocore/MainActivity.java")));
    }

    #[test]
    fn detects_an_sdk_root_with_only_command_line_tools() -> Result<(), String> {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos();
        let root = std::env::temp_dir().join(format!("projectile-sdk-test-{unique}"));
        std::fs::create_dir_all(root.join("cmdline-tools/latest/bin")).map_err(|error| error.to_string())?;
        let has_root = has_sdk_contents(&root);
        let _ = std::fs::remove_dir_all(&root);
        if !has_root {
            return Err("command-line tools alone should identify an Android SDK root".into());
        }
        Ok(())
    }

    #[test]
    fn android_files_pin_the_selected_java_and_gradle_versions() {
        let generated = files("my-app", 26, 36, 21, "9.6.0");
        let low_target = files("my-app", 24, 24, 17, "9.6.0");
        let gradle = generated.iter().find(|(path, _)| path == "android/app/build.gradle").map(|(_, body)| body);
        let wrapper = generated.iter().find(|(path, _)| path == "android/gradle/wrapper/gradle-wrapper.properties").map(|(_, body)| body);
        let readme = generated.iter().find(|(path, _)| path == "android/README.md").map(|(_, body)| body);
        let manifest = generated.iter().find(|(path, _)| path == "apps/photocraft-android/Cargo.toml").map(|(_, body)| body);
        let entry = generated.iter().find(|(path, _)| path == "apps/photocraft-android/src/lib.rs").map(|(_, body)| body);
        let build_script = generated.iter().find(|(path, _)| path == "android/build-rust.sh").map(|(_, body)| body);
        assert!(gradle.is_some_and(|s| s.contains("JavaVersion.VERSION_17")
            && s.contains("compileSdk 36")
            && s.contains("targetSdk 36")
            && s.contains("'arm64-v8a', 'x86_64'")));
        assert!(wrapper.is_some_and(|s| s.contains("gradle-9.6.0-bin.zip")));
        assert!(readme.is_some_and(|s| s.contains("JDK 21")
            && s.contains("Gradle 9.6.0")
            && s.contains("avdmanager create avd")
            && s.contains("emulator -avd PhotoCore_API_36")));
        assert!(manifest.is_some_and(|s| s.contains("winit = { version = \"0.30\"") && s.contains("android-native-activity")));
        assert!(entry.is_some_and(|s| s.contains("use winit::platform::android::activity::AndroidApp;") && !s.contains("eframe::egui_winit")));
        assert!(build_script.is_some_and(|s| s.contains("-t arm64-v8a -t x86_64")));
        assert!(low_target.iter().any(|(path, body)| path == "android/app/build.gradle" && body.contains("compileSdk 36") && body.contains("targetSdk 24")));
    }
}
