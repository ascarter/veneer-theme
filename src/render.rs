use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::Value;
use tera::{Context as TeraContext, Tera};
use walkdir::WalkDir;

use crate::palette::{ResolvedPalette, load_palette, resolve_palette};

pub fn build(palette_path: &PathBuf, src: &PathBuf, dest: Option<&PathBuf>) -> Result<()> {
    let ctx = {
        let palette = load_palette(palette_path)?;
        let resolved = resolve_palette(&palette)?;
        build_context(&resolved)?
    };

    let src_kind = detect_source_kind(src)?;
    let (base, templates) = collect_templates(&src_kind)?;

    if templates.is_empty() {
        anyhow::bail!("no templates matched {}", src.display());
    }

    match src_kind {
        SourceKind::SingleFile { path } => {
            let out_path = determine_out_path(&path, dest)?;
            render_one(&path, &ctx, &out_path)
        }
        _ => {
            let dest_mode = resolve_dest_mode(dest)?;
            for path in templates {
                let rel = path.strip_prefix(&base).unwrap_or(path.as_path());
                let rel = strip_tera_from_path(rel);
                let out_path = match &dest_mode {
                    DestMode::Directory(dir) => dir.join(&rel),
                    DestMode::Prefix(prefix) => {
                        let combined = format!("{}{}", prefix.display(), rel.to_string_lossy());
                        PathBuf::from(combined)
                    }
                };
                render_one(&path, &ctx, &out_path)?;
            }
            Ok(())
        }
    }
}

pub fn check_single(palette_path: &PathBuf, template_path: &PathBuf) -> Result<()> {
    let palette = load_palette(palette_path)?;
    let resolved = resolve_palette(&palette)?;
    let ctx = build_context(&resolved)?;

    let template = fs::read_to_string(template_path)
        .with_context(|| format!("reading {}", template_path.display()))?;

    let mut tera = Tera::default();
    tera.add_raw_template("inline", &template)
        .with_context(|| format!("registering template {}", template_path.display()))?;
    tera.autoescape_on(vec![]);
    register_helpers(&mut tera);

    tera.render("inline", &ctx)
        .with_context(|| format!("rendering template {}", template_path.display()))?;
    Ok(())
}

fn determine_out_path(template_path: &Path, dest: Option<&PathBuf>) -> Result<PathBuf> {
    // Base filename: template filename with .tera removed.
    let file_name = template_path
        .file_name()
        .map(strip_tera_extension)
        .unwrap_or_else(|| std::ffi::OsString::from("output"));

    let out_path = match dest {
        Some(path) => {
            if path.is_dir() {
                path.join(file_name)
            } else {
                path.clone()
            }
        }
        None => std::env::current_dir()
            .context("reading current directory")?
            .join(file_name),
    };

    Ok(out_path)
}

fn strip_tera_extension(os: &std::ffi::OsStr) -> std::ffi::OsString {
    let s = os.to_string_lossy();
    if let Some(stripped) = s.strip_suffix(".tera") {
        return std::ffi::OsString::from(stripped);
    }
    os.to_owned()
}

fn build_context(resolved: &ResolvedPalette) -> Result<TeraContext> {
    let mut ctx = TeraContext::new();
    ctx.try_insert("meta", &resolved.meta)?;
    ctx.try_insert("light", &resolved.colors.light)?;
    ctx.try_insert("dark", &resolved.colors.dark)?;
    ctx.try_insert("accents", &resolved.accents)?;
    ctx.try_insert("ansi", &resolved.ansi)?;
    Ok(ctx)
}

fn register_helpers(tera: &mut Tera) {
    tera.register_function("with_alpha", with_alpha);
    tera.register_function("rgba", rgba);
    tera.register_function("hsla", hsla);
    tera.register_function("rgba_floats", rgba_floats);
    tera.register_function("mix", mix);
    tera.register_function("palette_color", palette_color);
    tera.register_function("ron_color", ron_color);
    tera.register_function("ron_rgb", ron_rgb);
    tera.register_function("blend", blend);
    tera.register_filter("lowercase", lowercase_filter);
}

fn render_one(template_path: &Path, ctx: &TeraContext, out_path: &Path) -> Result<()> {
    let template = fs::read_to_string(template_path)
        .with_context(|| format!("reading {}", template_path.display()))?;

    let mut tera = Tera::default();
    tera.add_raw_template("inline", &template)
        .with_context(|| format!("registering template {}", template_path.display()))?;
    tera.autoescape_on(vec![]);
    register_helpers(&mut tera);

    let rendered = tera
        .render("inline", ctx)
        .with_context(|| format!("rendering template {}", template_path.display()))?;

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    fs::write(out_path, rendered).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
}

fn with_alpha(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let color = expect_string(args, "color")?;
    let alpha = expect_number(args, "alpha")?;
    let hex = with_alpha_hex(&color, alpha)?;
    Ok(Value::String(hex))
}

fn rgba(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let color = expect_string(args, "color")?;
    let alpha = expect_number(args, "alpha")?;
    let (r, g, b) = hex_to_rgb(&color)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {color}")))?;
    let s = format!("rgba({r}, {g}, {b}, {alpha:.3})");
    Ok(Value::String(s))
}

fn hsla(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let color = expect_string(args, "color")?;
    let alpha = expect_number(args, "alpha")?;
    let (r, g, b) = hex_to_rgb(&color)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {color}")))?;
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let s = format!("hsla({h:.3}, {s:.3}, {l:.3}, {alpha:.3})");
    Ok(Value::String(s))
}

fn rgba_floats(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let color = expect_string(args, "color")?;
    let alpha = expect_number(args, "alpha")?;
    let (r, g, b) = hex_to_rgb(&color)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {color}")))?;

    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    Ok(Value::String(format!("{r:.6} {g:.6} {b:.6} {alpha:.6}")))
}

fn ron_color(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let color = expect_string(args, "color")?;
    let alpha = args
        .get("alpha")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0) as f32;
    let (r, g, b) = hex_to_rgb(&color)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {color}")))?;
    Ok(Value::String(format!(
        "(\n        red: {:.7},\n        green: {:.7},\n        blue: {:.7},\n        alpha: {:.7},\n    )",
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        alpha,
    )))
}

/// Linear interpolation (lerp) between two sRGB hex colors.
/// Operates directly on the sRGB channel values (0–255) without color-space conversion.
fn mix(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let a = expect_string(args, "a")?;
    let b = expect_string(args, "b")?;
    let t = expect_number(args, "t")?;
    if !(0.0..=1.0).contains(&t) {
        return Err(tera::Error::msg("mix: t must be between 0.0 and 1.0"));
    }
    let (ar, ag, ab) = hex_to_rgb(&a)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {a}")))?;
    let (br, bg, bb) = hex_to_rgb(&b)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {b}")))?;
    let lerp = |a: u8, b: u8| -> u8 {
        (a as f32 + t * (b as f32 - a as f32)).round() as u8
    };
    let (r, g, b) = (lerp(ar, br), lerp(ag, bg), lerp(ab, bb));
    Ok(Value::String(format!("#{r:02X}{g:02X}{b:02X}")))
}

/// Generate a reasonable color for a named hue relative to a background color.
///
/// `name` maps to a hue: red, orange, yellow, green, cyan, blue, indigo, purple, pink, warm_grey.
/// `background` is a `#RRGGBB` hex used to determine if we are on a dark or light surface.
/// On a dark background (L < 0.5) the result is lightened (L ≈ 0.65); on a light background
/// (L ≥ 0.5) it is darkened (L ≈ 0.35). Saturation is ~0.55 for chromatic names, ~0.08 for
/// warm_grey. Returns a `#RRGGBB` hex, composable with `ron_color`.
fn palette_color(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let name = expect_string(args, "name")?;
    let background = expect_string(args, "background")?;

    let (hue, saturation): (f32, f32) = match name.to_lowercase().as_str() {
        "red" => (0.0, 0.55),
        "orange" => (30.0, 0.55),
        "yellow" => (60.0, 0.55),
        "green" => (120.0, 0.55),
        "cyan" => (180.0, 0.55),
        "blue" => (210.0, 0.55),
        "indigo" => (245.0, 0.55),
        "purple" => (270.0, 0.55),
        "pink" => (330.0, 0.55),
        "warm_grey" | "warmgrey" | "warm grey" => (30.0, 0.08),
        other => {
            return Err(tera::Error::msg(format!(
                "palette_color: unknown color name '{other}'. \
                 Valid names: red, orange, yellow, green, cyan, blue, indigo, purple, pink, warm_grey"
            )));
        }
    };

    let (br, bg, bb) = hex_to_rgb(&background)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {background}")))?;
    let (_, _, bg_lightness) = rgb_to_hsl(br, bg, bb);

    let lightness = if bg_lightness < 0.5 { 0.65 } else { 0.35 };

    let (r, g, b) = hsl_to_rgb(hue / 360.0, saturation, lightness);
    Ok(Value::String(format!("#{r:02X}{g:02X}{b:02X}")))
}

fn ron_rgb(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let color = expect_string(args, "color")?;
    let (r, g, b) = hex_to_rgb(&color)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {color}")))?;
    Ok(Value::String(format!(
        "(\n        red: {:.7},\n        green: {:.7},\n        blue: {:.7},\n    )",
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
    )))
}

/// Alpha-blend `color` at `alpha` over `background`, returning an opaque #RRGGBB hex.
/// Formula: out_channel = round(alpha * fg + (1 - alpha) * bg) per channel.
fn blend(args: &std::collections::HashMap<String, Value>) -> tera::Result<Value> {
    let color = expect_string(args, "color")?;
    let background = expect_string(args, "background")?;
    let alpha = expect_number(args, "alpha")?;

    if !(0.0..=1.0).contains(&alpha) {
        return Err(tera::Error::msg("alpha must be between 0.0 and 1.0"));
    }

    let (fr, fg, fb) = hex_to_rgb(&color)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {color}")))?;
    let (br, bg, bb) = hex_to_rgb(&background)
        .ok_or_else(|| tera::Error::msg(format!("invalid hex color: {background}")))?;

    let blend_channel = |f: u8, b: u8| -> u8 {
        (alpha * f as f32 + (1.0 - alpha) * b as f32).round() as u8
    };

    let r = blend_channel(fr, br);
    let g = blend_channel(fg, bg);
    let b = blend_channel(fb, bb);

    Ok(Value::String(format!("#{r:02X}{g:02X}{b:02X}")))
}

fn lowercase_filter(
    value: &Value,
    _: &std::collections::HashMap<String, Value>,
) -> tera::Result<Value> {
    match value {
        Value::String(s) => Ok(Value::String(s.to_lowercase())),
        other => Err(tera::Error::msg(format!(
            "lowercase filter expects a string, got {other:?}"
        ))),
    }
}

fn expect_string(
    args: &std::collections::HashMap<String, Value>,
    key: &str,
) -> tera::Result<String> {
    match args.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(tera::Error::msg(format!(
            "missing or invalid string arg '{key}'"
        ))),
    }
}

fn expect_number(args: &std::collections::HashMap<String, Value>, key: &str) -> tera::Result<f32> {
    match args.get(key) {
        Some(Value::Number(n)) => n
            .as_f64()
            .map(|v| v as f32)
            .ok_or_else(|| tera::Error::msg(format!("invalid numeric arg '{key}'"))),
        _ => Err(tera::Error::msg(format!(
            "missing or invalid numeric arg '{key}'"
        ))),
    }
}

fn with_alpha_hex(hex: &str, alpha: f32) -> tera::Result<String> {
    if !(0.0..=1.0).contains(&alpha) {
        return Err(tera::Error::msg("alpha must be between 0.0 and 1.0"));
    }
    let (r, g, b) =
        hex_to_rgb(hex).ok_or_else(|| tera::Error::msg(format!("invalid hex color: {hex}")))?;
    let a = (alpha * 255.0).round() as u8;
    Ok(format!("#{r:02X}{g:02X}{b:02X}{a:02X}"))
}

fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    if hex.len() != 7 || !hex.starts_with('#') {
        return None;
    }
    let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
    let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
    let b = u8::from_str_radix(&hex[5..7], 16).ok()?;
    Some((r, g, b))
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let mut h = if (max - r).abs() < f32::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h /= 6.0;

    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s.abs() < f32::EPSILON {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue_to_rgb = |mut t: f32| -> f32 {
        if t < 0.0 { t += 1.0; }
        if t > 1.0 { t -= 1.0; }
        if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
        if t < 1.0 / 2.0 { return q; }
        if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
        p
    };
    let r = (hue_to_rgb(h + 1.0 / 3.0) * 255.0).round() as u8;
    let g = (hue_to_rgb(h) * 255.0).round() as u8;
    let b = (hue_to_rgb(h - 1.0 / 3.0) * 255.0).round() as u8;
    (r, g, b)
}

#[derive(Clone)]
enum SourceKind {
    SingleFile { path: PathBuf },
    Directory { root: PathBuf },
    Glob { pattern: String, base: PathBuf },
}

fn detect_source_kind(src: &PathBuf) -> Result<SourceKind> {
    let src_str = src.to_string_lossy();
    if has_glob_chars(&src_str) {
        let base = glob_base(&src_str);
        return Ok(SourceKind::Glob {
            pattern: src_str.to_string(),
            base,
        });
    }

    if src.is_dir() {
        return Ok(SourceKind::Directory { root: src.clone() });
    }

    Ok(SourceKind::SingleFile { path: src.clone() })
}

fn has_glob_chars(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[') || s.contains('{')
}

fn glob_base(pattern: &str) -> PathBuf {
    let idx = pattern
        .find(|c| matches!(c, '*' | '?' | '[' | '{'))
        .unwrap_or(pattern.len());
    let before = &pattern[..idx];
    let base = match before.rfind(std::path::MAIN_SEPARATOR) {
        Some(pos) => &before[..=pos],
        None => "",
    };
    PathBuf::from(base)
}

fn collect_templates(kind: &SourceKind) -> Result<(PathBuf, Vec<PathBuf>)> {
    match kind {
        SourceKind::SingleFile { path } => Ok((
            path.parent().unwrap_or_else(|| Path::new("")).into(),
            vec![path.clone()],
        )),
        SourceKind::Directory { root } => {
            let mut paths = Vec::new();
            for entry in WalkDir::new(root)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
            {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("tera") {
                    paths.push(entry.path().to_path_buf());
                }
            }
            Ok((root.clone(), paths))
        }
        SourceKind::Glob { pattern, base } => {
            let mut paths = Vec::new();
            for entry in glob::glob(pattern)? {
                let path = entry?;
                if path.is_file() {
                    paths.push(path);
                }
            }
            Ok((base.clone(), paths))
        }
    }
}

fn strip_tera_from_path(path: &Path) -> PathBuf {
    let mut new = path.to_path_buf();
    if let Some(name) = path.file_name() {
        let stripped = strip_tera_extension(name);
        new.set_file_name(stripped);
    }
    new
}

enum DestMode {
    Directory(PathBuf),
    Prefix(PathBuf),
}

fn resolve_dest_mode(dest: Option<&PathBuf>) -> Result<DestMode> {
    let sep = std::path::MAIN_SEPARATOR;
    let mode = match dest {
        None => DestMode::Directory(std::env::current_dir().context("reading current directory")?),
        Some(path) => {
            let s = path.to_string_lossy();
            if path.is_dir() || s.ends_with(sep) {
                DestMode::Directory(path.clone())
            } else {
                DestMode::Prefix(path.clone())
            }
        }
    };
    Ok(mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const MINIMAL_PALETTE: &str = r##"
[meta]
name = "Test"
version = "0.0.1"

[colors.light]
background = "#000000"

[colors.dark]
background = "#000000"

[accents]
primary = "#111111"

[ansi.light.normal]
black="#000000"
red="#000000"
green="#000000"
yellow="#000000"
blue="#000000"
magenta="#000000"
cyan="#000000"
white="#000000"

[ansi.light.bright]
black="#111111"
red="#111111"
green="#111111"
yellow="#111111"
blue="#111111"
magenta="#111111"
cyan="#111111"
white="#111111"

[ansi.dark.normal]
black="#000000"
red="#000000"
green="#000000"
yellow="#000000"
blue="#000000"
magenta="#000000"
cyan="#000000"
white="#000000"

[ansi.dark.bright]
black="#111111"
red="#111111"
green="#111111"
yellow="#111111"
blue="#111111"
magenta="#111111"
cyan="#111111"
white="#111111"
"##;

    #[test]
    fn blend_composites_over_background() {
        use std::collections::HashMap;

        // 50% black (#000000) over white (#FFFFFF) -> mid-gray (#808080)
        let mut args = HashMap::new();
        args.insert("color".into(), Value::String("#000000".into()));
        args.insert("background".into(), Value::String("#FFFFFF".into()));
        args.insert("alpha".into(), serde_json::json!(0.5));
        let out = blend(&args).unwrap();
        assert_eq!(out, Value::String("#808080".into()));

        // alpha=1.0 -> color unchanged
        let mut args = HashMap::new();
        args.insert("color".into(), Value::String("#AABBCC".into()));
        args.insert("background".into(), Value::String("#FFFFFF".into()));
        args.insert("alpha".into(), serde_json::json!(1.0));
        let out = blend(&args).unwrap();
        assert_eq!(out, Value::String("#AABBCC".into()));

        // alpha=0.0 -> background unchanged
        let mut args = HashMap::new();
        args.insert("color".into(), Value::String("#AABBCC".into()));
        args.insert("background".into(), Value::String("#112233".into()));
        args.insert("alpha".into(), serde_json::json!(0.0));
        let out = blend(&args).unwrap();
        assert_eq!(out, Value::String("#112233".into()));
    }

    #[test]
    fn lowercase_helper_downcases_text() {
        use std::collections::HashMap;

        let args = HashMap::new();
        let out = lowercase_filter(&Value::String("Emerald MIX".into()), &args).unwrap();
        assert_eq!(out, Value::String("emerald mix".into()));
    }

    #[test]
    fn ron_color_formats_floats() {
        use std::collections::HashMap;

        // Default alpha = 1.0
        let mut args = HashMap::new();
        args.insert("color".into(), Value::String("#6390CF".into()));
        let out = ron_color(&args).unwrap();
        assert!(out.as_str().unwrap().contains("red: 0.3882353"));
        assert!(out.as_str().unwrap().contains("alpha: 1.0000000"));

        // Explicit alpha
        let mut args = HashMap::new();
        args.insert("color".into(), Value::String("#FFFFFF".into()));
        args.insert("alpha".into(), serde_json::json!(0.5));
        let out = ron_color(&args).unwrap();
        assert!(out.as_str().unwrap().contains("red: 1.0000000"));
        assert!(out.as_str().unwrap().contains("alpha: 0.5000000"));
    }

    #[test]
    fn mix_interpolates_srgb_channels() {
        use std::collections::HashMap;

        // t=0.0 returns a
        let mut args = HashMap::new();
        args.insert("a".into(), Value::String("#000000".into()));
        args.insert("b".into(), Value::String("#FFFFFF".into()));
        args.insert("t".into(), serde_json::json!(0.0));
        assert_eq!(mix(&args).unwrap(), Value::String("#000000".into()));

        // t=1.0 returns b
        let mut args = HashMap::new();
        args.insert("a".into(), Value::String("#000000".into()));
        args.insert("b".into(), Value::String("#FFFFFF".into()));
        args.insert("t".into(), serde_json::json!(1.0));
        assert_eq!(mix(&args).unwrap(), Value::String("#FFFFFF".into()));

        // t=0.5 midpoint between black and white is mid-gray
        let mut args = HashMap::new();
        args.insert("a".into(), Value::String("#000000".into()));
        args.insert("b".into(), Value::String("#FFFFFF".into()));
        args.insert("t".into(), serde_json::json!(0.5));
        assert_eq!(mix(&args).unwrap(), Value::String("#808080".into()));

        // t out of range is rejected
        let mut args = HashMap::new();
        args.insert("a".into(), Value::String("#000000".into()));
        args.insert("b".into(), Value::String("#FFFFFF".into()));
        args.insert("t".into(), serde_json::json!(1.5));
        assert!(mix(&args).is_err());
    }

    #[test]
    fn ron_rgb_formats_floats_without_alpha() {
        use std::collections::HashMap;

        let mut args = HashMap::new();
        args.insert("color".into(), Value::String("#6390CF".into()));
        let out = ron_rgb(&args).unwrap();
        let s = out.as_str().unwrap();
        assert!(s.contains("red: 0.3882353"));
        assert!(s.contains("green: 0.5647059"));
        assert!(s.contains("blue: 0.8117647"));
        assert!(!s.contains("alpha"));
    }

    #[test]
    fn palette_color_dark_bg_lightens_result() {
        use std::collections::HashMap;

        // Dark background (#1C1C1C): result should be lighter (L ≈ 0.65)
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("blue".into()));
        args.insert("background".into(), Value::String("#1C1C1C".into()));
        let out = palette_color(&args).unwrap();
        let hex = out.as_str().unwrap();
        let (r, g, b) = hex_to_rgb(hex).unwrap();
        let (_, _, l) = rgb_to_hsl(r, g, b);
        assert!(l > 0.5, "expected light color on dark bg, got L={l}");
    }

    #[test]
    fn palette_color_light_bg_darkens_result() {
        use std::collections::HashMap;

        // Light background (#F0F0F0): result should be darker (L ≈ 0.35)
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("pink".into()));
        args.insert("background".into(), Value::String("#F0F0F0".into()));
        let out = palette_color(&args).unwrap();
        let hex = out.as_str().unwrap();
        let (r, g, b) = hex_to_rgb(hex).unwrap();
        let (_, _, l) = rgb_to_hsl(r, g, b);
        assert!(l < 0.5, "expected dark color on light bg, got L={l}");
    }

    #[test]
    fn palette_color_unknown_name_errors() {
        use std::collections::HashMap;

        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("chartreuse".into()));
        args.insert("background".into(), Value::String("#000000".into()));
        assert!(palette_color(&args).is_err());
    }

    #[test]
    fn strips_tera_extension_for_default_output() {
        let path = Path::new("templates/vscode/themes/theme.json.tera");
        let out = strip_tera_extension(path.file_name().unwrap());
        assert_eq!(out, std::ffi::OsString::from("theme.json"));
    }

    #[test]
    fn builds_directory_into_dest_directory() {
        let tmp = tempdir().unwrap();
        let palette_path = tmp.path().join("veneer.toml");
        fs::write(&palette_path, MINIMAL_PALETTE).unwrap();

        let src_dir = tmp.path().join("src");
        fs::create_dir_all(src_dir.join("nested")).unwrap();
        fs::write(src_dir.join("one.tera"), "Hello {{ meta.name }}").unwrap();
        fs::write(
            src_dir.join("nested").join("two.tera"),
            "World {{ meta.name }}",
        )
        .unwrap();

        let dest_dir = tmp.path().join("out");
        fs::create_dir_all(&dest_dir).unwrap();
        build(&palette_path, &src_dir, Some(&dest_dir)).unwrap();

        let one_out = dest_dir.join("one");
        let two_out = dest_dir.join("nested").join("two");
        assert_eq!(fs::read_to_string(one_out).unwrap(), "Hello Test");
        assert_eq!(fs::read_to_string(two_out).unwrap(), "World Test");
    }

    #[test]
    fn builds_glob_with_prefix() {
        let tmp = tempdir().unwrap();
        let palette_path = tmp.path().join("veneer.toml");
        fs::write(&palette_path, MINIMAL_PALETTE).unwrap();

        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("alpha.tera"), "Alpha {{ meta.name }}").unwrap();
        fs::write(src_dir.join("beta.tera"), "Beta {{ meta.name }}").unwrap();

        let pattern = src_dir.join("*.tera");
        let prefix = tmp.path().join("dist").join("theme-");

        build(&palette_path, &pattern, Some(&prefix)).unwrap();

        let alpha_out = tmp.path().join("dist").join("theme-alpha");
        let beta_out = tmp.path().join("dist").join("theme-beta");
        assert_eq!(fs::read_to_string(alpha_out).unwrap(), "Alpha Test");
        assert_eq!(fs::read_to_string(beta_out).unwrap(), "Beta Test");
    }
}
