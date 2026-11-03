// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! The app's colours (DESIGN.md, “Color”), for light and dark, and the check
//! that every pair the interface uses is readable.

/// An opaque colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn hex(value: u32) -> Self {
        Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
    }

    pub fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// This colour drawn at `alpha` over `base`: how a tint looks on a card.
    #[cfg(test)]
    pub fn over(self, base: Rgb, alpha: f64) -> Rgb {
        let mix = |top: u8, bottom: u8| {
            (f64::from(top) * alpha + f64::from(bottom) * (1.0 - alpha)).round() as u8
        };
        Rgb(
            mix(self.0, base.0),
            mix(self.1, base.1),
            mix(self.2, base.2),
        )
    }

    /// WCAG relative luminance.
    #[cfg(test)]
    fn luminance(self) -> f64 {
        let channel = |value: u8| {
            let c = f64::from(value) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.0) + 0.7152 * channel(self.1) + 0.0722 * channel(self.2)
    }
}

/// WCAG contrast ratio, 1 to 21.
#[cfg(test)]
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (light, dark) = if la > lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// Opacity of a state colour laid over a card to tint it. These three numbers
/// are the ones in `data/style.css`; a test keeps them in step.
#[cfg(test)]
pub const TINT: f64 = 0.12;
/// Opacity of the accent behind the next-run banner.
#[cfg(test)]
pub const BANNER_TINT: f64 = 0.16;
/// Opacity of the accent behind the “Yours” chip.
#[cfg(test)]
pub const CHIP_TINT: f64 = 0.14;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub accent: Rgb,
    pub on_accent: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub bad: Rgb,
    pub idle: Rgb,
    /// libadwaita's window, card and text colours, for the checks only: the
    /// interface itself uses `@window_bg_color` and friends.
    #[cfg_attr(not(test), allow(dead_code))]
    pub window_bg: Rgb,
    #[cfg_attr(not(test), allow(dead_code))]
    pub card_bg: Rgb,
    /// Behind the lines of a log.
    #[cfg_attr(not(test), allow(dead_code))]
    pub view_bg: Rgb,
    #[cfg_attr(not(test), allow(dead_code))]
    pub text: Rgb,
}

pub const fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            accent: Rgb::hex(0xB8ACFF),
            on_accent: Rgb::hex(0x1D1D20),
            ok: Rgb::hex(0x57D38C),
            warn: Rgb::hex(0xF8E45C),
            bad: Rgb::hex(0xFF938C),
            idle: Rgb::hex(0xA8A8A8),
            window_bg: Rgb::hex(0x242424),
            // White at 8 % over the window.
            card_bg: Rgb::hex(0x353535),
            view_bg: Rgb::hex(0x1E1E1E),
            text: Rgb::hex(0xFFFFFF),
        }
    } else {
        Palette {
            accent: Rgb::hex(0x5A3EDC),
            on_accent: Rgb::hex(0xFFFFFF),
            ok: Rgb::hex(0x1E8E4E),
            warn: Rgb::hex(0x855D00),
            bad: Rgb::hex(0xC01C28),
            idle: Rgb::hex(0x5E5E5E),
            window_bg: Rgb::hex(0xFAFAFB),
            card_bg: Rgb::hex(0xFFFFFF),
            view_bg: Rgb::hex(0xFFFFFF),
            text: Rgb::hex(0x333333),
        }
    }
}

/// The colour definitions put in front of `data/style.css`.
pub fn css_colors(dark: bool) -> String {
    let p = palette(dark);
    format!(
        "@define-color tempo_accent {};\n@define-color tempo_on_accent {};\n\
         @define-color tempo_ok {};\n@define-color tempo_warn {};\n\
         @define-color tempo_bad {};\n@define-color tempo_idle {};\n",
        p.accent.css(),
        p.on_accent.css(),
        p.ok.css(),
        p.warn.css(),
        p.bad.css(),
        p.idle.css(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text: 4.5, graphics (dots, bars): 3.
    const TEXT: f64 = 4.5;
    const GRAPHIC: f64 = 3.0;

    fn check(name: &str, dark: bool, a: Rgb, b: Rgb, needed: f64) {
        let ratio = contrast(a, b);
        assert!(
            ratio >= needed,
            "{name} ({}): {} on {} is {ratio:.2}, needs {needed}",
            if dark { "dark" } else { "light" },
            a.css(),
            b.css()
        );
    }

    #[test]
    fn contrast_of_known_pairs() {
        let ratio = contrast(Rgb::hex(0x000000), Rgb::hex(0xFFFFFF));
        assert!((ratio - 21.0).abs() < 0.01);
        assert!((contrast(Rgb::hex(0x777777), Rgb::hex(0xFFFFFF)) - 4.48).abs() < 0.02);
    }

    #[test]
    fn every_text_pair_is_readable_in_both_themes() {
        for dark in [false, true] {
            let p = palette(dark);
            let banner = p.accent.over(p.window_bg, BANNER_TINT);
            let chip = p.accent.over(p.card_bg, CHIP_TINT);
            let failed_card = p.bad.over(p.card_bg, TINT);
            check("accent on window", dark, p.accent, p.window_bg, TEXT);
            check("accent on banner", dark, p.accent, banner, TEXT);
            check("accent on card", dark, p.accent, p.card_bg, TEXT);
            check("accent on chip", dark, p.accent, chip, TEXT);
            check("text on accent", dark, p.on_accent, p.accent, TEXT);
            check("failed on card", dark, p.bad, p.card_bg, TEXT);
            check("failed on tinted card", dark, p.bad, failed_card, TEXT);
            check("text on tinted card", dark, p.text, failed_card, TEXT);
            check("warning on card", dark, p.warn, p.card_bg, TEXT);
            check("idle on card", dark, p.idle, p.card_bg, TEXT);
            check("idle on window", dark, p.idle, p.window_bg, TEXT);
            let bad_line = p.bad.over(p.view_bg, TINT);
            let warn_line = p.warn.over(p.view_bg, TINT);
            check("failed on a log line", dark, p.bad, bad_line, TEXT);
            check("warning on a log line", dark, p.warn, warn_line, TEXT);
            check("text on a failed log line", dark, p.text, bad_line, TEXT);
            check("text on a warning log line", dark, p.text, warn_line, TEXT);
        }
    }

    #[test]
    fn every_graphic_is_visible_in_both_themes() {
        for dark in [false, true] {
            let p = palette(dark);
            let failed_card = p.bad.over(p.card_bg, TINT);
            for (name, colour) in [
                ("ok", p.ok),
                ("failed", p.bad),
                ("idle", p.idle),
                ("accent", p.accent),
            ] {
                check(&format!("{name} on card"), dark, colour, p.card_bg, GRAPHIC);
            }
            check("failed on tinted card", dark, p.bad, failed_card, GRAPHIC);
            check("ok on tinted card", dark, p.ok, failed_card, GRAPHIC);
        }
    }

    #[test]
    fn the_stylesheet_uses_the_tints_that_were_checked() {
        let css = include_str!("../data/style.css");
        for (colour, tint) in [
            ("@tempo_bad", TINT),
            ("@tempo_warn", TINT),
            ("@tempo_accent", BANNER_TINT),
            ("@tempo_accent", CHIP_TINT),
        ] {
            let wanted = format!("alpha({colour}, {tint})");
            assert!(css.contains(&wanted), "style.css lacks {wanted}");
        }
    }

    #[test]
    fn colours_are_written_for_the_stylesheet() {
        let css = css_colors(false);
        assert!(css.contains("@define-color tempo_accent #5a3edc;"));
        assert!(css_colors(true).contains("@define-color tempo_bad #ff938c;"));
    }
}
