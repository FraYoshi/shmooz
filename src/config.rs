use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::cli::Cli;
use crate::error::ConfigError;
use crate::keymap::{self, KeyAction};

pub const DEFAULT_SCREENSHOT_DIR: &str = "~/Desktop/Screenshots";
pub const CONFIG_DIRECTORY_ENV: &str = "XDG_CONFIG_HOME";
pub const CONFIG_SUBPATH: &str = "shmooz/config.toml";
pub const KEYBINDINGS_SECTION: &str = "keybindings";

#[derive(Debug, Clone)]
pub struct Config {
    pub keybindings: BTreeMap<KeyAction, Vec<u32>>,
    pub initial_zoom: f64,
    pub output_filter: Option<String>,
    pub invert_scroll: bool,
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

        let mut keybindings = keymap::default_bindings();
        if let Some(path) = config_path(cli.config.as_deref()) {
            if path.is_file() {
                merge_keybindings_file(&path, &mut keybindings)?;
            } else if cli.config.is_some() {
                return Err(ConfigError::ConfigFileMissing {
                    path: path.display().to_string(),
                });
            }
        }

        if let Some(raw) = cli.map_close.as_deref() {
            let code = keymap::key_code(raw).ok_or_else(|| ConfigError::CloseKey {
                value: raw.to_owned(),
            })?;
            keybindings.insert(KeyAction::Close, vec![code]);
        }

        Ok(Self {
            keybindings,
            initial_zoom,
            output_filter: cli.output,
            invert_scroll: cli.invert_scroll,
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
fn merge_keybindings_file(
    path: &Path,
    keybindings: &mut BTreeMap<KeyAction, Vec<u32>>,
) -> Result<(), ConfigError> {
    let path_text = path.display().to_string();
    let raw = std::fs::read_to_string(path).map_err(|err| ConfigError::ConfigFileRead {
        path: path_text.clone(),
        message: err.to_string(),
    })?;
    let table: toml::Table = toml::from_str(&raw).map_err(|err| ConfigError::ConfigFileParse {
        path: path_text.clone(),
        message: err.to_string(),
    })?;

    for (option, _) in &table {
        if option != KEYBINDINGS_SECTION {
            return Err(ConfigError::UnknownConfigKey {
                path: path_text.clone(),
                value: option.clone(),
            });
        }
    }

    let Some(bindings) = table.get(KEYBINDINGS_SECTION) else {
        return Ok(());
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

    Ok(())
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

#[cfg(test)]
pub fn test_config() -> Config {
    Config {
        keybindings: keymap::default_bindings(),
        initial_zoom: 0.0,
        output_filter: None,
        invert_scroll: false,
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
    fn merge_keybindings_rejects_unknown_option() {
        let err = bindings_err("scroll_direction = \"up\"");
        assert!(matches!(err, ConfigError::UnknownConfigKey { .. }));
    }

    #[test]
    fn merge_keybindings_rejects_non_string_values() {
        let err = bindings_err("[keybindings]\nclose = 3");
        assert!(matches!(err, ConfigError::KeyBindingValue { .. }));
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
