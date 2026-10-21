//! What system ProjectIle runs on: operating system, Linux distribution, desktop, package
//! manager. Drives the status-bar indicator, the layout ProjectIle picks, and the install commands
//! it suggests (it never runs `sudo` itself).

use crate::theme::Palette;
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub struct System {
    /// `linux`, `macos`, `windows`, …
    pub os: &'static str,
    /// os-release `ID` (`arch`, `ubuntu`, `fedora`, …); empty off Linux.
    pub distro_id: String,
    /// os-release `ID_LIKE` (`arch`, `debian`, …).
    pub distro_like: String,
    /// Human name: "Arch Linux", "macOS", "Windows".
    pub name: String,
    /// `XDG_CURRENT_DESKTOP` (GNOME, KDE, Hyprland, …), if any.
    pub desktop: Option<String>,
    pub package_manager: Option<PackageManager>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Pacman,
    Apt,
    Dnf,
    Zypper,
    Brew,
    Winget,
}

impl PackageManager {
    /// The install command for `what`, for the user to run in a terminal.
    pub fn install(self, what: Tool) -> Option<String> {
        let package = match (self, what) {
            (Self::Pacman, Tool::Rustup) => "rustup",
            (Self::Pacman, Tool::Gh) => "github-cli",
            (Self::Pacman, Tool::Git) => "git",
            (Self::Apt, Tool::Rustup) => "rustup",
            (Self::Apt, Tool::Gh) => "gh",
            (Self::Apt, Tool::Git) => "git",
            (Self::Dnf, Tool::Rustup) => "rustup",
            (Self::Dnf, Tool::Gh) => "gh",
            (Self::Dnf, Tool::Git) => "git",
            (Self::Zypper, Tool::Rustup) => "rustup",
            (Self::Zypper, Tool::Gh) => "gh",
            (Self::Zypper, Tool::Git) => "git",
            (Self::Brew, Tool::Rustup) => "rustup",
            (Self::Brew, Tool::Gh) => "gh",
            (Self::Brew, Tool::Git) => "git",
            (Self::Winget, Tool::Rustup) => "Rustlang.Rustup",
            (Self::Winget, Tool::Gh) => "GitHub.cli",
            (Self::Winget, Tool::Git) => "Git.Git",
        };
        Some(match self {
            Self::Pacman => format!("sudo pacman -S {package}"),
            Self::Apt => format!("sudo apt install {package}"),
            Self::Dnf => format!("sudo dnf install {package}"),
            Self::Zypper => format!("sudo zypper install {package}"),
            Self::Brew => format!("brew install {package}"),
            Self::Winget => format!("winget install {package}"),
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Pacman => "pacman",
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Zypper => "zypper",
            Self::Brew => "Homebrew",
            Self::Winget => "winget",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Rustup,
    Gh,
    Git,
}

/// How dialogs and spacing are arranged. `Auto` follows the detected desktop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutChoice {
    #[default]
    Auto,
    /// Main action on the right of dialogs (GNOME, macOS).
    ActionRight,
    /// Main action on the left of dialogs (KDE Plasma, Windows).
    ActionLeft,
}

impl LayoutChoice {
    pub const ALL: [LayoutChoice; 3] = [LayoutChoice::Auto, LayoutChoice::ActionRight, LayoutChoice::ActionLeft];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatic",
            Self::ActionRight => "Main action on the right",
            Self::ActionLeft => "Main action on the left",
        }
    }
}

/// The layout actually in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Dialogs put their main button last (right) instead of first.
    pub action_last: bool,
    /// Tighter spacing, for tiling window managers where windows get less room.
    pub compact: bool,
}

impl System {
    pub fn detect() -> Self {
        let os = std::env::consts::OS;
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok().filter(|d| !d.trim().is_empty());
        match os {
            "linux" => {
                let release = std::fs::read_to_string("/etc/os-release").or_else(|_| std::fs::read_to_string("/usr/lib/os-release")).unwrap_or_default();
                Self::from_os_release(&release, desktop)
            }
            "macos" => {
                Self { os, name: "macOS".into(), desktop: Some("Aqua".into()), package_manager: has("brew").then_some(PackageManager::Brew), ..Self::default() }
            }
            "windows" => {
                Self { os, name: "Windows".into(), desktop: None, package_manager: has("winget").then_some(PackageManager::Winget), ..Self::default() }
            }
            other => Self { os: "other", name: other.to_owned(), desktop, ..Self::default() },
        }
    }

    /// Parse `/etc/os-release` (a list of `KEY=value` lines, values optionally quoted).
    pub fn from_os_release(release: &str, desktop: Option<String>) -> Self {
        let value = |key: &str| {
            release
                .lines()
                .find_map(|line| line.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')))
                .map(|v| v.trim().trim_matches('"').trim_matches('\'').to_owned())
                .unwrap_or_default()
        };
        let distro_id = value("ID");
        let distro_like = value("ID_LIKE");
        let name =
            Some(value("PRETTY_NAME")).filter(|n| !n.is_empty()).or_else(|| Some(value("NAME")).filter(|n| !n.is_empty())).unwrap_or_else(|| "Linux".into());
        let family = |ids: &[&str]| ids.iter().any(|id| distro_id == *id || distro_like.split_whitespace().any(|like| like == *id));
        let package_manager = if family(&["arch"]) {
            Some(PackageManager::Pacman)
        } else if family(&["debian", "ubuntu"]) {
            Some(PackageManager::Apt)
        } else if family(&["fedora", "rhel", "centos"]) {
            Some(PackageManager::Dnf)
        } else if family(&["suse", "opensuse"]) {
            Some(PackageManager::Zypper)
        } else {
            None
        };
        Self { os: "linux", distro_id, distro_like, name, desktop, package_manager }
    }

    pub fn is_arch(&self) -> bool {
        self.distro_id == "arch" || self.distro_like.split_whitespace().any(|like| like == "arch")
    }

    /// Tiling window managers arrange windows themselves and usually give each less room.
    pub fn tiling(&self) -> bool {
        self.desktop.as_deref().is_some_and(|d| {
            let d = d.to_ascii_lowercase();
            ["hyprland", "sway", "i3", "bspwm", "river", "niri", "awesome", "dwm", "qtile", "xmonad"].iter().any(|wm| d.contains(wm))
        })
    }

    /// The layout `choice` resolves to here, and why.
    pub fn layout(&self, choice: LayoutChoice) -> (Layout, String) {
        let desktop = self.desktop.clone().unwrap_or_default().to_ascii_lowercase();
        let compact = self.tiling();
        let (action_last, reason) = match choice {
            LayoutChoice::ActionRight => (true, "your choice".to_owned()),
            LayoutChoice::ActionLeft => (false, "your choice".to_owned()),
            LayoutChoice::Auto if desktop.contains("kde") || self.os == "windows" => (false, "KDE Plasma and Windows put the main action first".to_owned()),
            LayoutChoice::Auto if desktop.contains("gnome") || self.os == "macos" => (true, "GNOME and macOS put the main action last".to_owned()),
            LayoutChoice::Auto => (true, "the common Linux convention puts the main action last".to_owned()),
        };
        let mut why = reason;
        if compact {
            why.push_str("; compact spacing for a tiling window manager");
        }
        (Layout { action_last, compact }, why)
    }

    /// The colour of the system's own logo, for the status-bar dot.
    pub fn brand_color(&self, p: &Palette) -> Color32 {
        match (self.os, self.distro_id.as_str()) {
            (_, "arch" | "endeavouros" | "manjaro" | "cachyos" | "garuda") => Color32::from_rgb(23, 147, 209),
            (_, "ubuntu") => Color32::from_rgb(233, 84, 32),
            (_, "fedora") => Color32::from_rgb(81, 162, 218),
            (_, "debian") => Color32::from_rgb(215, 10, 83),
            (_, "opensuse-tumbleweed" | "opensuse-leap") => Color32::from_rgb(115, 186, 37),
            ("windows", _) => Color32::from_rgb(0, 120, 212),
            _ => p.text_dim,
        }
    }

    /// "Arch Linux · Hyprland".
    pub fn summary(&self) -> String {
        match &self.desktop {
            Some(desktop) => format!("{} · {desktop}", self.name),
            None => self.name.clone(),
        }
    }
}

fn has(program: &str) -> bool {
    crate::toolchain::find_program(program).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_arch_and_derivatives() {
        let arch = System::from_os_release("NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\n", Some("Hyprland".into()));
        assert!(arch.is_arch() && arch.tiling());
        assert_eq!(arch.package_manager, Some(PackageManager::Pacman));
        assert_eq!(arch.summary(), "Arch Linux · Hyprland");
        assert_eq!(arch.package_manager.and_then(|pm| pm.install(Tool::Gh)).as_deref(), Some("sudo pacman -S github-cli"));
        let endeavour = System::from_os_release("ID=endeavouros\nID_LIKE=arch\nPRETTY_NAME='EndeavourOS'", None);
        assert!(endeavour.is_arch());
        let ubuntu = System::from_os_release("ID=ubuntu\nID_LIKE=debian\nPRETTY_NAME=\"Ubuntu 26.04 LTS\"", Some("ubuntu:GNOME".into()));
        assert_eq!(ubuntu.package_manager, Some(PackageManager::Apt));
        assert!(ubuntu.layout(LayoutChoice::Auto).0.action_last && !ubuntu.tiling());
        let kde = System::from_os_release("ID=fedora", Some("KDE".into()));
        assert!(!kde.layout(LayoutChoice::Auto).0.action_last);
        assert!(kde.layout(LayoutChoice::ActionRight).0.action_last);
    }

    #[test]
    fn empty_os_release_is_harmless() {
        let unknown = System::from_os_release("", None);
        assert_eq!(unknown.name, "Linux");
        assert!(unknown.package_manager.is_none() && !unknown.is_arch());
    }
}
