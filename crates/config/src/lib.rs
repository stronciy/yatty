use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;

const DEFAULT_THEME: &str = "default";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl FromStr for Rgb {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("invalid color `{s}`, expected #rrggbb"));
        }
        let value = u32::from_str_radix(hex, 16).map_err(|err| err.to_string())?;
        Ok(Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8))
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Font {
    pub family: String,
    pub size: f32,
    pub weight: u16,
    pub ligatures: bool,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: "JetBrains Mono".into(),
            size: 14.0,
            weight: 400,
            ligatures: true,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Colors {
    pub theme: Option<String>,
    pub foreground: Option<Rgb>,
    pub background: Option<Rgb>,
    pub cursor: Option<Rgb>,
    pub palette: Option<Vec<Rgb>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Footer {
    pub enabled: bool,
}

impl Default for Footer {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub font: Font,
    pub colors: Colors,
    pub footer: Footer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub palette: [Rgb; 16],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: Rgb(0xC0, 0xC5, 0xD0),
            background: Rgb(0x1A, 0x1B, 0x1F),
            cursor: Rgb(0x7A, 0xA2, 0xF7),
            palette: [
                Rgb(0x15, 0x16, 0x1A),
                Rgb(0xF7, 0x76, 0x8E),
                Rgb(0x9E, 0xCE, 0x6A),
                Rgb(0xE0, 0xAF, 0x68),
                Rgb(0x7A, 0xA2, 0xF7),
                Rgb(0xBB, 0x9A, 0xF7),
                Rgb(0x7D, 0xCF, 0xFF),
                Rgb(0xA9, 0xB1, 0xD6),
                Rgb(0x41, 0x48, 0x68),
                Rgb(0xF7, 0x76, 0x8E),
                Rgb(0x9E, 0xCE, 0x6A),
                Rgb(0xE0, 0xAF, 0x68),
                Rgb(0x7A, 0xA2, 0xF7),
                Rgb(0xBB, 0x9A, 0xF7),
                Rgb(0x7D, 0xCF, 0xFF),
                Rgb(0xC0, 0xCA, 0xF5),
            ],
        }
    }
}

impl Theme {
    fn apply(&mut self, colors: &Colors, warnings: &mut Vec<String>) {
        if let Some(color) = colors.foreground {
            self.foreground = color;
        }
        if let Some(color) = colors.background {
            self.background = color;
        }
        if let Some(color) = colors.cursor {
            self.cursor = color;
        }
        if let Some(palette) = &colors.palette {
            match <[Rgb; 16]>::try_from(palette.as_slice()) {
                Ok(palette) => self.palette = palette,
                Err(_) => warnings.push(format!(
                    "palette must have 16 colors, got {}",
                    palette.len()
                )),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub font: Font,
    pub theme: Theme,
    pub footer: Footer,
    pub warnings: Vec<String>,
}

impl Settings {
    pub fn load(config_dir: &Path) -> Self {
        let mut warnings = Vec::new();
        let config =
            read_config_file(&config_dir.join("config.toml"), &mut warnings).unwrap_or_default();

        let mut theme = Theme::default();
        let theme_name = config.colors.theme.as_deref().unwrap_or(DEFAULT_THEME);
        if theme_name != DEFAULT_THEME {
            let path = config_dir.join("themes").join(format!("{theme_name}.toml"));
            match read_colors_file(&path, &mut warnings) {
                Some(colors) => theme.apply(&colors, &mut warnings),
                None => warnings.push(format!(
                    "theme `{theme_name}` not found at {}",
                    path.display()
                )),
            }
        }
        theme.apply(&config.colors, &mut warnings);

        let mut font = config.font;
        if font.family.trim().is_empty() {
            warnings.push("font.family is empty, using default".into());
            font.family = Font::default().family;
        }
        if !(4.0..=200.0).contains(&font.size) {
            warnings.push(format!(
                "font.size {} is out of range, using default",
                font.size
            ));
            font.size = Font::default().size;
        }

        Self {
            font,
            theme,
            footer: config.footer,
            warnings,
        }
    }
}

pub fn default_config_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".config").join("yatty")
}

fn read_config_file(path: &Path, warnings: &mut Vec<String>) -> Option<Config> {
    let text = read_optional(path, warnings)?;
    match toml::from_str(&text) {
        Ok(config) => Some(config),
        Err(err) => {
            warnings.push(format!("{}: {err}; using defaults", path.display()));
            None
        }
    }
}

fn read_colors_file(path: &Path, warnings: &mut Vec<String>) -> Option<Colors> {
    let text = read_optional(path, warnings)?;
    match toml::from_str(&text) {
        Ok(colors) => Some(colors),
        Err(err) => {
            warnings.push(format!("{}: {err}", path.display()));
            None
        }
    }
}

fn read_optional(path: &Path, warnings: &mut Vec<String>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            warnings.push(format!("{}: {err}", path.display()));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("yatty-config-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_config_uses_defaults() {
        let dir = temp_dir("missing");
        let settings = Settings::load(&dir);
        assert_eq!(settings.font.family, "JetBrains Mono");
        assert_eq!(settings.font.size, 14.0);
        assert_eq!(settings.font.weight, 400);
        assert!(settings.font.ligatures);
        assert_eq!(settings.theme, Theme::default());
        assert!(settings.warnings.is_empty());
    }

    #[test]
    fn parses_font_and_inline_colors() {
        let dir = temp_dir("inline");
        fs::write(
            dir.join("config.toml"),
            "[font]\nfamily = \"Menlo\"\nsize = 16.0\nligatures = false\n\n[colors]\nbackground = \"#000000\"\n",
        )
        .unwrap();
        let settings = Settings::load(&dir);
        assert_eq!(settings.font.family, "Menlo");
        assert_eq!(settings.font.size, 16.0);
        assert!(!settings.font.ligatures);
        assert_eq!(settings.theme.background, Rgb(0, 0, 0));
        assert_eq!(settings.theme.foreground, Theme::default().foreground);
    }

    #[test]
    fn named_theme_is_applied_and_inline_overrides_it() {
        let dir = temp_dir("theme");
        fs::create_dir_all(dir.join("themes")).unwrap();
        fs::write(
            dir.join("themes").join("night.toml"),
            "foreground = \"#112233\"\nbackground = \"#223344\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("config.toml"),
            "[colors]\ntheme = \"night\"\nbackground = \"#334455\"\n",
        )
        .unwrap();
        let settings = Settings::load(&dir);
        assert_eq!(settings.theme.foreground, Rgb(0x11, 0x22, 0x33));
        assert_eq!(settings.theme.background, Rgb(0x33, 0x44, 0x55));
        assert!(settings.warnings.is_empty());
    }

    #[test]
    fn invalid_values_fall_back_with_warnings() {
        let dir = temp_dir("invalid");
        fs::write(
            dir.join("config.toml"),
            "[font]\nsize = 1.0\n\n[colors]\nforeground = \"blue\"\n",
        )
        .unwrap();
        let settings = Settings::load(&dir);
        assert_eq!(settings.font.size, 14.0);
        assert!(!settings.warnings.is_empty());
    }

    #[test]
    fn broken_toml_uses_defaults_with_warning() {
        let dir = temp_dir("broken");
        fs::write(dir.join("config.toml"), "[font\nfamily =").unwrap();
        let settings = Settings::load(&dir);
        assert_eq!(settings.font, Font::default());
        assert_eq!(settings.warnings.len(), 1);
    }

    #[test]
    fn palette_must_have_sixteen_colors() {
        let dir = temp_dir("palette");
        fs::write(
            dir.join("config.toml"),
            "[colors]\npalette = [\"#000000\"]\n",
        )
        .unwrap();
        let settings = Settings::load(&dir);
        assert_eq!(settings.theme.palette, Theme::default().palette);
        assert_eq!(settings.warnings.len(), 1);
    }
}
