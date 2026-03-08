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
- `blend(color, alpha, background)` → opaque `#RRGGBB` hex with alpha pre-composited over `background`. Useful for themes (e.g. Ghostty) that don't support alpha channels.
  Example: `{{ blend(color=dark.primary, alpha=0.15, background=dark.background) }}` → `#161C27`
- `mix(a, b, t)` → linear interpolation (lerp) between two hex colors, returning an opaque `#RRGGBB` hex. Operates directly on sRGB channel values. `t=0.0` returns `a`, `t=1.0` returns `b`, `t=0.5` returns the midpoint. Useful for deriving intermediate steps in a color ramp from palette anchor points.
  Example: `{{ mix(a=dark.secondary, b=dark.tertiary, t=0.5) }}` → `#353535`
- `ron_color(color[, alpha])` → RON inline struct with `red`, `green`, `blue`, `alpha` float fields (0.0–1.0). `alpha` defaults to `1.0`. Use for `Srgba` fields in Cosmic desktop theme files (palette color slots, `bg_color`, container backgrounds).
  Example: `{{ ron_color(color=accents.info) }}` →
  ```ron
  (
          red: 0.2470588,
          green: 0.6549020,
          blue: 0.8392157,
          alpha: 1.0000000,
      )
  ```
- `ron_rgb(color)` → RON inline struct with `red`, `green`, `blue` float fields only (0.0–1.0), no alpha. Use for `Srgb` fields in Cosmic desktop theme files (`neutral_tint`, `text_tint`, `accent`, `success`, `warning`, `destructive`).
  Example: `{{ ron_rgb(color=accents.info) }}` →
  ```ron
  (
          red: 0.2470588,
          green: 0.6549020,
          blue: 0.8392157,
      )
  ```
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

Example Ghostty template snippet (`ghostty.tera`):
```tera
# Selection uses a pre-composited color since Ghostty doesn't support alpha
selection-background = {{ blend(color=dark.primary, alpha=0.3, background=dark.background) }}
```

Example Cosmic desktop theme snippet (`Dark.ron.tera`):
```tera
{{/* Palette color slots are Srgba — use ron_color */}}
accent_blue: {{ ron_color(color=accents.info) }},
bg_color: Some({{ ron_color(color=dark.background) }}),
{{/* Semantic override fields are Srgb (no alpha) — use ron_rgb */}}
neutral_tint: Some({{ ron_rgb(color=dark.neutral) }}),
accent: Some({{ ron_rgb(color=accents.info) }}),
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

## Example themes

- [ghostty-alpental-theme](https://github.com/ascarter/ghostty-alpental-theme)
- [vscode-alpental-theme](https://github.com/ascarter/vscode-alpental-theme)
- [xcode-alpental-theme](https://github.com/ascarter/xcode-alpental-theme)
- [zed-alpental-theme](https://github.com/ascarter/zed-alpental-theme)

## License
MIT, see `LICENSE`.
