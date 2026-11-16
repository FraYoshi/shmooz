use clap::Parser;

const HELP_EPILOG: &str = "\
Controls:
  Navigation:
    Mouse scroll          Zoom in/out at mouse position
    Left click + drag     Pan the view
    Double click          Restore/unzoom
    +/-                   Zoom in/out at center
    Arrow keys            Pan the view
    0                     Restore/unzoom
    d                     Toggle draw mode on current zoom
    w                     Toggle draw-without-zoom mode
    s                     Save screenshot
    Ctrl+C                Copy screenshot to clipboard
    f                     Toggle spotlight dim overlay
    [ / ]                 Decrease/increase spotlight radius

  Color picker:
    i                     Toggle color picker
    Mouse move            Update the swatch
    Left click            Copy picked hex color
    Esc                   Leave color picker

  Annotation:
    Left click + drag     Draw, or move annotations while move mode is active
    Left click            Place text when text mode is active
    1-0 / - / =           Select one of 12 preset annotation colors
    [ / ]                 Decrease/increase text height
    p                     Pen
    h                     Highlighter
    m                     Toggle move mode for existing annotations
    t                     Toggle text mode and place text with click
    l                     Line
    r                     Rectangle
    e                     Ellipse
    Enter                 Commit active text and stay in text mode
    Backspace             Delete last text character
    u                     Undo last annotation
    c                     Clear annotations
    Esc                   Leave active modifier first, then leave annotation mode

  Global:
    Right click           Exit
    Esc                   Exit from navigation by default

Config File:
  Path: $XDG_CONFIG_HOME/shmooz/config.toml, or --config PATH; strict TOML.
  Top-level global options come before any table (e.g. scroll_direction = \"up\"
  reverses the wheel so it zooms on the up rotation). Under [keybindings], each
  action may be one key name or an array of names; [] disables an action.
  See config.example.toml for the annotated complete reference.

Accepted actions in [keybindings] (all built-in defaults shown there):
    close, zoom_in, zoom_out, pan_left, pan_right, pan_up, pan_down,
    reset_view, save_screenshot, toggle_spotlight, toggle_color_picker,
    toggle_annotate_zoomed, toggle_annotate_unzoomed, tool_pen,
    tool_highlighter, tool_line, tool_rectangle, tool_ellipse, tool_move,
    tool_text, undo_annotation, clear_annotations, decrease_size, increase_size";

#[derive(Debug, Clone, Parser)]
#[command(
    name = "shmooz",
    disable_help_subcommand = true,
    about = "A zoom / magnifier utility for Wayland compositors",
    override_usage = "shmooz [options...]",
    after_help = HELP_EPILOG
)]
pub struct Cli {
    #[arg(
        long = "map-close",
        value_name = "KEY",
        help = "Set key to close (XKB-style key name, e.g., 'Escape', 'q', 'x')"
    )]
    pub map_close: Option<String>,

    #[arg(
        long = "config",
        value_name = "PATH",
        help = "Config file path (default: $XDG_CONFIG_HOME/shmooz/config.toml)"
    )]
    pub config: Option<String>,

    #[arg(
        long = "output",
        value_name = "NAME",
        help = "Run on a specific output (e.g., 'DP-1')"
    )]
    pub output: Option<String>,

    #[arg(
        long = "zoom-in",
        value_name = "PERCENT",
        help = "Set initial zoom percentage (e.g., '10%', '0.5')"
    )]
    pub zoom_in: Option<String>,

    #[arg(
        long = "scroll-direction",
        value_name = "DIRECTION",
        help = "Which way to push the wheel to zoom in (down or up)"
    )]
    pub scroll_direction: Option<String>,

    #[arg(
        long = "invert-scroll",
        conflicts_with = "scroll_direction",
        help = "Invert scroll direction (shorthand for --scroll-direction up)"
    )]
    pub invert_scroll: bool,

    #[arg(
        long = "spotlight",
        help = "Dim screen outside a spotlight circle around the pointer"
    )]
    pub spotlight: bool,

    #[arg(
        long = "screenshot-dir",
        value_name = "DIR",
        help = "Directory for saved screenshots (default: ~/Desktop/Screenshots)"
    )]
    pub screenshot_dir: Option<String>,

    #[arg(long = "no-indicator", help = "Hide the zoom mode indicator badge")]
    pub no_indicator: bool,
}

#[cfg(test)]
mod tests {
    use crate::keymap::KeyAction;

    #[test]
    fn help_epilog_lists_all_configurable_actions() {
        for name in KeyAction::known_actions() {
            assert!(
                super::HELP_EPILOG.contains(name),
                "help epilog missing action: {}",
                name
            );
        }
    }
}
