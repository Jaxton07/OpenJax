use std::sync::LazyLock;
use std::time::Instant;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaletteHint {
    TrueColor,
    Fallback,
}

impl PaletteHint {
    pub(crate) fn detect() -> Self {
        static DETECTED: LazyLock<PaletteHint> = LazyLock::new(|| {
            let hint = std::env::var("COLORTERM")
                .unwrap_or_default()
                .to_ascii_lowercase();
            if hint.contains("truecolor") || hint.contains("24bit") {
                PaletteHint::TrueColor
            } else {
                PaletteHint::Fallback
            }
        });
        *DETECTED
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShimmerProfile {
    pub(crate) tick_ms: u64,
    pub(crate) sweep_ms: u64,
    pub(crate) band_half_width: f32,
    pub(crate) padding: usize,
}

impl Default for ShimmerProfile {
    fn default() -> Self {
        Self {
            tick_ms: 80,
            sweep_ms: 2_000,
            band_half_width: 5.0,
            padding: 10,
        }
    }
}

pub(crate) fn shimmer_spans(
    text: &str,
    now: Instant,
    started_at: Instant,
    profile: &ShimmerProfile,
    palette: PaletteHint,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let elapsed_ms = now.saturating_duration_since(started_at).as_millis() as u64;
    let tick_ms = profile.tick_ms.max(1);
    let sweep_ms = profile.sweep_ms.max(1);
    let snapped_ms = (elapsed_ms / tick_ms) * tick_ms;

    let period = chars.len() + profile.padding * 2;
    let phase = (snapped_ms % sweep_ms) as f32 / sweep_ms as f32;
    let pos = phase * period as f32;

    let mut spans = Vec::with_capacity(chars.len());
    for (i, ch) in chars.iter().enumerate() {
        let i_pos = i as isize + profile.padding as isize;
        let dist = (i_pos as f32 - pos).abs();

        let intensity = if dist <= profile.band_half_width {
            let x = std::f32::consts::PI * (dist / profile.band_half_width.max(0.1));
            0.5 * (1.0 + x.cos())
        } else {
            0.0
        };

        let style = match palette {
            PaletteHint::TrueColor => {
                let (r, g, b) = blend_rgb((225, 225, 225), (90, 90, 90), intensity * 0.9);
                #[allow(clippy::disallowed_methods)]
                {
                    Style::default()
                        .fg(Color::Rgb(r, g, b))
                        .add_modifier(Modifier::BOLD)
                }
            }
            PaletteHint::Fallback => fallback_style(intensity),
        };
        spans.push(Span::styled(ch.to_string(), style));
    }

    spans
}

fn blend_rgb(base: (u8, u8, u8), shadow: (u8, u8, u8), alpha: f32) -> (u8, u8, u8) {
    let a = alpha.clamp(0.0, 1.0);
    let r = (base.0 as f32 * (1.0 - a) + shadow.0 as f32 * a) as u8;
    let g = (base.1 as f32 * (1.0 - a) + shadow.1 as f32 * a) as u8;
    let b = (base.2 as f32 * (1.0 - a) + shadow.2 as f32 * a) as u8;
    (r, g, b)
}

fn fallback_style(intensity: f32) -> Style {
    if intensity < 0.2 {
        Style::default().add_modifier(Modifier::DIM)
    } else if intensity < 0.6 {
        Style::default()
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::style::Modifier;

    use super::*;

    #[test]
    fn empty_text_has_no_spans() {
        let now = Instant::now();
        let spans = shimmer_spans(
            "",
            now,
            now,
            &ShimmerProfile::default(),
            PaletteHint::Fallback,
        );
        assert!(spans.is_empty());
    }

    #[test]
    fn same_tick_is_deterministic() {
        let started = Instant::now();
        let now = started + Duration::from_millis(240);
        let profile = ShimmerProfile::default();
        let a = shimmer_spans("Working", now, started, &profile, PaletteHint::Fallback);
        let b = shimmer_spans("Working", now, started, &profile, PaletteHint::Fallback);
        assert_eq!(a, b);
    }

    #[test]
    fn different_time_changes_output() {
        let started = Instant::now();
        let profile = ShimmerProfile::default();
        let a = shimmer_spans(
            "Working",
            started + Duration::from_millis(760),
            started,
            &profile,
            PaletteHint::Fallback,
        );
        let b = shimmer_spans(
            "Working",
            started + Duration::from_millis(1_240),
            started,
            &profile,
            PaletteHint::Fallback,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn fallback_uses_multiple_levels() {
        let started = Instant::now();
        let profile = ShimmerProfile::default();
        let spans = shimmer_spans(
            "Working",
            started + Duration::from_millis(760),
            started,
            &profile,
            PaletteHint::Fallback,
        );

        let has_dim = spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::DIM));
        let has_bold = spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::BOLD));
        assert!(has_dim);
        assert!(has_bold);
    }
}
