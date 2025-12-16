use std::path::PathBuf;

use anyhow::Result;

use crate::palette::{ResolvedAnsiRow, ResolvedPalette, load_palette, resolve_palette};

pub fn run(palette_path: &PathBuf) -> Result<()> {
    let palette = load_palette(palette_path)?;
    let resolved = resolve_palette(&palette)?;
    print_palette(palette_path, &resolved);
    Ok(())
}

fn print_palette(palette_path: &PathBuf, palette: &ResolvedPalette) {
    println!(
        "Palette: {} ({})",
        palette.meta.name,
        palette_path.display()
    );
    if let Some(version) = palette.meta.version.as_ref() {
        println!("Version: {version}");
    }
    println!("Slug: {}", palette.meta.slug.as_deref().unwrap_or("<none>"));
    println!();

    let label_width = max_label_width(palette);

    print_section(
        "Colors (Light)",
        palette
            .colors
            .light
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        label_width,
    );
    print_section(
        "Colors (Dark)",
        palette
            .colors
            .dark
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        label_width,
    );
    print_section(
        "Accents",
        palette
            .accents
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        label_width,
    );

    print_section(
        "ANSI (Light / Normal)",
        ansi_row_items(&palette.ansi.light.normal),
        label_width,
    );
    print_section(
        "ANSI (Light / Bright)",
        ansi_row_items(&palette.ansi.light.bright),
        label_width,
    );
    print_section(
        "ANSI (Dark / Normal)",
        ansi_row_items(&palette.ansi.dark.normal),
        label_width,
    );
    print_section(
        "ANSI (Dark / Bright)",
        ansi_row_items(&palette.ansi.dark.bright),
        label_width,
    );
}

fn max_label_width(palette: &ResolvedPalette) -> usize {
    let mut max_len = 0;

    for key in palette.colors.light.keys() {
        max_len = max_len.max(key.len());
    }
    for key in palette.colors.dark.keys() {
        max_len = max_len.max(key.len());
    }
    for key in palette.accents.keys() {
        max_len = max_len.max(key.len());
    }
    for key in [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ] {
        max_len = max_len.max(key.len());
    }

    max_len.max(8)
}

fn print_section(title: &str, items: Vec<(String, String)>, label_width: usize) {
    if items.is_empty() {
        return;
    }

    println!("{title}");
    println!(
        "{:<width$}  {:<6}  {}",
        "key",
        "swatch",
        "hex",
        width = label_width
    );
    println!(
        "{:-<width$}  {:-<6}  {}",
        "",
        "",
        "----",
        width = label_width
    );

    for (label, hex) in items {
        print!("{:<width$}  ", label, width = label_width);
        let sw = swatch(&hex);
        print!("{sw}");
        println!("  {hex}");
    }
    println!();
}

fn ansi_row_items(row: &ResolvedAnsiRow) -> Vec<(String, String)> {
    vec![
        ("black".to_string(), row.black.clone()),
        ("red".to_string(), row.red.clone()),
        ("green".to_string(), row.green.clone()),
        ("yellow".to_string(), row.yellow.clone()),
        ("blue".to_string(), row.blue.clone()),
        ("magenta".to_string(), row.magenta.clone()),
        ("cyan".to_string(), row.cyan.clone()),
        ("white".to_string(), row.white.clone()),
    ]
}

fn swatch(hex: &str) -> String {
    if let Some((r, g, b)) = hex_to_rgb(hex) {
        let luminance = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0;
        let text = if luminance < 0.5 { 255 } else { 0 };
        return format!("\u{1b}[48;2;{r};{g};{b}m\u{1b}[38;2;{text};{text};{text}m      \u{1b}[0m");
    }
    hex.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_to_rgb() {
        assert_eq!(hex_to_rgb("#A1B2C3"), Some((0xA1, 0xB2, 0xC3)));
        assert_eq!(hex_to_rgb("#000000"), Some((0, 0, 0)));
        assert_eq!(hex_to_rgb("123456"), None);
        assert_eq!(hex_to_rgb("#ffff"), None);
    }
}
