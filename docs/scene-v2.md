# Scene JSON v2 — Design Doc

Status: draft for review · 2026-09-27 · Yuval Tal
Inputs: [Scene JSON + MCP — Design Spec](https://claude.ai/artifact/JA2qcvuxn3GgL1aTDkjMEk) (build order step 2), [graphic-layer.md](graphic-layer.md) (Skia mapping and render order, still the implementation reference), the competitive analysis (kept local, not committed), and four studies done for this doc: a survey of 60 real templates, a Figma and SwiftUI inventory, a Penpot inventory, and the competitor landscape. Sources are at the end.

## Summary

v2 is a superset of the MVP. Every MVP scene stays valid: MVP fields become the short forms of richer v2 fields. v2 adds five things:

1. **Layout that adapts by itself.** Sizes `hug`, `fill` or `%`, min/max, wrap, grid, `place: "bottom-right"`, and SwiftUI's first-that-fits. Most per-size `at` fixes go away.
2. **One paint model on every visible layer.** Fill, stroke and shadow arrays; radial and conic gradients; image fills on any shape; blur, backdrop blur, noise, image adjustments; masks by shape, layer or image.
3. **Rich text without character counting.** Inline markup (`Proven <accent>RESULTS</accent>`), highlights behind words, balanced wrapping, vertical alignment, decoration.
4. **Reuse.** Tokens (`$brand`), styles on any layer, and components repeated from data (`each`).
5. **More shapes.** Paths, including a built-in library of named shapes (ribbons, bubbles, blobs); polygons and stars (seals); arcs (rings, gauges); per-corner and capsule radii; dashes and arrowheads.

The tool surface stays at six tools. Checked against the 60-template survey, v2 renders 59 of the 60 with live, editable layers; the last one needs warped text, planned for later (see Coverage). `render` also gains JPEG, WebP and PDF output with a file-size target.

## Goals and rules

Goals:

- Render everything common in real marketing templates (see Coverage) with live, editable layers.
- Adapt one master to every size without per-size edits wherever the design allows.
- Keep the agent's cost at the MVP's level or lower: ~2k tokens of tool traffic per design, 2–4 calls.

Non-goals for v2: animation (the schema is ready for it, see Animation readiness), boolean path operations, vector networks, print color (CMYK), SVG output, templates with slots (the next milestone, built on components).

Rules every field follows:

1. **Names models guess.** CSS words where CSS has the concept (`gap`, `padding`, `justify`, `minWidth`, `aspectRatio`, `letterSpacing`, `textWrap`, `blur`, `backdropBlur`), Figma words for sizing and pinning (`hug`, `fill`, `constraints`), SwiftUI words only where CSS has none (`firstFit`, `priority`, `minFontScale`). Where two vocabularies disagree, input accepts both and output uses one: `fit: fill|fit` also reads `cover|contain`; `textCase` also reads `textTransform`.
2. **Short form and full form.** Each compound field has a one-token short form: `fills: "#fff"`, `padding: 24`, `radius: 12`, `stroke: "#000"`. The server stores and reports the shortest equivalent form.
3. **Every field has a default and defaults are omitted,** in inputs and outputs.
4. **Arrays for effects, never `…Enabled` flags.** Absent means off.
5. **Earn the place.** A field is added only if it lets the agent make a design it can't today, removes per-size manual fixes, or cuts tokens, and a benchmark confirms it (3+ runs, see Build order).
6. **Deterministic.** Same scene, same PNG on a given OS version. Procedural effects (noise, torn edges, rough strokes) take a `seed`.

## Changes at a glance

| Area | MVP | v2 |
|---|---|---|
| Sizing | px; an unsized stack frame hugs | `width`/`height`: px, `"hug"`, `"fill"`, `"50%"`; `grow`; `minWidth`…`maxHeight`; `aspectRatio` |
| Stacks | `dir`, `gap`, `padding`, `align`, `justify` | + `wrap`, `gap: [row, col]`, padding shorthand, `align: stretch|baseline`, `justify: around`, child `alignSelf`, `priority`, `position: "absolute"`, `spacer` |
| Adaptive layout | per-size `at` | + `firstFit`, `dir: ["row", "column"]`, grid with `areas`, `at` by aspect class, `place` |
| Paint | `color` or linear `gradient`, one `stroke`, text `shadow` | `fills[]`, `strokes[]`, `shadows[]` on any layer; radial, conic gradients; image and pattern fills; `noise`; `blur`, `backdropBlur` |
| Masks | linear gradient | + shape, path, layer, image masks; `clip` on any frame |
| Shapes | rect, ellipse, line, icon | + `path` (own `d` or a named `shape`), `polygon` (stars too), ellipse `arc`, per-corner and `"full"` radius, `smoothing`, dashes, caps, arrowheads |
| Text | color `ranges` by offset | inline markup; any style in a range; `highlight`; `textWrap`; `verticalAlign`; `justify`; `decoration`; `italic`; `trim`; `curve`; `leader`; `knockout` |
| Output | PNG | + JPEG, WebP, PDF; `quality`; `maxKB` file-size target |
| Transforms | `rotation` | + `scale`, `offset`, `skew`, `flipX`, `flipY` (all after layout) |
| Reuse | text `styles` | `tokens`; styles on any layer, several per layer; `components` with `use` and `each` |
| Sizes | `{id, width, height, scale}` | + presets by slug (`"instagram-story"`, `"1200x628"`), `safe` insets |

## Document model

```json
{
  "schemaVersion": 0,
  "width": 1080, "height": 1350,
  "sizes": ["instagram-portrait", {"id": "wide", "width": 1200, "height": 1000, "scale": 0.85}, "300x600"],
  "background": "#FFFFFF",
  "tokens": {"navy": "#1B2A5C", "red": "#D0202E", "pad": 40},
  "styles": {"h1": {"fontSize": 64, "weight": 800, "color": "$navy"}, "accent": {"color": "$red"}},
  "components": {"step": {"type": "frame", "stack": {"dir": "row", "gap": 16}, "children": ["…"]}},
  "assets": {"photo": {"width": 1600, "height": 900}},
  "layers": ["…"]
}
```

| Field | Meaning | Default |
|---|---|---|
| `schemaVersion` | Format version. Stays at the MVP's value (0): v2 only adds fields and the MVP has no outside users, so there is nothing to tell apart. The first breaking change after release bumps it | 0 |
| `width`, `height` | Master size, px; layers are authored at this size | required |
| `sizes` | Output sizes: `{id, width, height, scale, safe}`, a preset slug, or `"WxH"` (id = the string) | required |
| `background` | Any paint (color, gradient, image) | `#FFFFFF` |
| `tokens` | Named values used as `"$name"` in any field | `{}` |
| `styles` | Named partial layers, applied by `style` | `{}` |
| `components` | Named layer trees, placed by `use` layers | `{}` |
| `assets` | Content-addressed images and SVGs, added by `asset_add` | `{}` |
| `layers` | The layer tree, bottom to top | `[]` |

`safe` on a size (px, or `[t, r, b, l]`) marks an inset the platform covers (a story's UI bars). `scene_describe` reports text or buttons inside it as `!unsafe`.

Size presets (initial list, extended by request): `instagram-portrait` 1080×1350, `instagram-square` 1080×1080, `instagram-story` 1080×1920 (safe 250 top, 340 bottom), `facebook-feed` 1200×628, `linkedin-post` 1200×627, `x-post` 1600×900, `youtube-thumbnail` 1280×720, `iab-medium-rectangle` 300×250, `iab-leaderboard` 728×90, `iab-skyscraper` 160×600, `iab-half-page` 300×600, `a4-portrait` 2480×3508 (300 dpi).

### Layer types

v2 keeps a type per thing an agent names, instead of the single `graphic` type proposed in [graphic-layer.md](graphic-layer.md). The MVP benchmarks showed that naming what a thing is (icons, styles) keeps runs cheap. Every visible type shares the same paint fields, so Figma's composability survives: swapping a photo or gradient is still one field.

| `type` | What it is | Type fields (defaults) |
|---|---|---|
| `frame` | Container; free, stack or grid layout | `children`, `clip` (true), `stack`, `grid`, `radius` |
| `text` | Text in a box | see Text |
| `image` | A rect whose bottom fill is an image | `asset`, `fit`, `focus`, `crop`, `tileScale`, `adjust`, `radius` |
| `rect` | Rectangle | `radius` (0), `smoothing` (0) |
| `ellipse` | Ellipse in the box; arcs and rings | `arc {start, end, inner}` (0, 360, 0) |
| `polygon` | Regular polygon in the box; with `innerRadius`, a star | `sides` (3), `innerRadius` (none: alternate points move in to this share of the outer radius, e.g. 0.38 for a classic star, 0.8 for a starburst), `radius` (0: rounded points make a scalloped seal) |
| `line` | From the box's top-left by `width, height` | `cap`, `start`/`end` markers |
| `path` | SVG path: your own `d`, or a built-in `shape` by name | `d` or `shape` (one required), `fillRule` (`nonzero`), `fitPath` `stretch`\|`contain` (`contain`) |
| `icon` | Named Lucide or Font Awesome icon | `name`, `set`, `color`, `strokeWidth` (unchanged) |
| `firstFit` | Draws the first child that fits (SwiftUI `ViewThatFits`) | `children` |
| `use` | An instance of a component | `component`, props, `each` |
| `spacer` | Flexible empty space in a stack | `minLength` (0) |

### Named shapes

`path` takes either your own SVG `d` or `shape`, a name from a built-in library of SVG paths, the way `icon` names icons. Shapes that aren't parametric are cheaper to name than to draw: agents write a name, not coordinates. Initial set: `ribbon`, `ribbon-banner` (folded ends), `bubble` (speech), `bubble-round`, `arrow`, `arrow-curved`, `chevron`, `tag` (price tag), `arch`, `shield`, `heart`, `cloud`, `wave`, `burst` (irregular), `blob-1` … `blob-6`, `brush-stroke`. A shape is a real path, so every paint applies: a photo in a blob, a gradient ribbon, a dashed speech bubble. The paths are drawn for keyline or taken from a permissive (CC0, MIT, ISC) source, with the license next to them as for icons.

### Common fields

| Field | Meaning | Default |
|---|---|---|
| `id` | Stable id; generated when omitted | generated |
| `role` | Semantic name; ops can target every layer with a role | none |
| `x`, `y` | Position in the parent, px or `"%"`; ignored inside a stack or grid | 0 |
| `width`, `height` | px, `"%"` of the parent, `"hug"` (fit content), `"fill"` (take the free space) | type-specific |
| `minWidth`, `maxWidth`, `minHeight`, `maxHeight` | Clamps, px | none |
| `aspectRatio` | Keeps width/height; the free side follows | none |
| `constraints` | `{h: left|right|center|stretch|scale, v: top|bottom|center|stretch|scale}` for free layout | left, top |
| `place` | Pins to one of 9 spots of the parent: `"top-left"` … `"center"` … `"bottom-right"`, with `inset` (px or `[x, y]`); sets position and constraints together | none |
| `hidden` | Not drawn (useful per size via `at`) | false |
| `opacity` | 0–1 | 1 |
| `blendMode` | The 16 CSS blend modes | `normal` |
| `fills`, `strokes`, `shadows` | Paint arrays; see Paint | type-specific |
| `blur`, `backdropBlur` | Layer blur, background blur, px | 0 |
| `mask` | See Masks | none |
| `rotation`, `scale`, `offset`, `skew`, `flipX`, `flipY` | Visual transforms after layout, about the box center; they never move siblings | 0, 1, [0,0], [0,0], false, false |
| `allowOverlap` | This layer may overlap text on purpose (a cutout over a headline, a script word over caps); silences `!overlaps` for it | false |
| `style` | A style name, or several applied in order; the layer's own fields win | none |
| `at` | Changes per size id or aspect class | `{}` |

## Layout

A frame lays out its children in one of three ways: **free** (each child's `x`, `y`, `constraints` or `place`), **stack** (`stack`: a row or column, like CSS flexbox and Figma auto layout), or **grid** (`grid`: CSS grid). Children of a stack or grid ignore `x`, `y` and `constraints` unless `position: "absolute"`.

### Sizing

| Value | Meaning | Figma | SwiftUI | CSS |
|---|---|---|---|---|
| `320` | Fixed px (scaled by the size's `scale`) | Fixed | `.frame(width:)` | `width: 320px` |
| `"hug"` | As big as the content | Hug | ideal size | `fit-content` |
| `"fill"` | Takes the free space in a stack or grid; the whole parent in free layout | Fill | `maxWidth: .infinity` | `flex: 1` / `100%` |
| `"40%"` | Share of the parent's inner size | — | `containerRelativeFrame` | `40%` |

Defaults: text and frames with a stack `hug`; images, shapes and frames without a stack keep the MVP behaviour (intrinsic size or required). `grow` (1) weights how `fill` children split the space. Clamps apply after.

### Stacks

```json
"stack": {"dir": "row", "gap": 16, "padding": [24, 32], "align": "center", "justify": "between", "wrap": true}
```

| Field | Values | Default |
|---|---|---|
| `dir` | `row`, `column`, `row-reverse`, `column-reverse`, or a list tried in order (`["row", "column"]`: a row if it fits, else a column) | required |
| `gap` | px, `[rowGap, columnGap]` when wrapping | 0 |
| `padding` | px, `[vertical, horizontal]`, or `[top, right, bottom, left]` | 0 |
| `align` | Cross axis: `start`, `center`, `end`, `stretch`, `baseline` | `start` |
| `justify` | Main axis: `start`, `center`, `end`, `between`, `around`, `evenly` | `start` |
| `wrap` | Wrap into lines (chips, logos, tags) | false |

Children in a stack may set `alignSelf`, `grow`, `priority` and `position: "absolute"` (taken out of the flow and placed like a free child, e.g. a badge over a card's corner). `priority` (0) is SwiftUI's `layoutPriority`: when a row is too narrow, lower-priority children shrink first (a subtitle gives way before the price). A `spacer` layer takes leftover space.

### Grid

```json
"grid": {"columns": "1fr 1fr 1fr", "rows": "auto 1fr", "gap": 16, "areas": ["hero hero side", "cta cta side"]}
```

`columns` and `rows` use CSS track syntax (`px`, `fr`, `auto`, `%`, `repeat(3, 1fr)`); `columns: {"min": 160}` fits as many columns as the width allows (CSS `auto-fit, minmax`; SwiftUI adaptive grid). Children take `area: "hero"` or `cell: [row, column]` with `span: [rows, columns]`, else auto-flow in order. A size can re-arrange the whole grid by overriding only `columns` and `areas` in `at`: a banner and a square become the same children with two short templates.

### Layouts that pick what fits

`firstFit` draws the first child whose content fits its box at this size, with no defect inside it (no `!overflow`, `!truncated`, text shrunk below its `minFontScale`, or child clamped below its minimum). Typical uses: a long and a short headline, a row CTA and a stacked CTA. `stack.dir` as a list is the common case written in one field. `scene_describe` reports the choice per size (`candidates: column (row too narrow)`), so the agent sees it without rendering.

### Per size

`at` keys are a size id or an aspect class, applied broadest first: `landscape` (w/h > 1.1), `square` (0.9–1.1), `portrait` (< 0.9), then `wide` (2:1 or wider) and `tall` (1:2 or taller), then the size id. A 4:5 post is portrait; a 300 × 600 half-page is tall. One `"tall": {…}` entry covers every skyscraper a template is ever rendered at, including sizes added later.

## Paint

Every visible layer (frames, shapes, images, text, icons) takes the same fields. `color` and `gradient` stay as the short forms of a single fill; `stroke` and `shadow` of a single stroke and shadow.

### Fills

`fills` is one paint or an array, bottom to top. A paint is:

| Form | Example |
|---|---|
| Color | `"#D0202E"`, `"#D0202E80"`, or `{"color": "$red", "opacity": 0.5}` |
| Gradient | `{"gradient": {"type": "radial", "stops": ["#0000", "#000C"]}}` |
| Image | `{"image": "photo", "fit": "fill", "focus": [0.5, 0.3], "adjust": {"grayscale": 1}}` |
| Pattern | `{"pattern": "dots", "color": "#0002", "size": 12}`; `dots`, `stripes`, `grid`, `checker`, `zigzag`, `rays`; `angle` |
| Noise | `{"noise": 0.08, "seed": 1}` grain, monochrome unless `color` |

Every paint takes `opacity` (1) and `blendMode` (`normal`). An image fill works on any shape, so a photo in a circle, blob or star is `{"type": "ellipse", "fills": {"image": "a1"}}`.

Gradients: `type` `linear` (default), `radial`, `conic`. Linear takes the MVP's `from`/`to` (0–1 of the box) or a CSS `angle` in degrees (0 = up, 90 = right). Radial takes `center` ([0.5, 0.5]) and `radius` ([0.5, 0.5] of width and height, so it stretches to the box like Figma's). Conic takes `center` and `angle`. `stops` is a list of colors (evenly spaced) or `{at, color}`.

Image adjustments (`adjust`, 0 = unchanged, CSS filter names): `brightness`, `contrast`, `saturate` (−1…1), `grayscale`, `sepia` (0…1), `hue` (degrees), `duotone: [dark, light]`, `tint` (recolor an SVG or logo, e.g. white), `halftone` (dot spacing, px: the image redrawn as dots sized by brightness; a small SkSL shader).

### Strokes

`strokes` is one stroke or an array; `stroke: "#000"` is a 1 px stroke. Fields: `color` or `gradient`, `width` (1; `[t, r, b, l]` on rects for per-side borders), `align` `inside`|`center`|`outside` (inside for closed shapes, center for open ones), `dash` ([length, gap], px), `cap` `butt`|`round`|`square`, `join` `miter`|`round`|`bevel`. Lines and paths also take `start`/`end` markers: `arrow`, `triangle`, `circle`, `diamond`. `rough` (px, with `seed`) jitters the stroke's path so any line, circle or underline looks hand-drawn (Skia `PathEffect::discrete`).

### Shadows and effects

`shadows` is one shadow or an array, CSS `box-shadow` semantics: `{x, y, blur, spread, color, inset}`. `inset: true` is an inner shadow. A shadow follows the layer's alpha, so it hugs a cutout photo or the letters of a text. Glow is a shadow at `x: 0, y: 0`; a hard offset shadow is `blur: 0`.

`blur` blurs the layer; `backdropBlur` blurs what's behind it within its shape (frosted-glass panels). Render order follows [graphic-layer.md](graphic-layer.md): backdrop blur, drop shadows, fills, inner shadows, strokes, layer blur, mask.

### Masks

| `mask` | Effect |
|---|---|
| `{"from": …, "stops": …}` | MVP gradient mask: its alpha fades the layer |
| `"ellipse"`, a named shape (`"blob-3"`), `{"path": "M…"}` | Clip to that shape in the box |
| `{"layer": "logo"}` | Use another layer's alpha (the mask layer isn't drawn) |
| `{"image": "torn-edge"}` | Use an image's alpha (torn paper, brush strokes) |

Add `"mode": "luminance"` to use brightness instead of alpha, `"invert": true` to reverse it. Frames clip their children by default (`clip`).

### Corners

`radius`: px, `[tl, tr, br, bl]`, or `"full"` (a capsule: pills stay pills at every size). `smoothing` 0–1 draws continuous corners (Figma corner smoothing, SwiftUI `.continuous`); 0.6 matches iOS.

### Edges

`edges: {"style": "torn", "sides": ["top", "bottom"], "depth": 12, "seed": 1}` roughens the chosen sides of any shape, image or frame (torn paper, ragged label strips, panel splits). `sides` defaults to all; `depth` is px. It's Skia's `PathEffect::discrete` on the box outline, so it costs a few lines and stays deterministic through `seed`.

## Text

A text layer keeps every MVP field. New and changed fields:

| Field | Meaning | Default |
|---|---|---|
| `text` | Text with optional inline markup; `\n` breaks lines | required |
| `italic` | Italic or oblique face | false |
| `align` | + `justify` | `left` |
| `verticalAlign` | `top`, `center`, `bottom` inside a fixed box | `center` for fixed boxes (as the MVP), `top` otherwise |
| `textWrap` | `balance` (even line lengths), `pretty` (no orphan word), `wrap` | `wrap` |
| `textCase` | + `capitalize` | none |
| `decoration` | `underline`, `strike` | none |
| `paragraphSpacing` | Extra space between paragraphs, px | 0 |
| `trim` | `"cap"` trims the space above cap height and below the baseline, so text centres optically in pills and buttons | none |
| `highlight` | A box behind each line: `{color, padding, radius, style: box|brush}` | none |
| `curve` | Sets the text along a circular arc: `{"radius": 300}` bends it upward, a negative radius downward (seals, badges); one line only | none |
| `leader` | A character that fills the gap at each tab (`\t`): `"Espresso\t$3"` with `leader: "."` draws a menu's dot leaders, the price flush right | none |
| `knockout` | The letters cut through their parent frame's fill, showing what's behind (the photo-through-text look) | false |
| `direction` | `auto`, `ltr`, `rtl` | `auto` |
| `features` | OpenType features, e.g. `{"tnum": 1}` | `{}` |
| `fills`, `strokes`, `shadows` | Paint the letters (color, gradient, image), outline them (`fills: []` + a stroke makes outlined text), shadow or glow them | black fill |

MVP text `fill` (image), `gradient`, `outline` and `shadow` become short forms of `fills`, `strokes` and `shadows`.

### Inline markup

Character ranges by offset are error-prone for models, which miscount characters. v2 reads a small HTML subset inside `text`:

```json
{"type": "text", "style": "h1", "text": "Proven <accent>RESULTS</accent> for <accent>WILLOWMERE</accent> Families"}
{"type": "text", "text": "<s>$49</s> <b>$29</b><sup>99</sup> today"}
```

- Tags: `<b>`, `<i>`, `<u>`, `<s>`, `<sup>`, `<sub>`, `<br>`, a style name as a tag (`<accent>`), and `<span …>` with any text field as an attribute (`<span color="#D0202E" weight="800" highlight="#FFE600">`).
- A `<` that doesn't open a known tag is literal text; `&lt;` forces it.
- `ranges` stays as the full form and now takes any text field, not just `color`. The server stores markup as the agent wrote it.

`highlight` inside a span marks a phrase (the marker-pen look in 10 of 60 templates); on the layer it boxes every line.

### Fitting

Resize modes, `maxLines`, `minFontScale` and `ellipsis` are unchanged; the MVP's measured fitting already goes beyond the competition. A fit that reaches its limit still reports `!overflow` instead of shrinking silently.

## Reuse

### Tokens

`tokens` holds named values; any field takes `"$name"`. Names may contain dots (`$color.brand`). A token may refer to another (`"accent": "$red"`). `at` at the scene level can override tokens per size or aspect class (`"tall": {"tokens": {"pad": 16}}`), Figma's variable modes and Penpot's themes in one mechanism.

### Styles

`styles` now hold any layer fields, not just text (a `card` style with fills, radius and shadows). `style` takes one name or a list applied in order. Updating a style by `{style}` target changes every layer that uses it, as in the MVP.

### Components

```json
"components": {
  "candidate": {"type": "frame", "stack": {"dir": "column", "gap": 8, "align": "center"}, "children": [
    {"type": "image", "role": "photo", "asset": "{photo}", "width": 160, "height": 160, "radius": "full"},
    {"type": "text", "role": "name", "text": "{name}", "style": "name"},
    {"type": "text", "role": "office", "text": "{office}", "style": "office"}]}
}
```

```json
{"type": "use", "id": "cands", "component": "candidate", "each": [
  {"name": "Dana Levi", "office": "Mayor", "photo": "dana"},
  {"name": "Omar Haddad", "office": "Council", "photo": "omar"},
  {"name": "Ruth Cohen", "office": "Council", "photo": "ruth"}]}
```

- `{prop}` in any string of a component is filled from the instance's props. `each` places one instance per entry, in the parent's flow.
- Instances stay linked: editing the component changes every instance. An instance differs only by its props and `at`; `layer_update` with `detach: true` turns it into plain layers.
- Inner layers are addressed as `cands.1.name` (instance 1's `name` role), or by role across all instances.
- Templates (next milestone) build on this: a template is a scene whose slots are props.

## Coverage of real templates

Checked against 60 templates from PosterMyWall, Kittl, Venngage and Creatopy (Canva, Figma Community and Freepik blocked automated access). Count = templates using the technique.

| Technique | # | v2 | Priority |
|---|---|---|---|
| Full-bleed photo with text | 21 | image `fit`/`focus`, gradient scrim fill, `warn contrast` | MVP |
| Illustrations (vector, watercolor, clipart) | 18 | image layers, SVG assets, `adjust.tint` | MVP |
| Icons | 17 | `icon` | MVP |
| Letter-spaced caps | 15 | `letterSpacing`, `textCase` | MVP |
| Mixed colors, weights, sizes in one line | 14 | inline markup, `<sup>` | P1 |
| Photo cutout over shapes or text | 13 | PNG with alpha, `allowOverlap`, shadows follow alpha; background removal tool (spec build order step 3) | P1 + step 3 |
| Rules, dividers, separators | 13 | `line`, `dash` | MVP / P1 |
| Script fonts, often overlapping | 12 | Google Fonts, `allowOverlap` | MVP / P1 |
| Textures (grain, paper, grunge) | 10 | `noise` fill; texture image with `blendMode` on top | P1 / P2 |
| Box or brush behind words | 10 | `highlight` (`box`, `brush`) | P1 |
| Rotated or skewed text and groups | 9 | `rotation`, `skew` on any layer, frames included | P1 |
| Outline frames, brackets | 9 | strokes; per-side `width`; `dash` | P1 |
| Gradients | 9 | linear, radial, conic | P1 |
| CTA pills | 8 | frame `hug` + `radius: "full"` + `trim: "cap"` | P1 |
| Logos | 8 | image, `tint` | MVP / P1 |
| Patterns (stripes, zigzag, grid, hexes) | 7 | `pattern` fill; SVG tile via image `fit: tile` | P2 / MVP |
| Seals, starbursts, scalloped badges | 6 | `polygon` with `innerRadius` (+ `radius`), text centered by stack | P1 |
| Geometric confetti, rings | 6 | `ellipse` `arc.inner`, `polygon`, strokes | P1 |
| Drop or hard offset text shadow | 6 | `shadows` (several, `blur: 0`) | P1 |
| Border and ornamental frames | 6 | strokes; ornaments as SVG | P1 |
| Torn or ragged edges | 5 | `edges: {style: "torn"}`, or `mask: {image}` with an edge asset | P2 |
| Duotone, grayscale, sepia, tint | 5 | `adjust` | P1 |
| Diagonal blocks, polygon splits | 5 | `path`, `polygon`, rotated rects | P1 |
| Glow, neon | 5 | glow shadows, `blendMode: screen` | P1 |
| Blobs, wavy edges | 5 | `path` (`shape: "blob-1"` …, or own `d`) | P1 |
| Dashed, dotted lines | 5 | `dash`, `cap: round` | P1 |
| Vertical text | 4 | `rotation: -90` (fitting measured on the unrotated box) | MVP |
| Translucent overlaps, watermarks | 4 | `opacity` | MVP |
| 3D or neon text as art | 4 | raster images (not editable text, same as the originals) | MVP |
| Headline behind the subject | 4 | layer order + `allowOverlap` | P1 |
| Photo clipped to a polygon or blob | 3 | image fill on `path`/`polygon`, or `mask` | P1 |
| QR codes | 3 | SVG asset from `asset_add` (a QR service URL or another tool's output), `adjust.tint` to recolor; a built-in `qr` waits for templates | MVP |
| Price lists, schedules | 3 | stacks, `use` + `each`, `spacer` | P1 |
| Grid, collage | 3 | `grid` | P2 |
| Stat cards | 3 | `radius`, `grid` or stacks | P1 |
| Hand-drawn arrows, scribbles | 3 | `path` or `line` with a `rough` stroke; SVG assets | P2 / MVP |
| Circle photos with ring | 2 | `ellipse` image fill + stroke | P1 |
| Gradient or metallic text | 3 | text `fills` gradient | MVP |
| Halftone | 2 | `adjust.halftone` on the image, or `pattern: "dots"` | P2 |
| Sunburst rays | 2 | `pattern: "rays"` | P2 |
| Ribbons | 2 | `path` `shape: "ribbon"` | P1 |
| Outlined text | 2 | `fills: []` + stroke | P1 |
| Steps on a ring or path | 2 | positioned by hand (free layout) | MVP |
| Multi-column text | 2 | stack of text layers | MVP |
| Photo fading into background | 2 | gradient `mask` | MVP |
| Frosted glass | 1 | `backdropBlur` | P1 |
| Blurred background photo | 1 | `blur` | P1 |
| Double exposure | 1 | `blendMode`, `opacity` | MVP |
| Ring gauges with gradient arcs | 1 | `ellipse` arc + gradient stroke | P1 |
| Calendar grid | 1 | `grid` + `each` | P2 |
| Barcode | 1 | SVG asset | MVP |
| Dot leaders in a price list | 1 | `leader: "."` | P2 |
| Text on an arc | 1 | `curve` | P2 |
| Envelope-warped text | 1 | later (glyph warping) | later |
| Multi-size reflow set | 1 | `firstFit`, `dir` lists, grid `areas`, aspect classes | P1 / P2 |

Not rendered live in v2: envelope-warped text (1 template), which can use an image in the meantime. Not seen in this sample: maps, star ratings, device mockups, speech bubbles, mesh gradients.

## Animation readiness

v2 has no time, but it leaves room for it:

- **Post-layout properties are separate fields.** `opacity`, `rotation`, `scale`, `offset`, `skew`, `blur`, colors, shadows and `arc` animate without re-running layout, as in SwiftUI and CSS transforms. Layout fields (`width`, `padding`, `gap`, `fontSize`, `text`, `dir`) would re-run layout every frame, so animations will target post-layout fields first.
- **Every animatable field is a plain scalar, color or vector** at a stable path, so a later `animate` list can address it: `{"opacity": [0, 1], "duration": 0.4, "ease": "ease-out", "delay": 0.2}`.
- **Transitions between two scenes by role** (Figma Smart Animate, SwiftUI `matchedGeometryEffect`): layers with the same role interpolate. Penpot's prototype vocabulary (`dissolve`, `slide`, `push`; `after-delay`) is a ready seed for animated ads.
- Reserved keys, rejected by v2: `animate`, `duration`, `fps`, `transition`, and the `video` paint.

### Video (later)

- **Output.** A video is the scene drawn once per frame (e.g. 30 fps × 15 s = 450 frames) and passed to an encoder. Layout runs once; frames only redraw post-layout fields, so encoding dominates the cost. `render` would gain `format: mp4|webm|gif` and `time` (a still at that moment, e.g. a poster frame).
- **Video as a fill.** `{"video": "clip1", "start": 2, "end": 12, "loop": false, "speed": 1, "muted": false}` goes anywhere an image fill goes (full-bleed, in a circle, under a scrim), with the same `fit` and `focus`. The scene lasts as long as the video unless `duration` is set; the source audio is copied into the output without re-encoding.
- **Checks, not viewing.** `scene_describe` checks the scene at key moments ("at 2.1 s the headline overlaps the logo"); a preview is a strip of stills at those moments.
- **Encoder and license.** Ad platforms mostly want MP4 with H.264. Options:
  - ffmpeg run as a separate process: complete, encodes and decodes, and can use hardware encoders. Its default builds include GPL parts, so it's never linked into keyline: require an installed ffmpeg, or ship an LGPL-only build.
  - Cisco's OpenH264 (BSD) plus a permissive MP4 writer: in-process, but H.264 patent royalties are covered only when users download Cisco's own binary.
  - WebM (VP9 or AV1 via permissive encoders such as rav1e), GIF and animated WebP: simple, but not accepted everywhere.

  Starting point: ffmpeg as a separate process, since a video fill needs a decoder as well.

## MCP tools

The six tools stay; no per-property setters.

| Tool | Change |
|---|---|
| `scene_create` | `sizes` accepts presets and `"WxH"` |
| `asset_add` | Unchanged (background removal is spec step 3) |
| `layer_add` | Also takes `tokens` and `components`; `styles` take any fields |
| `layer_update` | Targets `{id}`, `{role}`, `{style}`, `{component}`, `{token}`; `detach` |
| `scene_describe` | Reports `firstFit` choices, wraps, `!unsafe`; `full` shows resolved sizes (`hug`/`fill` in px) |
| `render` | `format` `png`\|`jpeg`\|`webp`\|`pdf` (png), `quality` 0–100 (90), `maxKB`: lowers JPEG/WebP quality until the file fits (display ads are often capped at 150 KB), and reports the quality it used or `!too-big` |

The tool definitions carry the layer grammar, and today they total under 7,000 characters (a test enforces it). v2 more than doubles the grammar, so it can't all go there. Proposal: the `layer_add` description keeps a short cheat sheet of the common fields (≤ 9,000 characters for all tools); the full reference ships as an agent skill and an MCP prompt, and every unknown or misused field returns one line with the valid fields and their defaults, so the agent learns on the first mistake. Remotion deprecated its MCP server over token cost; Creatomate's on-demand `get_guide` is the pattern to follow.

## Migration

MVP scenes need no conversion: each MVP field is a v2 short form.

| MVP | v2 reading |
|---|---|
| `color`, `gradient` on rect, ellipse, frame | one fill |
| `stroke` | one stroke |
| `cornerRadius` | `radius` (the old name stays accepted) |
| text `shadow`, `outline`, `fill`, `gradient` | `shadows`, outside `strokes`, image and gradient `fills` |
| `ranges` `{start, end, color}` | unchanged; now any text field |
| `line` `color`, `strokeWidth` | its stroke |
| `mask` (gradient) | unchanged |
| `styles` (text only) | unchanged; now any fields |

A golden test renders every MVP fixture before and after and requires identical pixels, the reference ad in all three sizes included.

## Build order and measurement

Each step ships with unit and end-to-end tests and is judged on the benchmarks (reference ad in 3 sizes, recreate-a-design), at least 3 runs, against the MVP baseline.

| Step | Contents | Expected effect |
|---|---|---|
| P1a Layout | sizing, stack additions, `place`, `firstFit`, `dir` lists, aspect classes | Fewer `at` edits; skyscraper right without per-size work; lower cost |
| P1b Paint | fill/stroke/shadow arrays, radial and conic gradients, image fills on shapes, `blur`, `backdropBlur`, `adjust`, `noise`, masks, radius forms | New designs (cards, glass, photo in shapes, grain) at equal cost |
| P1c Text | markup, `highlight`, `textWrap`, `verticalAlign`, `decoration`, `trim` | No offset errors; highlight looks; fewer tokens for styled headlines |
| P1d Reuse | tokens, styles on any layer, components with `each` | Fewer tokens for repeated structures (candidates, steps) |
| P1e Shapes | `path` and the named-shape library, `polygon` (stars), arcs, markers, transforms | Seals, blobs, ribbons, gauges |
| P2 | grid, `pattern`, `edges`, `rough` strokes, `halftone`, `curve`, `leader`, `knockout`, image masks, `safe`, JPEG/WebP/PDF output with `maxKB` | Collages, calendars, menus, textures, stories; ad-network file limits; print |
| Later | warped text, boolean ops, mesh gradients, animation | Long tail |

## Code modules

v2 roughly doubles the format, so the code is split by area before it grows. Rule: a source file stays under about 400 lines, unit tests included; a file that passes it is split by responsibility, not by line count. Unit tests stay next to their code, in a sibling `tests.rs` when they would push a file over the limit (`#[cfg(test)] mod tests;`).

### Today

| File | Lines | Holds |
|---|---|---|
| `src/scene.rs` | 1,432 | Every scene type, defaults, key checks, tests |
| `src/render.rs` | 930 | Layer drawing, paints, text drawing, PNG, contact sheet |
| `src/describe.rs` | 724 | Defects, advisories, facts, `full` listing |
| `src/text.rs` | 717 | Paragraph building, fitting, drawn lines |
| `src/layout.rs` | 522 | Scale, constraints, stacks |
| `src/ops.rs` | 466 | `layer_add`, `layer_update`, styles, `at` |
| `src/server.rs` | 436 | The six MCP tools |

### Target layout

```
src/
  scene/          the format: types, defaults, short forms, key checks
    mod.rs        Scene, Size, size presets, Asset
    layer.rs      Layer, common fields, Kind
    paint.rs      Paint, Gradient, Stroke, Shadow, Mask, Edges
    text.rs       text fields, Range
    shapes.rs     polygon, path, named-shape lookup
    short.rs      short forms ("#fff" → a fill, padding shorthands, …)
  reuse/          resolved before layout
    tokens.rs     "$name" substitution, per-size token overrides
    styles.rs     style lists merged into layers
    components.rs `use` and `each` expansion, instance ids
    at.rs         per-size and aspect-class changes
  layout/
    mod.rs        entry point, sizing (hug, fill, %, min/max)
    free.rs       constraints, place
    stack.rs      stacks, wrap, priority, spacers, absolute children
    grid.rs       tracks, areas, spans
    first_fit.rs  firstFit and dir lists
  text/
    mod.rs        paragraph building
    markup.rs     inline markup → ranges
    fit.rs        resize modes, shrink to fit
    wrap.rs       balance and pretty wrapping
    extras.rs     highlight, leaders, curve, trim
  render/
    mod.rs        layer walk, render order, transforms
    fills.rs      colors, gradients, image, pattern, noise
    effects.rs    shadows, blur, backdrop blur, adjust, halftone
    masks.rs      masks, clip, knockout
    shapes.rs     path building, edges, rough strokes
    output.rs     PNG, JPEG, WebP, PDF, maxKB, contact sheet
  describe/
    mod.rs        per-size report
    defects.rs    !overflow, !overlaps, !unsafe, …
    facts.rs      smallest text, upscaling, firstFit choices
  ops.rs, server.rs, store.rs, fetch.rs, fonts.rs, icons.rs, gpu.rs   unchanged or lightly trimmed
```

The pipeline reads top to bottom: `scene` parses, `reuse` resolves tokens, styles, components and `at` into plain layers, `layout` places them per size, `text` measures, `render` draws, `describe` reports. Each stage consumes the previous one's output, so each can be tested alone.

### Order

1. **P0, split before adding.** Move today's code into this layout with no behaviour change: same tests, byte-identical golden PNGs. One commit per module, so each move is easy to review.
2. Build v2 features into their modules, following the build order above.


## Example: the reference ad in v2

A sketch, not yet benchmarked. The photo takes whatever height is left at each size; the candidates switch to a column where a row doesn't fit; no `at` entries.

```json
{
  "tokens": {"navy": "#1B2A5C", "red": "#D0202E", "grey": "#6B7280"},
  "styles": {
    "accent": {"color": "$red"},
    "name": {"fontSize": 36, "weight": 700, "color": "$navy", "align": "center"},
    "office": {"fontSize": 30, "weight": 500, "color": "$grey", "align": "center"}
  },
  "components": {
    "candidate": {"type": "frame", "stack": {"dir": "column", "gap": 4}, "children": [
      {"type": "text", "role": "name", "text": "{name}", "style": "name"},
      {"type": "text", "role": "office", "text": "{office}", "style": "office"}]},
    "step": {"type": "frame", "width": "fill", "stack": {"dir": "row", "gap": 16, "align": "center"}, "children": [
      {"type": "icon", "name": "circle-check", "color": "$red", "width": 40, "height": 40},
      {"type": "text", "role": "step", "text": "{text}", "fontSize": 32, "weight": 500, "color": "$navy", "width": "fill"}]}
  },
  "layers": [{"type": "frame", "width": "fill", "height": "fill", "stack": {"dir": "column", "gap": 32, "padding": [48, 0]}, "children": [
    {"type": "text", "role": "headline", "width": "fill", "padding": [0, 40], "fontSize": 64, "weight": 800, "color": "$navy", "align": "center", "textWrap": "balance",
     "text": "Proven <accent>RESULTS</accent> for <accent>WILLOWMERE</accent> Families"},
    {"type": "image", "role": "photo", "asset": "photo", "width": "fill", "height": "fill", "minHeight": 160},
    {"type": "frame", "role": "candidates", "width": "fill", "stack": {"dir": ["row", "column"], "justify": "evenly", "gap": 16}, "children": [
      {"type": "use", "component": "candidate", "each": [
        {"name": "Dana Levi", "office": "Mayor"}, {"name": "Omar Haddad", "office": "Council"}, {"name": "Ruth Cohen", "office": "Council"}]}]},
    {"type": "frame", "role": "cta", "width": "fill", "color": "$red", "stack": {"dir": "row", "gap": 16, "padding": 24, "justify": "center", "align": "center"}, "children": [
      {"type": "image", "asset": "mail", "width": 56, "height": 44},
      {"type": "text", "text": "VOTE BY MAIL", "fontSize": 48, "weight": 800, "color": "#FFFFFF"}]},
    {"type": "frame", "role": "steps", "width": "fill", "stack": {"dir": "column", "gap": 12, "padding": [0, 40]}, "children": [
      {"type": "use", "component": "step", "each": [
        {"text": "Request your ballot by October 20"}, {"text": "Fill it out at home"}, {"text": "Mail it back by November 3"}]}]},
    {"type": "text", "role": "footer", "width": "fill", "text": "Paid for by Willowmere Forward · willowmereforward.org", "fontSize": 20, "color": "$grey", "align": "center"}]}]
}
```

This example gives text layers `padding`, which the tables above don't define yet (see Open questions).

## Open questions

1. **Types per shape vs one `graphic` type.** This doc keeps `rect`, `ellipse`, `image`… (cheaper for agents, per the MVP benchmarks) over graphic-layer.md's single type. Agree?
2. **Markup always on,** or only when a `markup: true` flag is set? Always-on costs nothing, but a literal `<b>` in copy would need escaping.
3. **`padding` on any layer** (text, images), as CSS allows, or only on frames? The example uses it; it saves a wrapper frame per padded text.
4. **Components live-linked** (proposed) or expanded into plain layers on add? Linked keeps one-edit changes; expanded is simpler to build.
5. **Aspect-class thresholds:** decided while building: square is 0.9–1.1 (so a 4:5 post is portrait), and tall and wide include 1:2 and 2:1 (a 300 × 600 half-page is tall).
6. **Tool-definition budget:** raise the test's cap from 7,000 to 9,000 characters, and ship the full reference as a skill and an MCP prompt?
7. **QR codes** stay an SVG asset in v2. A built-in `qr` type (`{"type": "qr", "data": "{url}"}`) waits for the templates milestone, where `render_batch` needs a different code per row; it will need the `qrcode` crate (MIT/Apache), which needs owner approval.
8. Carried from graphic-layer.md: `path` scaling (`contain` proposed), per-side strokes on rounded corners, `diamond` gradients (dropped here: rare, and Skia has no native shader).
9. **Spec doc upkeep:** the spec's open questions still list PolyForm Noncommercial and the name "scene-mcp"; update them when this doc is accepted.
10. **Skia build features:** WebP encoding and PDF output are skia-safe build features (`webp`, `pdf`) we don't enable today. Before building them, check that prebuilt Skia binaries exist for our feature set on all release targets; otherwise the build compiles Skia from source.
11. **File-size rule:** put the 400-line rule for source files into CLAUDE.md, so it applies to every change, not just v2?

## Sources

- Templates: [PosterMyWall](https://www.postermywall.com/index.php/l/poster-templates) (39 templates across its gallery pages), [Kittl](https://www.kittl.com/templates/instagram-posts) (11), [Venngage](https://venngage.com/templates/social-media) (7), [Creatopy](https://www.creatopy.com/templates/) (3). Canva, Figma Community, VistaCreate and Freepik returned 403 to automated access; Adobe Express returned titles only.
- Figma: [auto layout](https://help.figma.com/hc/en-us/articles/360040451373-Guide-to-auto-layout), [FrameNode](https://developers.figma.com/docs/plugins/api/FrameNode/), [Paint](https://developers.figma.com/docs/plugins/api/Paint/), [Effect](https://developers.figma.com/docs/plugins/api/Effect/), [TextNode](https://developers.figma.com/docs/plugins/api/TextNode/), [corner smoothing](https://help.figma.com/hc/en-us/articles/360050986854-Adjust-corner-radius-and-smoothing), [variables](https://help.figma.com/hc/en-us/articles/15339657135383-Guide-to-variables-in-Figma), [transitions](https://developers.figma.com/docs/plugins/api/Transition/).
- SwiftUI: [ViewThatFits](https://developer.apple.com/documentation/swiftui/viewthatfits), [frame(min/max)](https://developer.apple.com/documentation/swiftui/view/frame(minwidth:idealwidth:maxwidth:minheight:idealheight:maxheight:alignment:)), [layoutPriority](https://developer.apple.com/documentation/swiftui/view/layoutpriority(_:)), [GridItem](https://developer.apple.com/documentation/swiftui/griditem), [Text](https://developer.apple.com/documentation/swiftui/text), [RoundedCornerStyle](https://developer.apple.com/documentation/swiftui/roundedcornerstyle), [matchedGeometryEffect](https://developer.apple.com/documentation/swiftui/view/matchedgeometryeffect(id:in:properties:anchor:issource:)).
- Penpot: [source](https://github.com/penpot/penpot) (read at commit 9d08e26: `common/src/app/common/types/`, `backend/src/app/binfile/v3.clj`), [file format](https://help.penpot.app/technical-guide/developer/data-model/penpot-file-format/), [flexible layouts](https://help.penpot.app/user-guide/designing/flexible-layouts/), [design tokens](https://help.penpot.app/user-guide/design-systems/design-tokens/), [libraries and templates](https://penpot.app/penpothub/libraries-templates), [penpot-files](https://github.com/penpot/penpot-files) (CC BY 4.0).
- Competitors: the competitive analysis (kept local, not committed).
