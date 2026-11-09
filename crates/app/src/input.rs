use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Encodes a key press as the byte sequence a terminal application expects.
///
/// `application_cursor` is the DECCKM mode that programs such as `mc` or `vim` turn on;
/// in that mode the cursor keys use `ESC O x` instead of `ESC [ x`.
pub fn key_bytes(
    event: &KeyEvent,
    modifiers: ModifiersState,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    if let Key::Named(named) = &event.logical_key {
        return named_key_bytes(*named, modifiers, application_cursor);
    }

    let text = event.text.as_ref()?;
    if modifiers.control_key()
        && let Some(ch) = text.chars().next()
        && ch.is_ascii_alphabetic()
    {
        return Some(vec![ch.to_ascii_lowercase() as u8 & 0x1f]);
    }
    Some(text.as_bytes().to_vec())
}

fn named_key_bytes(
    key: NamedKey,
    modifiers: ModifiersState,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    let param = modifier_param(modifiers);
    let bytes = match key {
        NamedKey::Enter => "\r".to_string(),
        NamedKey::Backspace => "\x7f".to_string(),
        NamedKey::Tab if modifiers.shift_key() => "\x1b[Z".to_string(),
        NamedKey::Tab => "\t".to_string(),
        NamedKey::Escape => "\x1b".to_string(),
        NamedKey::Space => " ".to_string(),
        NamedKey::ArrowUp => letter_key('A', param, application_cursor),
        NamedKey::ArrowDown => letter_key('B', param, application_cursor),
        NamedKey::ArrowRight => letter_key('C', param, application_cursor),
        NamedKey::ArrowLeft => letter_key('D', param, application_cursor),
        NamedKey::Home => letter_key('H', param, application_cursor),
        NamedKey::End => letter_key('F', param, application_cursor),
        NamedKey::F1 => ss3_key('P', param),
        NamedKey::F2 => ss3_key('Q', param),
        NamedKey::F3 => ss3_key('R', param),
        NamedKey::F4 => ss3_key('S', param),
        NamedKey::Insert => tilde_key(2, param),
        NamedKey::Delete => tilde_key(3, param),
        NamedKey::PageUp => tilde_key(5, param),
        NamedKey::PageDown => tilde_key(6, param),
        NamedKey::F5 => tilde_key(15, param),
        NamedKey::F6 => tilde_key(17, param),
        NamedKey::F7 => tilde_key(18, param),
        NamedKey::F8 => tilde_key(19, param),
        NamedKey::F9 => tilde_key(20, param),
        NamedKey::F10 => tilde_key(21, param),
        NamedKey::F11 => tilde_key(23, param),
        NamedKey::F12 => tilde_key(24, param),
        _ => return None,
    };
    Some(bytes.into_bytes())
}

/// xterm modifier parameter: 1 + shift(1) + alt(2) + ctrl(4). `None` when no modifier is held.
fn modifier_param(modifiers: ModifiersState) -> Option<u8> {
    let mut value = 1;
    if modifiers.shift_key() {
        value += 1;
    }
    if modifiers.alt_key() {
        value += 2;
    }
    if modifiers.control_key() {
        value += 4;
    }
    (value > 1).then_some(value)
}

fn letter_key(letter: char, param: Option<u8>, application_cursor: bool) -> String {
    match param {
        Some(param) => format!("\x1b[1;{param}{letter}"),
        None if application_cursor => format!("\x1bO{letter}"),
        None => format!("\x1b[{letter}"),
    }
}

fn ss3_key(letter: char, param: Option<u8>) -> String {
    match param {
        Some(param) => format!("\x1b[1;{param}{letter}"),
        None => format!("\x1bO{letter}"),
    }
}

fn tilde_key(code: u8, param: Option<u8>) -> String {
    match param {
        Some(param) => format!("\x1b[{code};{param}~"),
        None => format!("\x1b[{code}~"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(key: NamedKey, modifiers: ModifiersState, app_cursor: bool) -> String {
        String::from_utf8(named_key_bytes(key, modifiers, app_cursor).unwrap()).unwrap()
    }

    #[test]
    fn arrows_follow_application_cursor_mode() {
        let none = ModifiersState::empty();
        assert_eq!(named(NamedKey::ArrowUp, none, false), "\x1b[A");
        assert_eq!(named(NamedKey::ArrowUp, none, true), "\x1bOA");
        assert_eq!(named(NamedKey::ArrowLeft, none, true), "\x1bOD");
    }

    #[test]
    fn modified_arrows_use_csi_with_parameter() {
        assert_eq!(
            named(NamedKey::ArrowRight, ModifiersState::CONTROL, false),
            "\x1b[1;5C"
        );
        assert_eq!(
            named(
                NamedKey::ArrowRight,
                ModifiersState::SHIFT | ModifiersState::ALT,
                true
            ),
            "\x1b[1;4C"
        );
    }

    #[test]
    fn function_keys_match_terminfo() {
        let none = ModifiersState::empty();
        assert_eq!(named(NamedKey::F1, none, false), "\x1bOP");
        assert_eq!(named(NamedKey::F4, none, true), "\x1bOS");
        assert_eq!(named(NamedKey::F5, none, false), "\x1b[15~");
        assert_eq!(named(NamedKey::F10, none, false), "\x1b[21~");
        assert_eq!(named(NamedKey::F12, none, false), "\x1b[24~");
        assert_eq!(
            named(NamedKey::F5, ModifiersState::SHIFT, false),
            "\x1b[15;2~"
        );
        assert_eq!(
            named(NamedKey::F1, ModifiersState::CONTROL, false),
            "\x1b[1;5P"
        );
    }

    #[test]
    fn navigation_keys() {
        let none = ModifiersState::empty();
        assert_eq!(named(NamedKey::Home, none, false), "\x1b[H");
        assert_eq!(named(NamedKey::Home, none, true), "\x1bOH");
        assert_eq!(named(NamedKey::End, none, false), "\x1b[F");
        assert_eq!(named(NamedKey::Delete, none, false), "\x1b[3~");
        assert_eq!(named(NamedKey::PageUp, none, false), "\x1b[5~");
        assert_eq!(named(NamedKey::PageDown, none, false), "\x1b[6~");
        assert_eq!(named(NamedKey::Insert, none, false), "\x1b[2~");
        assert_eq!(named(NamedKey::Tab, ModifiersState::SHIFT, false), "\x1b[Z");
    }
}
