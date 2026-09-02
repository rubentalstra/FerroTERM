# Notio brand

Notio is a working codename (Latin for "concept"). These assets travel with the
codename until the official name is set. All of it is MIT-licensed with the rest
of the repository.

## The mark

The mark is a concept graph: one concept at the centre with its related concepts
around it, joined by edges. It reads the same for any code system the server
handles, SNOMED CT, LOINC, or another, because every terminology is concepts and
the relationships between them. The top node is the cyan accent; the centre node
is the largest.

## Palette

| Token | Hex | Use |
|---|---|---|
| teal | `#0d9488` | primary mark, edges, accents |
| teal-deep | `#0f766e` | nodes, hover, pressed |
| cyan | `#22d3ee` | the accent node, highlights |
| ink | `#0f172a` | text on light |
| mist | `#f1f5f9` | text on dark |
| tile | `#0b1220` | dark tile background |

The values live in `tokens.css` as custom properties. On a dark tile the mark
brightens to teal `#14b8a6` edges and `#2dd4bf` nodes so it holds contrast.

## Files

| File | What it is |
|---|---|
| `notio-icon.svg` | primary icon, full colour, transparent background |
| `notio-icon-mono.svg` | one-colour icon, inherits `currentColor` |
| `notio-icon-dark.svg` | the icon on a dark rounded tile |
| `notio-lockup-light.svg` | icon and "Notio" wordmark for light backgrounds |
| `notio-lockup-dark.svg` | the lockup for dark backgrounds |
| `notio-lockup-auto.svg` | the lockup that follows `prefers-color-scheme` |
| `favicon.svg` | the mark on a teal tile, for browser tabs |
| `favicon-32.png`, `favicon-16.png`, `favicon.ico` | raster favicons |
| `notio-social.svg`, `notio-social.png` | 1200x630 social card |
| `tokens.css` | the palette as CSS custom properties |

The wordmark is set in Bricolage Grotesque (700). Body copy pairs with IBM Plex
Sans, and data and labels with IBM Plex Mono. The lockup SVGs reference these
with a system-sans fallback; outline the wordmark to paths before any print use.

## Usage

- Keep clear space around the mark equal to the diameter of one node.
- The smallest the mark reads is 16 px; below that use the favicon.
- Put the colour mark on light or quiet surfaces, and `notio-icon-dark.svg` (the
  tile) on busy or light-photographic backgrounds.
- Use `notio-icon-mono.svg` where one colour is required; it takes the
  surrounding text colour.
- Do not recolour the mark outside the palette, stretch it, add effects, or
  rebuild the wordmark in another typeface.

## Regenerating the rasters

The PNG and ICO files derive from the SVGs:

```bash
rsvg-convert -w 32 -h 32 favicon.svg -o favicon-32.png
rsvg-convert -w 16 -h 16 favicon.svg -o favicon-16.png
magick favicon-32.png favicon-16.png favicon.ico
rsvg-convert -w 1200 -h 630 notio-social.svg -o notio-social.png
```
