use eframe::egui::{self, Event, InputState, Key};

struct OverlayHotkey {
    ctrl: bool,
    shift: bool,
    alt: bool,
    command: bool,
    key: Key,
}

pub(crate) fn hotkey_pressed(input: &InputState, accelerator: &str) -> bool {
    let Some(hotkey) = parse_overlay_hotkey(accelerator) else {
        return false;
    };

    input.events.iter().any(|event| match event {
        Event::Key {
            key,
            physical_key,
            pressed: true,
            repeat: false,
            modifiers,
        } => {
            (*key == hotkey.key || *physical_key == Some(hotkey.key))
                && hotkey_modifiers_match(&hotkey, modifiers)
        }
        Event::Cut => hotkey.key == Key::X && hotkey_modifiers_match(&hotkey, &input.modifiers),
        _ => false,
    }) || (input.key_pressed(hotkey.key) && hotkey_modifiers_match(&hotkey, &input.modifiers))
}

pub(crate) fn copy_shortcut_pressed(input: &InputState) -> bool {
    input
        .events
        .iter()
        .any(|event| matches!(event, Event::Copy))
        || command_shortcut_pressed(input, Key::C)
}

pub(crate) fn command_shortcut_pressed(input: &InputState, key: Key) -> bool {
    input.events.iter().any(|event| match event {
        Event::Key {
            key: event_key,
            physical_key,
            pressed: true,
            repeat: false,
            modifiers,
        } => {
            (*event_key == key || *physical_key == Some(key))
                && (modifiers.ctrl || modifiers.command)
                && !modifiers.alt
        }
        _ => false,
    }) || (input.key_pressed(key)
        && (input.modifiers.ctrl || input.modifiers.command)
        && !input.modifiers.alt)
}

pub(crate) fn command_shift_shortcut_pressed(input: &InputState, key: Key) -> bool {
    input.events.iter().any(|event| match event {
        Event::Key {
            key: event_key,
            physical_key,
            pressed: true,
            repeat: false,
            modifiers,
        } => {
            (*event_key == key || *physical_key == Some(key))
                && (modifiers.ctrl || modifiers.command)
                && modifiers.shift
                && !modifiers.alt
        }
        _ => false,
    }) || (input.key_pressed(key)
        && (input.modifiers.ctrl || input.modifiers.command)
        && input.modifiers.shift
        && !input.modifiers.alt)
}

fn hotkey_modifiers_match(hotkey: &OverlayHotkey, modifiers: &egui::Modifiers) -> bool {
    let ctrl_down = modifiers.ctrl || modifiers.command;
    ctrl_down == hotkey.ctrl
        && modifiers.shift == hotkey.shift
        && modifiers.alt == hotkey.alt
        && (!hotkey.command || modifiers.command)
}

// Accept Electron/Tauri-style accelerators such as Ctrl+Shift+X and normalize them to egui keys.
fn parse_overlay_hotkey(accelerator: &str) -> Option<OverlayHotkey> {
    let mut hotkey = OverlayHotkey {
        ctrl: false,
        shift: false,
        alt: false,
        command: false,
        key: Key::X,
    };
    let mut has_key = false;

    for raw_part in accelerator
        .replace('-', "+")
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match raw_part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => hotkey.ctrl = true,
            "shift" => hotkey.shift = true,
            "alt" | "option" => hotkey.alt = true,
            "win" | "cmd" | "meta" | "super" => hotkey.command = true,
            _ => {
                hotkey.key = parse_overlay_key(raw_part)?;
                has_key = true;
            }
        }
    }

    has_key.then_some(hotkey)
}

fn parse_overlay_key(key: &str) -> Option<Key> {
    match key.to_ascii_uppercase().as_str() {
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "X" => Some(Key::X),
        "Y" => Some(Key::Y),
        "Z" => Some(Key::Z),
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        "ESC" | "ESCAPE" => Some(Key::Escape),
        "ENTER" | "RETURN" => Some(Key::Enter),
        "SPACE" => Some(Key::Space),
        "TAB" => Some(Key::Tab),
        value => value
            .strip_prefix('F')
            .and_then(|number| number.parse::<usize>().ok())
            .and_then(function_key),
    }
}

fn function_key(number: usize) -> Option<Key> {
    match number {
        1 => Some(Key::F1),
        2 => Some(Key::F2),
        3 => Some(Key::F3),
        4 => Some(Key::F4),
        5 => Some(Key::F5),
        6 => Some(Key::F6),
        7 => Some(Key::F7),
        8 => Some(Key::F8),
        9 => Some(Key::F9),
        10 => Some(Key::F10),
        11 => Some(Key::F11),
        12 => Some(Key::F12),
        13 => Some(Key::F13),
        14 => Some(Key::F14),
        15 => Some(Key::F15),
        16 => Some(Key::F16),
        17 => Some(Key::F17),
        18 => Some(Key::F18),
        19 => Some(Key::F19),
        20 => Some(Key::F20),
        21 => Some(Key::F21),
        22 => Some(Key::F22),
        23 => Some(Key::F23),
        24 => Some(Key::F24),
        _ => None,
    }
}
