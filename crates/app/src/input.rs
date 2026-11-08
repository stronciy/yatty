use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

pub fn key_bytes(event: &KeyEvent, modifiers: ModifiersState) -> Option<Vec<u8>> {
    let named = match &event.logical_key {
        Key::Named(named) => match named {
            NamedKey::Enter => Some(b"\r".as_slice()),
            NamedKey::Backspace => Some(b"\x7f".as_slice()),
            NamedKey::Tab => Some(b"\t".as_slice()),
            NamedKey::Escape => Some(b"\x1b".as_slice()),
            NamedKey::ArrowUp => Some(b"\x1b[A".as_slice()),
            NamedKey::ArrowDown => Some(b"\x1b[B".as_slice()),
            NamedKey::ArrowRight => Some(b"\x1b[C".as_slice()),
            NamedKey::ArrowLeft => Some(b"\x1b[D".as_slice()),
            NamedKey::Home => Some(b"\x1b[H".as_slice()),
            NamedKey::End => Some(b"\x1b[F".as_slice()),
            NamedKey::PageUp => Some(b"\x1b[5~".as_slice()),
            NamedKey::PageDown => Some(b"\x1b[6~".as_slice()),
            NamedKey::Delete => Some(b"\x1b[3~".as_slice()),
            NamedKey::Space => Some(b" ".as_slice()),
            _ => None,
        },
        _ => None,
    };
    if let Some(bytes) = named {
        return Some(bytes.to_vec());
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
