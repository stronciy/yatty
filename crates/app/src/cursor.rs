use std::time::Duration;

use config::{CursorSettings, CursorShape};
use render::CursorShape as RenderShape;

const PERIOD_SECS: f32 = 1.0;
const LEVELS: f32 = 32.0;

/// Brightness of the cursor at `elapsed` time since the last key press.
///
/// Without `blink` the cursor is always fully visible. With `blink` it switches on and off.
/// With `fade` it behaves like a phosphor screen: it glows at full brightness and decays
/// exponentially, then flares up again at the start of the next cycle.
pub fn alpha(elapsed: Duration, settings: &CursorSettings) -> f32 {
    if !settings.blink {
        return 1.0;
    }
    let t = (elapsed.as_secs_f32() % PERIOD_SECS) / PERIOD_SECS;
    if settings.fade {
        (-4.0 * t).exp()
    } else if t < 0.5 {
        1.0
    } else {
        0.0
    }
}

/// Alpha quantized to a few dozen levels, so the window redraws only when the cursor visibly changes.
pub fn level(elapsed: Duration, settings: &CursorSettings) -> u32 {
    (alpha(elapsed, settings) * LEVELS).round() as u32
}

pub fn render_shape(shape: CursorShape) -> RenderShape {
    match shape {
        CursorShape::Block => RenderShape::Block,
        CursorShape::Bar => RenderShape::Bar,
        CursorShape::Underline => RenderShape::Underline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(blink: bool, fade: bool) -> CursorSettings {
        CursorSettings {
            shape: CursorShape::Bar,
            blink,
            fade,
        }
    }

    #[test]
    fn steady_cursor_is_always_visible() {
        assert_eq!(
            alpha(Duration::from_millis(730), &settings(false, true)),
            1.0
        );
    }

    #[test]
    fn blinking_cursor_switches_each_half_period() {
        let s = settings(true, false);
        assert_eq!(alpha(Duration::from_millis(100), &s), 1.0);
        assert_eq!(alpha(Duration::from_millis(700), &s), 0.0);
        assert_eq!(alpha(Duration::from_millis(1100), &s), 1.0);
    }

    #[test]
    fn fading_cursor_decays_and_restarts_each_period() {
        let s = settings(true, true);
        let start = alpha(Duration::from_millis(0), &s);
        let later = alpha(Duration::from_millis(500), &s);
        assert_eq!(start, 1.0);
        assert!(later < 0.2, "decayed to {later}");
        assert_eq!(alpha(Duration::from_millis(1000), &s), 1.0);
    }
}
