use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::cli::Cli;
use crate::error::ConfigError;
use crate::keymap::{self, KeyAction};

pub const DEFAULT_SCREENSHOT_DIR: &str = "~/Desktop/Screenshots";
pub const CONFIG_DIRECTORY_ENV: &str = "XDG_CONFIG_HOME";
pub const CONFIG_SUBPATH: &str = "shmooz/config.toml";
pub const KEYBINDINGS_SECTION: &str = "keybindings";
pub const SCROLL_DIRECTION_OPTION: &str = "scroll_direction";
pub const LAYOUT_OPTION: &str = "layout";

/// Which way the wheel must be pushed for a zoom in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollDirection {
    Down,
    /// Built-in default (since vNext): wheel up magnifies; use "down" to restore
    /// the old behavior.
    #[default]
    Up,
}

impl ScrollDirection {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "down" => Some(Self::Down),
            "up" => Some(Self::Up),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub keybindings: BTreeMap<KeyAction, Vec<u32>>,
    pub initial_zoom: f64,
    pub output_filter: Option<String>,
    pub scroll_direction: ScrollDirection,
    pub spotlight: bool,
    pub screenshot_dir: PathBuf,
    pub show_indicator: bool,
}

impl Config {
    /// The action bound to an evdev key code, if any. On a binding conflict
    /// the earlier action in declaration order wins (Close is declared
    /// first, so an explicit close key keeps its exit semantics).
    pub fn key_action(&self, key_code: u32) -> Option<KeyAction> {
        self.keybindings
            .iter()
            .find(|(_, keys)| keys.contains(&key_code))
            .map(|(action, _)| *action)
    }

    /// Human-readable label for the close key, e.g. "Esc" or "Q".
    pub fn close_label(&self) -> String {
        self.keybindings
            .get(&KeyAction::Close)
            .and_then(|keys| keys.first())
            .map(|code| keymap::key_label(*code).to_owned())
            .unwrap_or_else(|| "Esc".to_owned())
    }
}

impl TryFrom<Cli> for Config {
    type Error = ConfigError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        let home_dir = std::env::var_os("HOME").map(PathBuf::from);
        let initial_zoom = cli
            .zoom_in
            .as_deref()
            .map(parse_zoom)
            .transpose()?
            .unwrap_or(0.0);
        let screenshot_dir = resolve_screenshot_dir(
            cli.screenshot_dir
                .as_deref()
                .unwrap_or(DEFAULT_SCREENSHOT_DIR),
            home_dir.as_deref(),
        )?;

        let mut file_only = std::collections::BTreeMap::new();
        let mut loaded = LoadedFile::default();
        if let Some(path) = config_path(cli.config.as_deref()) {
            if path.is_file() {
                loaded = merge_keybindings_file(&path, &mut file_only)?;
            } else if cli.config.is_some() {
                return Err(ConfigError::ConfigFileMissing {
                    path: path.display().to_string(),
                });
            }
        }

        let layout = loaded.layout.unwrap_or_default();
        let mut keybindings = keymap::default_bindings_layout(layout);
        for (action, codes) in file_only {
            keybindings.insert(action, codes);
        }

        if let Some(raw) = cli.map_close.as_deref() {
            let code = keymap::key_code(raw).ok_or_else(|| ConfigError::CloseKey {
                value: raw.to_owned(),
            })?;
            // map-close overrides every file and layout default
            keybindings.insert(KeyAction::Close, vec![code]);
        }

        let scroll_direction = match (&cli.scroll_direction, cli.invert_scroll) {
            (Some(raw), _) => ScrollDirection::parse(raw)
                .ok_or_else(|| ConfigError::InvalidScrollDirection { value: raw.clone() })?,
            (None, true) => ScrollDirection::Up,
            (None, false) => loaded.scroll.unwrap_or_default(),
        };

        Ok(Self {
            keybindings,
            initial_zoom,
            output_filter: cli.output,
            scroll_direction,
            spotlight: cli.spotlight,
            screenshot_dir,
            show_indicator: !cli.no_indicator,
        })
    }
}

/// Resolves the config file path: the CLI flag wins, then XDG_CONFIG_HOME
/// (falling back to HOME/.config), otherwise no file is consulted.
pub fn config_path(cli_override: Option<&str>) -> Option<PathBuf> {
    cli_override.map(PathBuf::from).or_else(default_config_path)
}

fn default_config_path() -> Option<PathBuf> {
    let base = std::env::var_os(CONFIG_DIRECTORY_ENV)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join(CONFIG_SUBPATH))
}

/// Applies the [keybindings] section of a config file on top of the current
/// bindings. Each action may be bound to a single key name or an array of
/// key names; an empty array disables the action.
/// Contents of a config file as it pertains to startup defaults.
#[derive(Default)]
struct LoadedFile {
    scroll: Option<ScrollDirection>,
    layout: Option<keymap::Layout>,
}

fn merge_keybindings_file(
    path: &Path,
    keybindings: &mut BTreeMap<KeyAction, Vec<u32>>,
) -> Result<LoadedFile, ConfigError> {
    let path_text = path.display().to_string();
    let raw = std::fs::read_to_string(path).map_err(|err| ConfigError::ConfigFileRead {
        path: path_text.clone(),
        message: err.to_string(),
    })?;
    let table: toml::Table = toml::from_str(&raw).map_err(|err| ConfigError::ConfigFileParse {
        path: path_text.clone(),
        message: err.to_string(),
    })?;

    let mut file_scroll = None;
    let mut file_layout = None;
    for (option, _) in &table {
        if option == KEYBINDINGS_SECTION
            || option == SCROLL_DIRECTION_OPTION
            || option == LAYOUT_OPTION
        {
            continue;
        }
        return Err(ConfigError::UnknownConfigKey {
            path: path_text.clone(),
            value: option.clone(),
        });
    }

    if let Some(value) = table.get(SCROLL_DIRECTION_OPTION) {
        let text = value.as_str().unwrap_or_default();
        let parsed = ScrollDirection::parse(text);
        let direction = parsed.ok_or_else(|| ConfigError::InvalidScrollDirection {
            value: match value.as_str() {
                Some(raw) => raw.to_owned(),
                None => String::from("expected down or up"),
            },
        })?;
        file_scroll = Some(direction);
    }

    if let Some(value) = table.get(LAYOUT_OPTION) {
        let text = value.as_str().unwrap_or_default();
        file_layout =
            Some(
                keymap::Layout::parse(text).ok_or_else(|| ConfigError::InvalidLayout {
                    value: match value.as_str() {
                        Some(raw) => raw.to_owned(),
                        None => String::from("expected qwerty or dvorak"),
                    },
                })?,
            );
    }

    let loaded = LoadedFile {
        scroll: file_scroll,
        layout: file_layout,
    };

    let Some(bindings) = table.get(KEYBINDINGS_SECTION) else {
        return Ok(loaded);
    };
    let bindings = bindings
        .as_table()
        .ok_or_else(|| ConfigError::KeyBindingValue {
            path: path_text.clone(),
            key: KEYBINDINGS_SECTION.to_owned(),
            value: "expected a table".to_owned(),
        })?;

    for (name, binding) in bindings {
        let action =
            KeyAction::from_config_name(name).ok_or_else(|| ConfigError::KeyBindingAction {
                value: name.clone(),
                known: KeyAction::known_actions().join(", "),
            })?;
        let names = match binding {
            toml::Value::String(name) => vec![name.as_str()],
            toml::Value::Array(items) => {
                let mut names = Vec::with_capacity(items.len());
                for item in items {
                    let name = item.as_str().ok_or_else(|| ConfigError::KeyBindingValue {
                        path: path_text.clone(),
                        key: name.clone(),
                        value: "expected key name strings".to_owned(),
                    })?;
                    names.push(name);
                }
                names
            }
            _ => {
                return Err(ConfigError::KeyBindingValue {
                    path: path_text.clone(),
                    key: name.clone(),
                    value: "expected a key name or an array of key names".to_owned(),
                });
            }
        };
        let mut codes = Vec::with_capacity(names.len());
        for key_name in names {
            codes.push(
                keymap::key_code(key_name).ok_or_else(|| ConfigError::KeyBindingKey {
                    action: name.clone(),
                    value: key_name.to_owned(),
                })?,
            );
        }
        keybindings.insert(action, codes);
    }

    Ok(loaded)
}

fn parse_zoom(raw: &str) -> Result<f64, ConfigError> {
    let trimmed = raw.trim();
    let (value, is_percent) = match trimmed.strip_suffix('%') {
        Some(value) => (value, true),
        None => (trimmed, false),
    };

    let parsed = value.parse::<f64>().map_err(|_| ConfigError::Zoom {
        value: raw.to_owned(),
    })?;

    let normalized = if is_percent { parsed / 100.0 } else { parsed };

    if !(0.0..1.0).contains(&normalized) {
        return Err(ConfigError::Zoom {
            value: raw.to_owned(),
        });
    }

    Ok(normalized)
}

fn resolve_screenshot_dir(raw: &str, home_dir: Option<&Path>) -> Result<PathBuf, ConfigError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::ScreenshotDirectory {
            value: raw.to_owned(),
        });
    }

    if trimmed == "~" {
        return home_dir.map(Path::to_path_buf).ok_or_else(|| {
            ConfigError::HomeDirectoryUnavailable {
                path: raw.to_owned(),
            }
        });
    }

    if let Some(rest) = trimmed.strip_prefix("~/") {
        let Some(home_dir) = home_dir else {
            return Err(ConfigError::HomeDirectoryUnavailable {
                path: raw.to_owned(),
            });
        };
        return Ok(home_dir.join(rest));
    }

    Ok(PathBuf::from(trimmed))
}

/// First-run chooser: writes a starter config file with the selected keymap
/// layout when the resolved config path does not exist yet. Runs only on an
/// interactive terminal; non-tty launches silently keep the built-in default
/// (Dvorak preset). An explicit --config target is honoured too (that is where
/// users expect "save alongside config path").
pub fn maybe_first_run_layout(cli_override: Option<&str>) {
    use std::io::{IsTerminal, Write as _};

    let Some(path) = config_path(cli_override) else {
        return;
    };
    if path.exists() || !std::io::stdin().is_terminal() {
        return; // configured before or non-interactive: keep built-in defaults
    }

    eprintln!(
        "No shmooz config found at {}. Choose default keymap layout:",
        path.display()
    );
    eprintln!("  [1] qwerty  classic v0.3 defaults (Esc, +/-, arrows, d/w/s/...)");
    eprintln!("  [2] dvorak  left-half Dvorak preset (quote, j/k/u/x pan, p plus...)");
    eprint!("Enter choice [1/2/qwerty/dvorak], empty = 2: ");
    let _ = std::io::stderr().flush();

    let mut choice = String::new();
    if std::io::stdin().read_line(&mut choice).is_err() {
        eprintln!(" (no input, keeping dvorak preset)");
        return;
    }

    let layout = match choice.trim().to_ascii_lowercase().as_str() {
        "" | "2" | "d" | "dv" | "dvorak" => keymap::Layout::Dvorak,
        "1" | "q" | "qw" | "qwerty" | "classic" => keymap::Layout::Qwerty,
        other => {
            eprintln!(" unrecognized input \"{other}\", using dvorak preset");
            keymap::Layout::Dvorak
        }
    };

    let template = std::vec![
        "# shmooz starter config (created on first run).",
        "# Remap actions under [keybindings]; see the shipped example",
        "# (config.example.toml) for every action name.",
        &format!("layout = \"{}\"", layout.as_str()),
        "",
        "[keybindings]",
        "# close = \"q\"   # try a line like this to change the exit key",
    ]
    .join("\n")
        + "\n";

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, &template) {
        Ok(()) => eprintln!("wrote {}", path.display()),
        Err(err) => eprintln!("warning: could not write config {}: {err}", path.display()),
    }
}

#[cfg(test)]
pub fn test_config() -> Config {
    Config {
        keybindings: keymap::default_bindings(),
        initial_zoom: 0.0,
        output_filter: None,
        scroll_direction: ScrollDirection::default(),
        spotlight: false,
        screenshot_dir: "shots".into(),
        show_indicator: true,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use super::ScrollDirection;
    use super::{merge_keybindings_file, parse_zoom, resolve_screenshot_dir, test_config};
    use crate::error::ConfigError;
    use crate::keymap::{self, KeyAction};

    fn bindings_with(contents: &str) -> BTreeMap<KeyAction, Vec<u32>> {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.flush().unwrap();
        let mut keybindings = keymap::default_bindings();
        merge_keybindings_file(file.path(), &mut keybindings).unwrap();
        keybindings
    }

    #[test]
    fn parse_zoom_percent() {
        assert_eq!(parse_zoom("10%").unwrap(), 0.1);
    }

    #[test]
    fn parse_zoom_fraction() {
        assert_eq!(parse_zoom("0.5").unwrap(), 0.5);
    }

    #[test]
    fn parse_zoom_rejects_out_of_range_values() {
        assert!(parse_zoom("1.0").is_err());
        assert!(parse_zoom("100%").is_err());
    }

    #[test]
    fn resolve_screenshot_dir_expands_home() {
        let resolved =
            resolve_screenshot_dir("~/Desktop/Screenshots", Some(Path::new("/home/test"))).unwrap();

        assert_eq!(resolved, PathBuf::from("/home/test/Desktop/Screenshots"));
    }

    #[test]
    fn resolve_screenshot_dir_rejects_missing_home_for_tilde_paths() {
        assert!(resolve_screenshot_dir("~/shots", None).is_err());
    }

    #[test]
    fn resolve_screenshot_dir_keeps_non_tilde_paths() {
        let resolved = resolve_screenshot_dir("shots", Some(Path::new("/home/test"))).unwrap();

        assert_eq!(resolved, PathBuf::from("shots"));
    }

    #[test]
    fn default_keybindings_carry_upstream_defaults() {
        let config = test_config();
        assert_eq!(config.key_action(1), Some(KeyAction::Close));
        assert_eq!(config.key_action(13), Some(KeyAction::ZoomIn));
        assert_eq!(config.key_action(78), Some(KeyAction::ZoomIn));
        assert_eq!(config.key_action(12), Some(KeyAction::ZoomOut));
        assert_eq!(config.key_action(105), Some(KeyAction::PanLeft));
        assert_eq!(config.key_action(103), Some(KeyAction::PanUp));
        assert_eq!(config.key_action(11), Some(KeyAction::ResetView));
        assert_eq!(config.key_action(82), Some(KeyAction::ResetView));
        assert_eq!(config.key_action(23), Some(KeyAction::ToggleColorPicker));
        assert_eq!(config.key_action(32), Some(KeyAction::ToggleAnnotateZoomed));
        assert_eq!(
            config.key_action(17),
            Some(KeyAction::ToggleAnnotateUnzoomed)
        );
        assert_eq!(config.key_action(25), Some(KeyAction::ToolPen));
        assert_eq!(config.key_action(35), Some(KeyAction::ToolHighlighter));
        assert_eq!(config.key_action(38), Some(KeyAction::ToolLine));
        assert_eq!(config.key_action(19), Some(KeyAction::ToolRectangle));
        assert_eq!(config.key_action(18), Some(KeyAction::ToolEllipse));
        assert_eq!(config.key_action(50), Some(KeyAction::ToolMove));
        assert_eq!(config.key_action(20), Some(KeyAction::ToolText));
        assert_eq!(config.key_action(22), Some(KeyAction::UndoAnnotation));
        assert_eq!(config.key_action(46), Some(KeyAction::ClearAnnotations));
        assert_eq!(config.key_action(31), Some(KeyAction::SaveScreenshot));
        assert_eq!(config.key_action(33), Some(KeyAction::ToggleSpotlight));
        assert_eq!(config.key_action(26), Some(KeyAction::DecreaseSize));
        assert_eq!(config.key_action(27), Some(KeyAction::IncreaseSize));
        assert_eq!(config.key_action(48), None);
    }

    #[test]
    fn close_label_defaults_to_escape() {
        assert_eq!(test_config().close_label(), "Esc");
    }

    #[test]
    fn merge_keybindings_overrides_string_and_array_values() {
        let keybindings = bindings_with(
            r#"[keybindings]
close = "q"
zoom_in = ["KP_Add"]
"#,
        );
        assert_eq!(keybindings[&KeyAction::Close], vec![16]);
        assert_eq!(keybindings[&KeyAction::ZoomIn], vec![78]);
        assert_eq!(keybindings[&KeyAction::ZoomOut], vec![12, 74]);
    }

    #[test]
    fn merge_keybindings_allows_disabling_actions() {
        let keybindings = bindings_with("[keybindings]\nclose = []");
        assert!(keybindings[&KeyAction::Close].is_empty());
    }

    #[test]
    fn merge_keybindings_accepts_hyphenated_action_names() {
        let keybindings = bindings_with("[keybindings]\nclose = \"x\"\nzoom-in = \"minus\"");
        assert_eq!(keybindings[&KeyAction::Close], vec![45]);
        assert_eq!(keybindings[&KeyAction::ZoomIn], vec![12]);
    }

    #[test]
    fn merge_keybindings_rejects_unknown_action() {
        let err = bindings_err(
            r#"[keybindings]
foo = "q"
"#,
        );
        assert!(matches!(err, ConfigError::KeyBindingAction { .. }));
    }

    #[test]
    fn merge_keybindings_rejects_unknown_key() {
        let err = bindings_err(
            r#"[keybindings]
close = "nope"
"#,
        );
        assert!(matches!(err, ConfigError::KeyBindingKey { .. }));
    }

    #[test]
    fn merge_keybindings_rejects_non_string_values() {
        let err = bindings_err("[keybindings]\nclose = 3");
        assert!(matches!(err, ConfigError::KeyBindingValue { .. }));
    }

    #[test]
    fn example_config_shipped_with_repo_is_valid_and_binds_close() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.toml");
        let mut keybindings = keymap::default_bindings();
        merge_keybindings_file(&path, &mut keybindings).expect("config.example.toml must parse");

        // The reference file lists the defaults verbatim.
        assert_eq!(keybindings[&KeyAction::Close], vec![1]);
        assert_eq!(
            keybindings[&KeyAction::ZoomIn],
            vec![
                keymap::key_code("=").unwrap(),
                keymap::key_code("kpadd").unwrap()
            ]
        );
        assert_eq!(
            keybindings[&KeyAction::PanLeft],
            vec![keymap::key_code("left").unwrap()]
        );
    }

    #[test]
    fn merge_keybindings_rejects_unknown_option() {
        let err = bindings_err("not_a_real_option = \"up\"");
        assert!(matches!(err, ConfigError::UnknownConfigKey { .. }));
    }

    #[test]
    fn merge_scroll_direction_rejects_invalid_value() {
        let err = bindings_err("scroll_direction = \"sideways\"");
        assert!(matches!(err, ConfigError::InvalidScrollDirection { .. }));

        // sanity: valid values are accepted through the file merge.
        assert_eq!(
            scroll_from_file("scroll_direction = \"down\""),
            Some(ScrollDirection::Down)
        );
        assert_eq!(
            scroll_from_file("scroll_direction = \"up\""),
            Some(ScrollDirection::Up)
        );
    }
    #[track_caller]
    fn scroll_from_file(contents: &str) -> Option<ScrollDirection> {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.flush().unwrap();
        let mut target = keymap::default_bindings();
        merge_keybindings_file(file.path(), &mut target)
            .unwrap()
            .scroll
    }

    #[track_caller]
    fn layout_from_file(contents: &str) -> Option<keymap::Layout> {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.flush().unwrap();
        let mut target = keymap::default_bindings();
        merge_keybindings_file(file.path(), &mut target)
            .unwrap()
            .layout
    }

    #[test]
    fn layout_parses_from_known_names_only() {
        use keymap::Layout;
        assert_eq!(Layout::parse("qwerty"), Some(Layout::Qwerty));
        assert_eq!(Layout::parse(" QWERTy "), Some(Layout::Qwerty));
        assert_eq!(Layout::parse("dvorak"), Some(Layout::Dvorak));
        assert_eq!(Layout::parse("azerty"), None);
    }

    #[test]
    fn layout_from_file_accepts_qwerty_and_dvorak() {
        use keymap::Layout;
        assert_eq!(
            layout_from_file("layout = \"qwerty\""),
            Some(Layout::Qwerty)
        );
        assert_eq!(
            layout_from_file("layout = \"dvorak\"\nscroll_direction = \"up\""),
            Some(Layout::Dvorak)
        );
        let err = bindings_err("layout = 42");
        assert!(matches!(err, ConfigError::InvalidLayout { .. }));
    }

    #[test]
    fn builtin_dvorak_pis_default_when_choice_omitted() {
        // The built-in fallback layout must be Dvorak (left half preset), not QWERTY.
        let dv = keymap::default_bindings_layout(keymap::Layout::Dvorak);
        assert_eq!(
            dv[&KeyAction::Close],
            vec![keymap::key_code("'").unwrap(), 1]
        );
        assert_eq!(
            dv[&KeyAction::PanLeft],
            vec![keymap::key_code("j").unwrap()]
        );
        // ...and config default_bindings() alias still means classic for tests.
        assert_eq!(keymap::default_bindings()[&KeyAction::Close], vec![1]);
    }

    #[test]
    fn default_scroll_zoom_is_up() {
        assert_eq!(ScrollDirection::default(), ScrollDirection::Up);
    }

    fn bindings_err(contents: &str) -> ConfigError {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.flush().unwrap();
        let mut keybindings = keymap::default_bindings();
        merge_keybindings_file(file.path(), &mut keybindings)
            .err()
            .unwrap()
    }
}
