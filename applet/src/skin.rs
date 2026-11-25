use cosmic::iced::Color;
use maprisma_video_translator_control::tr;

pub const SKIN_KEY: &str = "skin";
pub const SKINS: [&str; 6] = ["native", "synthwave", "cozy", "terminal", "disco", "zen"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinStyle {
    pub accent: Color,
    pub accent_rgb: [u8; 3],
    pub border_tint: Color,
    pub bg_tint: Color,
    pub window_bg: [u8; 3],
    pub foreground: [u8; 3],
    pub border_width: f32,
}

impl SkinStyle {
    pub fn palette(&self) -> crate::look::Palette {
        crate::look::Palette {
            accent: Some(self.accent_rgb), background: Some(self.window_bg), foreground: Some(self.foreground),
            red: None, yellow: None, green: None,
        }
    }
}

pub fn normalize_skin(skin: &str) -> &'static str {
    SKINS.iter().find(|s| **s == skin).copied().unwrap_or("native")
}

pub fn skin_style(skin: &str) -> Option<SkinStyle> {
    let (accent_rgb, border, bg, window_bg, foreground, border_width) = match normalize_skin(skin) {
        "synthwave" => ([0xff, 0x2e, 0x93], [0x00, 0xe5, 0xff], [0x3a, 0x12, 0x59], [0x13, 0x09, 0x24], [0xf4, 0xea, 0xff], 1.5),
        "cozy" => ([0xf0, 0x9a, 0x36], [0xe0, 0x82, 0x2a], [0x59, 0x30, 0x12], [0x21, 0x15, 0x0e], [0xf5, 0xe8, 0xd6], 1.25),
        "terminal" => ([0x2c, 0xe8, 0x68], [0x22, 0xc5, 0x5e], [0x0d, 0x38, 0x1a], [0x09, 0x13, 0x0b], [0xd2, 0xfc, 0xe0], 1.25),
        "disco" => ([0xd9, 0x46, 0xef], [0xfa, 0xcc, 0x15], [0x4c, 0x16, 0x66], [0x1c, 0x0c, 0x2b], [0xf8, 0xec, 0xff], 1.5),
        "zen" => ([0x52, 0xb7, 0x98], [0x64, 0xc2, 0xa4], [0x1e, 0x38, 0x32], [0x14, 0x1d, 0x1c], [0xe2, 0xf0, 0xec], 1.0),
        _ => return None,
    };
    Some(SkinStyle {
        accent: Color::from_rgb8(accent_rgb[0], accent_rgb[1], accent_rgb[2]), accent_rgb,
        border_tint: Color::from_rgb8(border[0], border[1], border[2]),
        bg_tint: Color::from_rgb8(bg[0], bg[1], bg[2]), window_bg, foreground, border_width,
    })
}

pub fn skin_label(skin: &str) -> String {
    match normalize_skin(skin) {
        "synthwave" => tr("Synthwave"), "cozy" => tr("Cozy"), "terminal" => tr("Terminal"),
        "disco" => tr("Disco"), "zen" => tr("Zen"), _ => tr("Native"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palettes_and_normalization() {
        assert_eq!(normalize_skin("bogus"), "native");
        assert_eq!(skin_style("native"), None);
        let expected = [
            ("synthwave", [0xff, 0x2e, 0x93], [0x00, 0xe5, 0xff], [0x3a, 0x12, 0x59], [0x13, 0x09, 0x24], [0xf4, 0xea, 0xff], 1.5),
            ("cozy", [0xf0, 0x9a, 0x36], [0xe0, 0x82, 0x2a], [0x59, 0x30, 0x12], [0x21, 0x15, 0x0e], [0xf5, 0xe8, 0xd6], 1.25),
            ("terminal", [0x2c, 0xe8, 0x68], [0x22, 0xc5, 0x5e], [0x0d, 0x38, 0x1a], [0x09, 0x13, 0x0b], [0xd2, 0xfc, 0xe0], 1.25),
            ("disco", [0xd9, 0x46, 0xef], [0xfa, 0xcc, 0x15], [0x4c, 0x16, 0x66], [0x1c, 0x0c, 0x2b], [0xf8, 0xec, 0xff], 1.5),
            ("zen", [0x52, 0xb7, 0x98], [0x64, 0xc2, 0xa4], [0x1e, 0x38, 0x32], [0x14, 0x1d, 0x1c], [0xe2, 0xf0, 0xec], 1.0),
        ];
        for (name, accent, border, bg, window, foreground, width) in expected {
            let st = skin_style(name).unwrap();
            assert_eq!((st.accent_rgb, st.window_bg, st.foreground, st.border_width), (accent, window, foreground, width));
            assert_eq!(st.border_tint, Color::from_rgb8(border[0], border[1], border[2]));
            assert_eq!(st.bg_tint, Color::from_rgb8(bg[0], bg[1], bg[2]));
        }
        for skin in SKINS {
            assert_eq!(normalize_skin(skin), skin);
            assert!(!skin_label(skin).is_empty());
            if let Some(style) = skin_style(skin) {
                assert_eq!(style.palette().accent, Some(style.accent_rgb));
                assert_eq!(style.palette().background, Some(style.window_bg));
                assert!(crate::look::theme_from(&style.palette(), Some(12.0)).cosmic().is_dark);
            }
        }
    }
}
