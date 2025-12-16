use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde_json::Value;
use tera::{Context as TeraContext, Tera};

use crate::palette::{ResolvedPalette, load_palette, resolve_palette};

pub fn build(
    palette_path: &PathBuf,
    template_path: &PathBuf,
    dest: Option<&PathBuf>,
) -> Result<()> {
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

    let rendered = tera
        .render("inline", &ctx)
        .with_context(|| format!("rendering template {}", template_path.display()))?;

    let out_path = determine_out_path(template_path, dest)?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    fs::write(&out_path, rendered).with_context(|| format!("writing {}", out_path.display()))?;
    Ok(())
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
    tera.register_filter("lowercase", lowercase_filter);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase_helper_downcases_text() {
        use std::collections::HashMap;

        let args = HashMap::new();
        let out = lowercase_filter(&Value::String("Emerald MIX".into()), &args).unwrap();
        assert_eq!(out, Value::String("emerald mix".into()));
    }

    #[test]
    fn strips_tera_extension_for_default_output() {
        let path = Path::new("templates/vscode/themes/theme.json.tera");
        let out = strip_tera_extension(path.file_name().unwrap());
        assert_eq!(out, std::ffi::OsString::from("theme.json"));
    }
}
