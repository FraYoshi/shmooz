//! Key/action tables for the customizable keymap.
//!
//! Key names are lowercase XKB-style names (esc, minus, KP_Add, F1, ...)
//! and resolve to Linux input event codes (evdev).

use std::collections::BTreeMap;

/// An action that can be assigned to keyboard keys via the config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KeyAction {
    Close,
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    ResetView,
    SaveScreenshot,
    ToggleSpotlight,
    ToggleColorPicker,
    ToggleAnnotateZoomed,
    ToggleAnnotateUnzoomed,
    ToolPen,
    ToolHighlighter,
    ToolLine,
    ToolRectangle,
    ToolEllipse,
    ToolMove,
    ToolText,
    UndoAnnotation,
    ClearAnnotations,
    DecreaseSize,
    IncreaseSize,
}

impl KeyAction {
    /// Whether holding the bound key auto-repeats (navigation controls only).
    pub fn is_repeatable(self) -> bool {
        matches!(
            self,
            Self::ZoomIn
                | Self::ZoomOut
                | Self::PanLeft
                | Self::PanRight
                | Self::PanUp
                | Self::PanDown
                | Self::DecreaseSize
                | Self::IncreaseSize
        )
    }

    /// Parses a config-file action name (hyphens/underscores/spaces, case
    /// ignored, e.g. "Zoom In" == "zoom_in").
    pub fn from_config_name(name: &str) -> Option<Self> {
        match normalize(name).as_str() {
            "close" => Some(Self::Close),
            "zoomin" => Some(Self::ZoomIn),
            "zoomout" => Some(Self::ZoomOut),
            "panleft" => Some(Self::PanLeft),
            "panright" => Some(Self::PanRight),
            "panup" => Some(Self::PanUp),
            "pandown" => Some(Self::PanDown),
            "resetview" => Some(Self::ResetView),
            "savescreenshot" => Some(Self::SaveScreenshot),
            "togglespotlight" => Some(Self::ToggleSpotlight),
            "togglecolorpicker" => Some(Self::ToggleColorPicker),
            "toggleannotatezoomed" => Some(Self::ToggleAnnotateZoomed),
            "toggleannotateunzoomed" => Some(Self::ToggleAnnotateUnzoomed),
            "toolpen" => Some(Self::ToolPen),
            "toolhighlighter" => Some(Self::ToolHighlighter),
            "toolline" => Some(Self::ToolLine),
            "toolrectangle" => Some(Self::ToolRectangle),
            "toolellipse" => Some(Self::ToolEllipse),
            "toolmove" => Some(Self::ToolMove),
            "tooltext" => Some(Self::ToolText),
            "undoannotation" => Some(Self::UndoAnnotation),
            "clearannotations" => Some(Self::ClearAnnotations),
            "decreasesize" => Some(Self::DecreaseSize),
            "increasesize" => Some(Self::IncreaseSize),
            _ => None,
        }
    }

    pub fn known_actions() -> Vec<&'static str> {
        vec![
            "close",
            "zoom_in",
            "zoom_out",
            "pan_left",
            "pan_right",
            "pan_up",
            "pan_down",
            "reset_view",
            "save_screenshot",
            "toggle_spotlight",
            "toggle_color_picker",
            "toggle_annotate_zoomed",
            "toggle_annotate_unzoomed",
            "tool_pen",
            "tool_highlighter",
            "tool_line",
            "tool_rectangle",
            "tool_ellipse",
            "tool_move",
            "tool_text",
            "undo_annotation",
            "clear_annotations",
            "decrease_size",
            "increase_size",
        ]
    }
}

/// Built-in keymap layout presets selectable at first run or in config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// Classic upstream defaults (leftover from v0.3).
    Qwerty,
    /// Left-half Dvorak preset — default when no choice was made.
    #[default]
    Dvorak,
}

impl Layout {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "qwerty" | "qw" | "classic" => Some(Self::Qwerty),
            "dvorak" | "dv" | "left" => Some(Self::Dvorak),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qwerty => "qwerty",
            Self::Dvorak => "dvorak",
        }
    }
}

/// Default key bindings for the classic (v0.3) layout, mirroring the upstream
/// built-in keymap.
pub fn default_bindings_qwerty() -> BTreeMap<KeyAction, Vec<u32>> {
    [
        (KeyAction::Close, vec![1u32]),
        (KeyAction::ZoomIn, vec![13, 78]),
        (KeyAction::ZoomOut, vec![12, 74]),
        (KeyAction::PanLeft, vec![105]),
        (KeyAction::PanRight, vec![106]),
        (KeyAction::PanUp, vec![103]),
        (KeyAction::PanDown, vec![108]),
        (KeyAction::ResetView, vec![11, 82]),
        (KeyAction::SaveScreenshot, vec![31]),
        (KeyAction::ToggleSpotlight, vec![33]),
        (KeyAction::ToggleColorPicker, vec![23]),
        (KeyAction::ToggleAnnotateZoomed, vec![32]),
        (KeyAction::ToggleAnnotateUnzoomed, vec![17]),
        (KeyAction::ToolPen, vec![25]),
        (KeyAction::ToolHighlighter, vec![35]),
        (KeyAction::ToolLine, vec![38]),
        (KeyAction::ToolRectangle, vec![19]),
        (KeyAction::ToolEllipse, vec![18]),
        (KeyAction::ToolMove, vec![50]),
        (KeyAction::ToolText, vec![20]),
        (KeyAction::UndoAnnotation, vec![22]),
        (KeyAction::ClearAnnotations, vec![46]),
        (KeyAction::DecreaseSize, vec![26]),
        (KeyAction::IncreaseSize, vec![27]),
    ]
    .into_iter()
    .collect()
}

/// Resolves a key name (e.g. "minus", "KP_Add", "F1", "a") to an evdev key
/// code.
pub fn key_code(name: &str) -> Option<u32> {
    // Literal single-character names (the printable ASCII forms of the keys)
    // must be checked first: normalize() drops '-' etc. as word separators and
    // would make the alias arms below dead code for these glyphs.
    if let Some(code) = match name.trim() {
        "-" => Some(12u32),
        "=" | "+" => Some(13),
        "[" | "{" => Some(26),
        "]" | "}" => Some(27),
        ";" => Some(39),
        "'" => Some(40),
        "`" => Some(41),
        "," | "<" => Some(51),
        "." | ">" => Some(52),
        "/" | "?" => Some(53),
        "\\" | "|" => Some(43),
        _ => None,
    } {
        return Some(code);
    }

    match normalize(name).as_str() {
        "esc" | "escape" => Some(1),
        "minus" | "-" => Some(12),
        "equal" | "=" => Some(13),
        "kpadd" | "kpplus" => Some(78),
        "kpsubtract" | "kpminus" => Some(74),
        "kpmultiply" => Some(55),
        "kpdivide" => Some(98),
        "kpenter" => Some(96),
        "kpdecimal" => Some(83),
        "kp0" => Some(82),
        "kp1" => Some(79),
        "kp2" => Some(80),
        "kp3" => Some(81),
        "kp4" => Some(75),
        "kp5" => Some(76),
        "kp6" => Some(77),
        "kp7" => Some(71),
        "kp8" => Some(72),
        "kp9" => Some(73),
        "up" => Some(103),
        "down" => Some(108),
        "left" => Some(105),
        "right" => Some(106),
        "home" => Some(102),
        "end" => Some(107),
        "insert" => Some(110),
        "delete" => Some(111),
        "pageup" | "pgup" => Some(104),
        "pagedown" | "pgdn" => Some(109),
        "enter" | "return" => Some(28),
        "backspace" => Some(14),
        "tab" => Some(15),
        "space" => Some(57),
        "bracketleft" | "leftbracket" | "[" => Some(26),
        "bracketright" | "rightbracket" | "]" => Some(27),
        "semicolon" | ";" => Some(39),
        "grave" => Some(41),
        "backslash" => Some(43),
        "comma" | "," => Some(51),
        "period" | "." => Some(52),
        "slash" | "/" => Some(53),
        "capslock" => Some(58),
        "f1" => Some(59),
        "f2" => Some(60),
        "f3" => Some(61),
        "f4" => Some(62),
        "f5" => Some(63),
        "f6" => Some(64),
        "f7" => Some(65),
        "f8" => Some(66),
        "f9" => Some(67),
        "f10" => Some(68),
        "f11" => Some(87),
        "f12" => Some(88),
        "0" => Some(11),
        "1" => Some(2),
        "2" => Some(3),
        "3" => Some(4),
        "4" => Some(5),
        "5" => Some(6),
        "6" => Some(7),
        "7" => Some(8),
        "8" => Some(9),
        "9" => Some(10),
        "q" => Some(16),
        "w" => Some(17),
        "e" => Some(18),
        "r" => Some(19),
        "t" => Some(20),
        "y" => Some(21),
        "u" => Some(22),
        "i" => Some(23),
        "o" => Some(24),
        "p" => Some(25),
        "a" => Some(30),
        "s" => Some(31),
        "d" => Some(32),
        "f" => Some(33),
        "g" => Some(34),
        "h" => Some(35),
        "j" => Some(36),
        "k" => Some(37),
        "l" => Some(38),
        "z" => Some(44),
        "x" => Some(45),
        "c" => Some(46),
        "v" => Some(47),
        "b" => Some(48),
        "n" => Some(49),
        "m" => Some(50),
        _ => None,
    }
}

/// Returns a human-readable key name for an evdev key code.
pub fn key_label(key_code: u32) -> &'static str {
    match key_code {
        1 => "Esc",
        2 => "1",
        3 => "2",
        4 => "3",
        5 => "4",
        6 => "5",
        7 => "6",
        8 => "7",
        9 => "8",
        10 => "9",
        11 => "0",
        12 => "-",
        13 => "=",
        14 => "Backspace",
        15 => "Tab",
        16 => "Q",
        17 => "W",
        18 => "E",
        19 => "R",
        20 => "T",
        21 => "Y",
        22 => "U",
        23 => "I",
        24 => "O",
        25 => "P",
        26 => "[",
        27 => "]",
        28 => "Enter",
        30 => "A",
        31 => "S",
        32 => "D",
        33 => "F",
        34 => "G",
        35 => "H",
        36 => "J",
        37 => "K",
        38 => "L",
        39 => ";",
        40 => "'",
        41 => "`",
        43 => "\\",
        44 => "Z",
        45 => "X",
        46 => "C",
        47 => "V",
        48 => "B",
        49 => "N",
        50 => "M",
        51 => ",",
        52 => ".",
        53 => "/",
        55 => "KP_Multiply",
        57 => "Space",
        58 => "Caps_Lock",
        59 => "F1",
        60 => "F2",
        61 => "F3",
        62 => "F4",
        63 => "F5",
        64 => "F6",
        65 => "F7",
        66 => "F8",
        67 => "F9",
        68 => "F10",
        71 => "KP_7",
        72 => "KP_8",
        73 => "KP_9",
        74 => "KP_Subtract",
        75 => "KP_4",
        76 => "KP_5",
        77 => "KP_6",
        78 => "KP_Add",
        79 => "KP_1",
        80 => "KP_2",
        81 => "KP_3",
        82 => "KP_0",
        83 => "KP_Decimal",
        87 => "F11",
        88 => "F12",
        96 => "KP_Enter",
        98 => "KP_Divide",
        102 => "Home",
        103 => "Up",
        104 => "Page_Up",
        105 => "Left",
        106 => "Right",
        107 => "End",
        108 => "Down",
        109 => "Page_Down",
        110 => "Insert",
        111 => "Delete",
        _ => "Unknown",
    }
}

fn normalize(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .replace([' ', '-', '_'], "")
}

/// Default key bindings for the classic v0.3 layout.
#[allow(dead_code)] // alias of qwerty preset kept for tests
pub fn default_bindings() -> BTreeMap<KeyAction, Vec<u32>> {
    default_bindings_qwerty()
}

/// Resolve the binding table for a preset layout (Dvorak is the built-in
/// default when the user never chose anything).
pub fn default_bindings_layout(layout: Layout) -> BTreeMap<KeyAction, Vec<u32>> {
    match layout {
        Layout::Qwerty => default_bindings_qwerty(),
        Layout::Dvorak => default_bindings_dvorak(),
    }
}

/// Left-half Dvorak preset: the printable glyphs of an ortho left half with
/// glove-sora firmware, bound to what that firmware sends per key.
pub fn default_bindings_dvorak() -> BTreeMap<KeyAction, Vec<u32>> {
    let named: &[(&str, &[&str])] = &[
        ("close", &["'", "esc"]),
        ("zoom_in", &["p"]),
        ("zoom_out", &[","]),
        ("pan_left", &["j"]),
        ("pan_right", &["k"]),
        ("pan_up", &["u"]),
        ("pan_down", &["x"]),
        ("reset_view", &["."]),
        ("save_screenshot", &["y"]),
        ("toggle_spotlight", &["space"]),
        ("toggle_color_picker", &[";"]),
        ("toggle_annotate_zoomed", &["e"]),
        ("toggle_annotate_unzoomed", &["a"]),
        ("tool_pen", &["1"]),
        ("tool_highlighter", &["2"]),
        ("tool_line", &["3"]),
        ("tool_rectangle", &["4"]),
        ("tool_ellipse", &["5"]),
        ("tool_move", &["m"]),
        ("tool_text", &["t"]),
        ("undo_annotation", &["o"]),
        ("clear_annotations", &["i"]),
        ("decrease_size", &["["]),
        ("increase_size", &["]"]),
    ];

    let mut map = BTreeMap::new();
    for (action_name, keys) in named {
        let action = KeyAction::from_config_name(action_name)
            .expect("builtin dvorak preset action must exist");
        let codes = keys
            .iter()
            .map(|name| key_code(name).expect("builtin dvorak preset key must exist"))
            .collect();
        map.insert(action, codes);
    }
    map
}
