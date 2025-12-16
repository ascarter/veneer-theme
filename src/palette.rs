use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Palette {
    pub meta: Meta,
    pub colors: Colors,
    pub accents: BTreeMap<String, ColorRef>,
    pub ansi: Ansi,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Meta {
    pub name: String,
    pub version: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Colors {
    pub light: BTreeMap<String, ColorRef>,
    pub dark: BTreeMap<String, ColorRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ansi {
    pub light: AnsiScheme,
    pub dark: AnsiScheme,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnsiScheme {
    pub normal: AnsiRow,
    pub bright: AnsiRow,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnsiRow {
    pub black: ColorRef,
    pub red: ColorRef,
    pub green: ColorRef,
    pub yellow: ColorRef,
    pub blue: ColorRef,
    pub magenta: ColorRef,
    pub cyan: ColorRef,
    pub white: ColorRef,
}

/// Color references: either literal hex (#RRGGBB) or a dotted path to another key.
#[derive(Debug, Clone)]
pub enum ColorRef {
    Hex(String),
    Path(String),
}

impl<'de> Deserialize<'de> for ColorRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.starts_with('#') {
            Ok(ColorRef::Hex(s))
        } else {
            Ok(ColorRef::Path(s))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPalette {
    pub meta: Meta,
    pub colors: ResolvedColors,
    pub accents: BTreeMap<String, String>,
    pub ansi: ResolvedAnsi,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedColors {
    pub light: BTreeMap<String, String>,
    pub dark: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedAnsi {
    pub light: ResolvedAnsiScheme,
    pub dark: ResolvedAnsiScheme,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedAnsiScheme {
    pub normal: ResolvedAnsiRow,
    pub bright: ResolvedAnsiRow,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedAnsiRow {
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
}

pub fn load_palette(path: &PathBuf) -> Result<Palette> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading palette file {}", path.display()))?;
    let palette: Palette =
        toml::from_str(&raw).with_context(|| format!("parsing TOML {}", path.display()))?;
    validate_palette(&palette)?;
    Ok(palette)
}

pub fn resolve_palette(palette: &Palette) -> Result<ResolvedPalette> {
    let mut memo = HashMap::<String, String>::new();
    let mut stack = Vec::<String>::new();

    let mut resolve_color = |label: &str, cref: &ColorRef| -> Result<String> {
        match cref {
            ColorRef::Hex(raw) => normalize_hex(raw),
            ColorRef::Path(path) => resolve_path(palette, path, &mut memo, &mut stack)
                .with_context(|| format!("resolving {label} -> {path}")),
        }
    };

    let mut colors_light = BTreeMap::new();
    for (k, v) in &palette.colors.light {
        colors_light.insert(k.clone(), resolve_color(&format!("colors.light.{k}"), v)?);
    }
    let mut colors_dark = BTreeMap::new();
    for (k, v) in &palette.colors.dark {
        colors_dark.insert(k.clone(), resolve_color(&format!("colors.dark.{k}"), v)?);
    }

    let mut accents = BTreeMap::new();
    for (k, v) in &palette.accents {
        accents.insert(k.clone(), resolve_color(&format!("accents.{k}"), v)?);
    }

    let resolve_row = |row: &AnsiRow,
                       base: &str,
                       f: &mut dyn FnMut(&str, &ColorRef) -> Result<String>|
     -> Result<ResolvedAnsiRow> {
        Ok(ResolvedAnsiRow {
            black: f(&format!("{base}.black"), &row.black)?,
            red: f(&format!("{base}.red"), &row.red)?,
            green: f(&format!("{base}.green"), &row.green)?,
            yellow: f(&format!("{base}.yellow"), &row.yellow)?,
            blue: f(&format!("{base}.blue"), &row.blue)?,
            magenta: f(&format!("{base}.magenta"), &row.magenta)?,
            cyan: f(&format!("{base}.cyan"), &row.cyan)?,
            white: f(&format!("{base}.white"), &row.white)?,
        })
    };

    let ansi_light_normal = resolve_row(
        &palette.ansi.light.normal,
        "ansi.light.normal",
        &mut resolve_color,
    )?;
    let ansi_light_bright = resolve_row(
        &palette.ansi.light.bright,
        "ansi.light.bright",
        &mut resolve_color,
    )?;
    let ansi_dark_normal = resolve_row(
        &palette.ansi.dark.normal,
        "ansi.dark.normal",
        &mut resolve_color,
    )?;
    let ansi_dark_bright = resolve_row(
        &palette.ansi.dark.bright,
        "ansi.dark.bright",
        &mut resolve_color,
    )?;

    Ok(ResolvedPalette {
        meta: palette.meta.clone(),
        colors: ResolvedColors {
            light: colors_light,
            dark: colors_dark,
        },
        accents,
        ansi: ResolvedAnsi {
            light: ResolvedAnsiScheme {
                normal: ansi_light_normal,
                bright: ansi_light_bright,
            },
            dark: ResolvedAnsiScheme {
                normal: ansi_dark_normal,
                bright: ansi_dark_bright,
            },
        },
    })
}

fn resolve_path(
    palette: &Palette,
    path: &str,
    memo: &mut HashMap<String, String>,
    stack: &mut Vec<String>,
) -> Result<String> {
    if let Some(val) = memo.get(path) {
        return Ok(val.clone());
    }

    if stack.contains(&path.to_string()) {
        let cycle = stack.join(" -> ");
        bail!("cycle detected: {cycle} -> {path}");
    }

    let cref = lookup_color_ref(palette, path).with_context(|| {
        format!(
            "missing path '{}'; expected colors.*, accents.*, or ansi.*.*.*",
            path
        )
    })?;

    stack.push(path.to_string());
    let resolved = match cref {
        ColorRef::Hex(raw) => normalize_hex(raw)?,
        ColorRef::Path(next) => resolve_path(palette, next, memo, stack)?,
    };
    stack.pop();

    memo.insert(path.to_string(), resolved.clone());
    Ok(resolved)
}

fn lookup_color_ref<'a>(palette: &'a Palette, path: &str) -> Option<&'a ColorRef> {
    let mut parts = path.split('.');
    match parts.next()? {
        "colors" => {
            let tone = parts.next()?;
            let key = parts.next()?;
            if parts.next().is_some() {
                return None;
            }
            match tone {
                "light" => palette.colors.light.get(key),
                "dark" => palette.colors.dark.get(key),
                _ => None,
            }
        }
        "accents" => {
            let key = parts.next()?;
            if parts.next().is_some() {
                return None;
            }
            palette.accents.get(key)
        }
        "ansi" => {
            let tone = parts.next()?;
            let level = parts.next()?;
            let color = parts.next()?;
            if parts.next().is_some() {
                return None;
            }
            let scheme = match tone {
                "light" => &palette.ansi.light,
                "dark" => &palette.ansi.dark,
                _ => return None,
            };
            let row = match level {
                "normal" => &scheme.normal,
                "bright" => &scheme.bright,
                _ => return None,
            };
            match color {
                "black" => Some(&row.black),
                "red" => Some(&row.red),
                "green" => Some(&row.green),
                "yellow" => Some(&row.yellow),
                "blue" => Some(&row.blue),
                "magenta" => Some(&row.magenta),
                "cyan" => Some(&row.cyan),
                "white" => Some(&row.white),
                _ => None,
            }
        }
        _ => None,
    }
}

fn normalize_hex(raw: &str) -> Result<String> {
    let re = Regex::new(r"^#[0-9A-Fa-f]{6}$").unwrap();
    if !re.is_match(raw) {
        bail!("invalid hex color: {raw}");
    }
    Ok(raw.to_uppercase())
}

fn validate_palette(palette: &Palette) -> Result<()> {
    let hex_re = Regex::new(r"^#[0-9A-Fa-f]{6}$").unwrap();

    let mut check_ref = |label: &str, cref: &ColorRef| -> Result<()> {
        match cref {
            ColorRef::Hex(s) if hex_re.is_match(s) => Ok(()),
            ColorRef::Hex(s) => bail!("{label} has invalid hex color: {s}"),
            ColorRef::Path(p) if p.contains('.') => Ok(()),
            ColorRef::Path(p) => bail!("{label} path must contain at least one '.' segment: {p}"),
        }
    };

    for (k, v) in &palette.colors.light {
        check_ref(&format!("colors.light.{k}"), v)?;
    }
    for (k, v) in &palette.colors.dark {
        check_ref(&format!("colors.dark.{k}"), v)?;
    }
    for (k, v) in &palette.accents {
        check_ref(&format!("accents.{k}"), v)?;
    }

    let check_row = |row: &AnsiRow,
                     base: &str,
                     f: &mut dyn FnMut(&str, &ColorRef) -> Result<()>|
     -> Result<()> {
        f(&format!("{base}.black"), &row.black)?;
        f(&format!("{base}.red"), &row.red)?;
        f(&format!("{base}.green"), &row.green)?;
        f(&format!("{base}.yellow"), &row.yellow)?;
        f(&format!("{base}.blue"), &row.blue)?;
        f(&format!("{base}.magenta"), &row.magenta)?;
        f(&format!("{base}.cyan"), &row.cyan)?;
        f(&format!("{base}.white"), &row.white)?;
        Ok(())
    };

    check_row(
        &palette.ansi.light.normal,
        "ansi.light.normal",
        &mut check_ref,
    )?;
    check_row(
        &palette.ansi.light.bright,
        "ansi.light.bright",
        &mut check_ref,
    )?;
    check_row(
        &palette.ansi.dark.normal,
        "ansi.dark.normal",
        &mut check_ref,
    )?;
    check_row(
        &palette.ansi.dark.bright,
        "ansi.dark.bright",
        &mut check_ref,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_TOML: &str = r##"
[meta]
name = "Test"
version = "0.1.0"

[colors.light]
primary = "#111111"
secondary = "#222222"
text_primary = "#ffffff"

[colors.dark]
primary = "#000000"
secondary = "#111111"
text_primary = "#eeeeee"

[accents]
info = "#123456"
warning = "colors.light.primary"

[ansi.light.normal]
black   = "colors.light.primary"
red     = "#AA0000"
green   = "#00AA00"
yellow  = "#AAAA00"
blue    = "#0000AA"
magenta = "#AA00AA"
cyan    = "#00AAAA"
white   = "colors.light.text_primary"

[ansi.light.bright]
black   = "#333333"
red     = "#FF4444"
green   = "#44FF44"
yellow  = "#FFFF44"
blue    = "#4444FF"
magenta = "#FF44FF"
cyan    = "#44FFFF"
white   = "#FFFFFF"

[ansi.dark.normal]
black   = "colors.dark.primary"
red     = "#880000"
green   = "#008800"
yellow  = "#888800"
blue    = "#000088"
magenta = "#880088"
cyan    = "#008888"
white   = "colors.dark.text_primary"

[ansi.dark.bright]
black   = "#555555"
red     = "#FF6666"
green   = "#66FF66"
yellow  = "#FFFF66"
blue    = "#6666FF"
magenta = "#FF66FF"
cyan    = "#66FFFF"
white   = "#FFFFFF"
"##;

    #[test]
    fn parses_valid_palette() {
        let palette: Palette = toml::from_str(BASE_TOML).unwrap();
        validate_palette(&palette).unwrap();
        assert_eq!(palette.meta.name, "Test");
    }

    #[test]
    fn rejects_bad_hex() {
        let bad = BASE_TOML.replace("#AA0000", "#GGGGGG");
        let palette: Palette = toml::from_str(&bad).unwrap();
        let err = validate_palette(&palette).unwrap_err();
        assert!(
            err.to_string().contains("invalid hex color"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_path_without_dot() {
        let bad = BASE_TOML.replace("colors.light.primary", "colors_light_primary");
        let palette: Palette = toml::from_str(&bad).unwrap();
        let err = validate_palette(&palette).unwrap_err();
        assert!(
            err.to_string()
                .contains("must contain at least one '.' segment"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolves_paths_to_hex() {
        let palette: Palette = toml::from_str(BASE_TOML).unwrap();
        let resolved = resolve_palette(&palette).unwrap();
        assert_eq!(
            resolved.accents.get("warning").unwrap(),
            "#111111",
            "warning should resolve to colors.light.primary"
        );
        assert_eq!(
            resolved.colors.light.get("text_primary").unwrap(),
            "#FFFFFF"
        );
    }

    #[test]
    fn detects_missing_path() {
        let bad = BASE_TOML.replace("colors.light.primary", "colors.light.missing");
        let palette: Palette = toml::from_str(&bad).unwrap();
        let err = resolve_palette(&palette).unwrap_err();
        assert!(
            format!("{err:#}").contains("missing path"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn detects_cycles() {
        let bad = BASE_TOML
            .replace(
                "warning = \"colors.light.primary\"",
                "warning = \"accents.info\"",
            )
            .replace("info = \"#123456\"", "info = \"accents.warning\"");
        let palette: Palette = toml::from_str(&bad).unwrap();
        let err = resolve_palette(&palette).unwrap_err();
        assert!(
            format!("{err:#}").contains("cycle detected"),
            "unexpected error: {err:#}"
        );
    }
}
