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
- `veneer build <src> [dest] [--palette veneer.toml]`  
  Render one or many templates. `src` can be a single file, a directory (all `*.tera` inside, recursively), or a glob such as `src/*.tera`.  
  - Single file: `dest` may be a file or directory (default: current directory, stripping `.tera`).  
  - Directory or glob: `dest` may be a directory or a filename prefix. If it points to an existing directory (or ends with `/`), files render into that directory with relative paths preserved and `.tera` removed. Otherwise `dest` is treated as a prefix and the matched path (minus `.tera`) is appended.
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

### Helpers
- `with_alpha(color, alpha)` → hex with alpha channel.  
  Example: `{{ with_alpha(color=dark.primary, alpha=0.2) }}` → `#11223333`
- `rgba(color, alpha)` → CSS `rgba(r, g, b, a)` string.  
  Example: `{{ rgba(color=light.background, alpha=0.85) }}` → `rgba(255, 255, 255, 0.850)`
- `hsla(color, alpha)` → CSS `hsla(h, s, l, a)` string.  
  Example: `{{ hsla(color=accents.info, alpha=0.6) }}` → `hsla(201.600, 0.650, 0.500, 0.600)`
- `rgba_floats(color, alpha)` → space-separated floats in 0–1 range.  
  Example: `{{ rgba_floats(color=dark.text, alpha=0.75) }}` → `0.902353 0.929413 0.952941 0.750000`
- `lowercase` filter → lowercases a string.  
  Example: `{{ accents.info | lowercase }}` → `#3fa7d6`

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

More examples:
```bash
# Render every template under src/ into dist/ (directories created as needed)
veneer build src dist/ --palette veneer.toml

# Glob render with a filename prefix
veneer build "templates/*.tera" dist/theme- --palette veneer.toml
```

## Development
- `cargo test` to run unit tests.
- `cargo run -- show --palette veneer.toml` to preview a palette.

## License
MIT, see `LICENSE`.
