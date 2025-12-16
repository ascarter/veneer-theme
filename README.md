# veneer-theme

Minimal Tera-based theme generator for palettes described in TOML.

## What it does
- Loads a palette file (`veneer.toml` by default) with light/dark colors, accents, and ANSI swatches.
- Resolves references inside the palette (paths like `colors.light.primary`) and validates hex formats.
- Renders a Tera template using the resolved palette into any output path.
- Gives a terminal preview of your palette with color swatches.

## Install
```bash
cargo install --path .
```
Or run locally without installing:
```bash
cargo run -- <command>
```

## CLI
- `veneer build <template.tera> [dest] [--palette veneer.toml]`  
  Renders the template and writes to `dest` (file or directory). Defaults to the current directory, stripping `.tera` from the filename.
- `veneer check --palette veneer.toml <template.tera>`  
  Validates palette + template rendering without writing files.
- `veneer show --palette veneer.toml`  
  Prints palette details with colored swatches in the terminal.

## Palette file (`veneer.toml`)
Colors can be hex (`#RRGGBB`) or references to other entries (`colors.light.primary`). Cycles and bad hex codes are rejected.

```toml
[meta]
name = "Veneer Demo"
version = "0.1.0"

[colors.light]
background = "#FFFFFF"
text = "#111111"
primary = "#2E73FF"

[colors.dark]
background = "#0E1117"
text = "#E6EDF3"
primary = "colors.light.primary"  # reference to another key

[accents]
info = "#3FA7D6"
warning = "#E6A700"

[ansi.light.normal]
black   = "colors.light.background"
red     = "#CC241D"
green   = "#98971A"
yellow  = "#D79921"
blue    = "#458588"
magenta = "#B16286"
cyan    = "#689D6A"
white   = "colors.light.text"

[ansi.light.bright]
black   = "#282828"
red     = "#FB4934"
green   = "#B8BB26"
yellow  = "#FABD2F"
blue    = "#83A598"
magenta = "#D3869B"
cyan    = "#8EC07C"
white   = "#FBF1C7"

[ansi.dark.normal]
black   = "colors.dark.background"
red     = "#CC241D"
green   = "#98971A"
yellow  = "#D79921"
blue    = "#458588"
magenta = "#B16286"
cyan    = "#689D6A"
white   = "colors.dark.text"

[ansi.dark.bright]
black   = "#3C3836"
red     = "#FB4934"
green   = "#B8BB26"
yellow  = "#FABD2F"
blue    = "#83A598"
magenta = "#D3869B"
cyan    = "#8EC07C"
white   = "#EBDBB2"
```

## Template context
When rendering, the Tera context exposes:
- `meta` (name, version)
- `light` and `dark` (maps of key -> hex)
- `accents` (map)
- `ansi.light.normal`, `ansi.light.bright`, `ansi.dark.normal`, `ansi.dark.bright`

Helpers registered in Tera:
- Functions: `with_alpha(color, alpha)`, `rgba(color, alpha)`, `hsla(color, alpha)`, `rgba_floats(color, alpha)`
- Filter: `lowercase`

Example snippet (`theme.json.tera`):
```tera
{
  "name": "{{ meta.name }}",
  "type": "dark",
  "colors": {
    "editor.background": "{{ dark.background }}",
    "editor.foreground": "{{ dark.text }}",
    "editor.selectionBackground": "{{ with_alpha(color=dark.primary, alpha=0.25) }}"
  },
  "accent": "{{ accents.info | lowercase }}"
}
```

Render it:
```bash
veneer build theme.json.tera dist/theme.json --palette veneer.toml
```

## Development
- `cargo test` to run unit tests.
- `cargo run -- show --palette veneer.toml` to preview a palette.

## License
MIT, see `LICENSE`.
