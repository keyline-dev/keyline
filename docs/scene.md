# Scene format

A scene is one JSON document: a master size, the target sizes, shared assets, styles, tokens and components, and a tree of layers. The agent writes it through the MCP tools ([tools.md](tools.md)); the server lays it out for each size and renders it. This page describes every part of it.

## Naming rules

Field names follow what models already know:

1. **CSS words where CSS has the concept** (`gap`, `padding`, `justify`, `minWidth`, `aspectRatio`, `letterSpacing`, `textWrap`, `blur`, `backdropBlur`), **Figma words for sizing and pinning** (`hug`, `fill`, `constraints`), and **SwiftUI words only where CSS has none** (`firstFit`, `priority`, `minFontScale`). Where two vocabularies differ, both are read: `fit: fill|fit` also takes `cover|contain`, `verticalAlign: center` also takes `middle`.
2. **Short forms.** A compound field has a one-token short form: `fills: "#fff"`, `padding: 24`, `radius: 12`, `stroke: "#000"`.
3. **Every field has a default, and defaults are omitted** in what the agent sends and in what the server stores.
4. **Effects are arrays, never `…Enabled` flags.** Absent means off.
5. **Deterministic.** The same scene renders the same PNG on a given OS version. Random-looking effects (grain, torn edges, rough strokes) take a `seed`.

## Document

```json
{
  "width": 1080, "height": 1350,
  "sizes": ["instagram-portrait", {"id": "wide", "width": 1200, "height": 1000, "scale": 0.85}, "300x600"],
  "background": "#FFFFFF",
  "tokens": {"navy": "#1B2A5C", "red": "#D0202E"},
  "styles": {"h1": {"fontSize": 64, "weight": 800, "color": "$navy"}},
  "components": {"step": {"type": "frame", "stack": {"dir": "row", "gap": 16}, "children": ["…"]}},
  "assets": {"photo": {"sha256": "…", "width": 1600, "height": 900}},
  "layers": ["…"]
}
```

| Field | Meaning | Default |
|---|---|---|
| `width`, `height` | Master size, px: the layers are written at this size | the first size's |
| `sizes` | Output sizes: `{id, width, height, scale, safe}`, a preset name, or `"WxH"` (its id is that string) | required |
| `background` | Canvas color | `#FFFFFF` |
| `tokens` | Named values used as `"$name"` in any field | none |
| `styles` | Named sets of layer fields, applied by `style` | none |
| `components` | Named layer trees, placed by `use` layers | none |
| `assets` | Images, SVGs and video clips added by `asset_add`, stored by content hash | none |
| `layers` | The layer tree, bottom to top | none |
| `duration`, `fps`, `loop` | Motion: see [Motion](#motion) | a still |

A size's `scale` (1) shrinks everything, fonts included, before the layout adapts to the size, like a design tool's Scale tool. `safe` (`[top, right, bottom, left]` px) is the part a platform covers, such as a story's UI bars; text there is reported as `!unsafe`.

Presets: `instagram-portrait` 1080×1350, `instagram-square` 1080×1080, `instagram-story` 1080×1920 (safe 250 top, 340 bottom), `facebook-feed` 1200×628, `linkedin-post` 1200×627, `x-post` 1600×900, `youtube-thumbnail` 1280×720, `iab-medium-rectangle` 300×250, `iab-leaderboard` 728×90, `iab-skyscraper` 160×600, `iab-half-page` 300×600, `a4-portrait` 2480×3508 (300 dpi).

## Layers

| `type` | What it is | Its own fields (defaults) |
|---|---|---|
| `frame` | Container with free, stack or grid layout | `children`, `clip` (true), `stack`, `grid` |
| `text` | Text in a box | see [Text](#text) |
| `image` | An image in a box | `asset`, `fit` (`fill`), `focus` ([0.5, 0.5]), `crop`, `tileScale` (1), `adjust` |
| `video` | A video clip in a box, playing in a moving scene | see [Video](#video) |
| `rect` | Rectangle | |
| `ellipse` | Ellipse in the box; arcs and rings | `arc {start, end, inner}` (0, 360, 0) |
| `polygon` | Regular polygon in the box; a star with `innerRadius` | `sides` (3), `innerRadius` (the inner points' share of the outer radius: 0.38 for a classic star, 0.8 for a starburst) |
| `path` | An SVG path, or a named shape | `d` or `shape`, `fillRule` (`nonzero`), `fitPath` (`contain`) |
| `line` | From the box's top-left by `width, height` | `color`, `strokeWidth` |
| `icon` | A named icon | `name`, `set` (`lucide`, or Font Awesome `solid`, `regular`, `brands`), `color`, `strokeWidth` |
| `spacer` | Flexible empty space in a stack | `minLength` (0) |
| `firstFit` | Draws the first child that fits (SwiftUI `ViewThatFits`) | `children` |
| `use` | Instances of a component | `component`, `props`, `each` |

Named shapes for `path` (and for shape masks): `ribbon`, `ribbon-banner`, `bubble`, `bubble-round`, `arrow`, `arrow-curved`, `chevron`, `tag`, `arch`, `shield`, `heart`, `cloud`, `wave`, `burst`, `blob-1` … `blob-6`, `brush-stroke`. A named shape is a real path, so every paint applies: a photo in a blob, a gradient ribbon, a dashed speech bubble.

Icons: about 5,000 by name, [Lucide](https://lucide.dev) outline icons and [Font Awesome Free](https://fontawesome.com) solid, regular and brand icons. An icon is 24 px tall unless sized.

### Fields every layer takes

| Field | Meaning | Default |
|---|---|---|
| `id` | Stable id | generated (`text1`, `rect2`, …) |
| `role` | Semantic name; an edit can target every layer with a role | none |
| `parent` | (`layer_add` only) The frame to add the layer into | top level |
| `x`, `y` | Position in the parent, px or `"25%"`; ignored in a stack or grid | 0 |
| `width`, `height` | px, `"hug"` (fit the content), `"fill"` (take the free space), or `"40%"` of the parent | by type: text and images size themselves, stack and grid frames hug, others are 100 |
| `minWidth`, `maxWidth`, `minHeight`, `maxHeight` | Clamps, px | none |
| `aspectRatio` | Width ÷ height, kept when only one side is set | none |
| `constraints` | How the layer follows its parent in free layout: `{h: left\|right\|center\|stretch\|scale, v: top\|bottom\|center\|stretch\|scale}` | left, top |
| `place`, `inset` | Pins to one of nine spots of the parent (`"top-left"` … `"center"` … `"bottom-right"`) at `inset` px (or `[x, y]`) from the edges, at every size | none |
| `hidden` | Not drawn and takes no space | false |
| `opacity` | 0–1, the layer and its children as one | 1 |
| `blendMode` | One of the 16 CSS blend modes | `normal` |
| `fills`, `strokes`, `shadows` | See [Paint](#paint) | by type |
| `blur`, `backdropBlur` | Blur of the layer, and of what's behind it within its shape, px | 0 |
| `radius` | Corners: px, `[tl, tr, br, bl]`, or `"full"` (a capsule at every size) | 0 |
| `mask` | See [Masks](#masks) | none |
| `edges` | Torn sides: `{sides, depth, seed}` | none |
| `rotation` | Degrees, clockwise, about the box center | 0 |
| `scale`, `offset`, `skew`, `flipX`, `flipY` | Visual transforms after layout, about the box center; they never move other layers | 1, [0, 0], [0, 0], false, false |
| `style` | A style name, or a list applied in order; the layer's own fields win | none |
| `at` | Changes for one size or aspect class, see [Per size](#per-size) | none |
| `in`, `out`, `animate`, `stagger`, `split` | Motion, see [Motion](#motion) | none |
| `shot` | Makes a top-level frame a shot, see [Shots and transitions](#shots-and-transitions) | none |

In a stack, children also take `alignSelf`, `grow`, `priority` and `position`; in a grid, `area`, `cell` and `span` (below).

## Layout

A frame lays out its children in one of three ways: **free** (each child's `x`, `y`, `constraints` or `place`), **stack** (a row or column, like CSS flexbox and Figma auto layout) or **grid** (like CSS grid). Children of a stack or grid ignore `x`, `y` and `constraints` unless they set `position: "absolute"`, which takes them out of the flow and places them like a free child (a badge over a card's corner).

Rendering a size scales the master by the size's `scale`, then fits it to the target size: free layers follow their constraints, stacks and grids lay out again, recursively. There's no constraint solver.

### Sizing

| Value | Meaning | Figma | SwiftUI | CSS |
|---|---|---|---|---|
| `320` | Fixed px (scaled by the size's `scale`) | Fixed | `.frame(width:)` | `320px` |
| `"hug"` | As big as the content | Hug | ideal size | `fit-content` |
| `"fill"` | The free space in a stack or grid; the rest of the parent in free layout | Fill | `maxWidth: .infinity` | `flex: 1` |
| `"40%"` | Share of the parent's size | | `containerRelativeFrame` | `40%` |

Min and max clamps apply last.

### Stacks

```json
"stack": {"dir": "row", "gap": 16, "padding": [24, 32], "align": "center", "justify": "between", "wrap": true}
```

| Field | Values | Default |
|---|---|---|
| `dir` | `row`, `column`, `row-reverse`, `column-reverse`, or a list tried in order: `["row", "column"]` is a row where it fits, else a column | required |
| `gap` | px, or `[rowGap, columnGap]` | 0 |
| `padding` | px, `[vertical, horizontal]`, or `[top, right, bottom, left]` | 0 |
| `align` | Across: `start`, `center`, `end`, `stretch`, `baseline` | `start` |
| `justify` | Along: `start`, `center`, `end`, `between`, `around`, `evenly` | `start` |
| `wrap` | Wrap onto more lines when they don't fit | false |

Children may set `alignSelf` (their own `align`), `grow` (their share of the free space when they `fill`, 1) and `priority` (0; when a row is too narrow, lower priorities give way first, like SwiftUI's `layoutPriority`). A `spacer` takes the leftover space.

### Grids

```json
"grid": {"columns": "2fr 1fr", "rows": "2fr 1fr", "gap": 24, "areas": ["photo side", "cta side"]}
```

| Field | Values | Default |
|---|---|---|
| `columns` | CSS tracks (`200px`, `1fr`, `auto`, `25%`, `repeat(3, 1fr)`), a count (`3` = three `1fr`), or `{"min": 160}` for as many equal columns as fit at that width or more | one `1fr` per `areas` column, else one |
| `rows` | Tracks, as `columns`; rows beyond them are `auto` | `auto` |
| `gap` | px, or `[rowGap, columnGap]` | 0 |
| `padding` | As a stack's | 0 |
| `areas` | Named areas, one string per row and a name per column; `.` is empty. Each name must form a rectangle | none |

A child takes `area: "photo"`, or `cell: [row, column]` (counted from 1) with `span: [rows, columns]`; otherwise it fills the next free cell, row by row. Children stretch to their cell, except on a side with a px size, where they sit at the cell's start. A size can rearrange the whole grid by changing only `columns`, `rows` and `areas` in `at`. Tracks, cells and spans go up to 100.

### Layouts that pick what fits

`firstFit` draws the first child that fits its box at this size with nothing wrong inside it (no overflow or truncated text, no text shrunk below its `minFontScale`). Typical uses: a long and a short headline, a row CTA and a stacked CTA. A stack's `dir` list is the common case in one field. `scene_describe` shows what was chosen at each size (`→ column`, `→ short`).

### Per size

`at` changes a layer's fields for some sizes: `"at": {"sky": {"fontSize": 20}}`. Its keys are a size id or an aspect class, applied broadest first: `landscape` (wider than 1.1:1), `square` (0.9–1.1) or `portrait`, then `wide` (2:1 or wider) or `tall` (1:2 or taller), then the size id. A 4:5 post is portrait; a 300×600 half-page is tall. One `"tall"` entry covers every skyscraper the design is ever rendered at. `at` can't change a layer's `id`, `type` or `children`.

## Paint

Frames, shapes, images, text and icons take the same paint fields. `color` and `gradient` are short forms of a single fill; `stroke` of a single stroke.

### Fills

`fills` is one paint or a list, bottom to top; `[]` fills nothing (outlined text, for example).

| Paint | Example |
|---|---|
| Color | `"#D0202E"`, `"#D0202E80"`, a CSS name, or `{"color": "$red", "opacity": 0.5}` |
| Gradient | `{"gradient": {"type": "radial", "stops": ["#0000", "#000C"]}}`; also written flat, `{"type": "linear", "angle": 180, "stops": […]}` |
| Image | `{"image": "photo", "fit": "fill", "focus": [0.5, 0.3], "adjust": {"grayscale": 1}}` |
| Pattern | `{"pattern": "dots", "color": "#0002", "size": 12}`: `dots`, `stripes`, `grid`, `checker`, `zigzag`, `rays` |
| Grain | `{"noise": 0.08, "seed": 1}` |

Every paint takes `opacity` (1) and `blendMode` (`normal`). An image fill works on any shape: a photo in a circle is `{"type": "ellipse", "fills": {"image": "photo"}}`.

**Gradients:** `type` `linear` (default), `radial` or `conic`. Linear takes a CSS `angle` in degrees (0 = up, 90 = right) or `from`/`to` points ([x, y], 0–1 of the box). Radial takes `center` ([0.5, 0.5]) and `radius`. `stops` is a list of colors, evenly spaced, or of `{at, color}` (`at` 0–1 or `"55%"`; `offset`, `position` and `pos` also read).

**Colors:** `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` or a CSS color name.

### Images

| Field | Meaning | Default |
|---|---|---|
| `fit` | `fill` (cover the box, cropping), `fit` (contain, letterboxed) or `tile` (repeat) | `fill` |
| `focus` | [x, y], 0–1: the point that stays in view when `fill` crops | [0.5, 0.5] |
| `crop` | `{x, y, width, height}`, 0–1 of the image: show only that part | none |
| `tileScale` | Tile size for `tile`, × the image's size | 1 |
| `adjust` | `brightness`, `contrast`, `saturate` (−1…1), `grayscale`, `sepia` (0…1), `hue` (degrees), `duotone` ([dark, light] colors), `tint` (recolor every visible pixel, e.g. a logo in white), `halftone` (dot spacing px: the image redrawn as black dots, larger where it's darker) | none |

### Strokes

`strokes` is one stroke or a list; `"#000"` is a 1 px stroke.

| Field | Meaning | Default |
|---|---|---|
| `width` | px; `[top, right, bottom, left]` on rects for per-side borders | required |
| `color`, `gradient` | Its paint | black |
| `align` | `inside`, `center` or `outside` the edge | `inside` (`center` for lines) |
| `dash` | `[on, off]` px | solid |
| `cap`, `join` | `butt`\|`round`\|`square`; `miter`\|`round`\|`bevel` | `butt`, `miter` |
| `start`, `end` | Markers on lines and paths: `arrow`, `triangle`, `circle`, `diamond` | none |
| `rough`, `seed` | Hand-drawn wobble, px | 0 |

### Shadows and blur

`shadows` is one shadow or a list, like CSS `box-shadow`: `{x, y, blur, spread, color, inset}`. `inset: true` is an inner shadow. A shadow follows the layer's shape: it hugs a cutout photo or the letters of a text. A glow is a shadow at `x: 0, y: 0`; a hard offset shadow has `blur: 0`.

`blur` blurs the layer; `backdropBlur` blurs what's behind it within its shape (frosted glass).

### Masks

| `mask` | Shows |
|---|---|
| a gradient | The layer faded by the gradient's alpha (a photo fading out) |
| `"ellipse"` or a named shape (`"blob-3"`) | The layer inside that shape |
| `{"path": "M…"}` | The layer inside that path |
| `{"layer": "logo"}` | The layer where another layer is; the mask layer isn't drawn itself |
| `{"image": "torn-edge"}` | The layer where an image is opaque |

`"mode": "luminance"` uses brightness instead of alpha; `"invert": true` reverses it. Frames clip their children unless `clip: false`.

### Edges

`edges: {"sides": ["top", "bottom"], "depth": 12, "seed": 1}` tears the chosen sides of any layer's box, like ripped paper. `sides` defaults to all four; `depth` (12) is how far the tears cut in, px.

## Text

| Field | Meaning | Default |
|---|---|---|
| `text` | The text, with optional [markup](#inline-markup) | required |
| `fontSize` | px; the largest size when the text shrinks to fit | 16 |
| `weight` | 100–900 | 400 |
| `fontFamily` | Inter (bundled), any [Google Fonts](https://fonts.google.com) family (downloaded on first use), or an installed font | Inter |
| `color` | Text color; `fills` can paint it with a gradient, image or pattern instead | black |
| `align` | `left`, `center`, `right`, `justify` | `left` |
| `verticalAlign` | `top`, `center`, `bottom` within the box | `center` for a fixed box, else `top` |
| `lineHeight` | × the font size | the font's |
| `letterSpacing` | px | 0 |
| `textCase` | `upper`, `lower`, `capitalize` | none |
| `italic` | Italic face | false |
| `decoration` | `underline`, `strike` | none |
| `textWrap` | `balance` (even line lengths), `pretty` (no lone last word) | `wrap` |
| `maxLines` | Lines before the ellipsis | none |
| `trim` | `"cap"`: trims the space above cap height and below the baseline, so text centers optically in pills and buttons | none |
| `padding` | Space around the text inside its box, as a stack's `padding` | 0 |
| `highlight` | A box behind each line: a color, or `{color, padding, radius, style: box\|brush}` | none |
| `curve` | Sets one line of text on a circular arc of this radius, px; negative bends down | none |
| `leader` | A character that fills each tab's gap: `"Espresso\t$3"` with `leader: "."` draws dot leaders, the price flush right | none |
| `knockout` | The letters cut through their parent frame's fill, showing what's behind | false |
| `direction` | `auto`, `ltr`, `rtl` | `auto` |
| `features` | OpenType features, e.g. `{"tnum": 1}` | none |
| `ranges` | `[{start, end, …}]`: character offsets of the displayed text with their own `color`, `weight`, `italic`, `fontSize`, `fontFamily`, `decoration`, `highlight` | none |

`strokes` outline the letters (`fills: []` with a stroke makes outlined text) and `shadows` follow their shapes.

### Fitting

The box decides how text fits:

- **Width and height:** the font shrinks until the text fits (down to `minFontScale`, 0.5 of `fontSize`), then ends with an ellipsis. `resize: "fixed"` keeps the size and lets it overflow; `"truncate"` keeps the size and cuts with an ellipsis.
- **Width only:** the text wraps and the box grows down.
- **Neither:** one line, as wide as the text.

When text still doesn't fit, `scene_describe` reports `!overflow` or `!truncated`, never a silent change.

### Inline markup

Models miscount character offsets, so text takes a small HTML subset instead:

```json
{"type": "text", "style": "h1", "text": "Proven <accent>RESULTS</accent> for <accent>WILLOWMERE</accent> Families"}
{"type": "text", "text": "<s>$49</s> <b>$29</b><sup>99</sup> today"}
```

- Tags: `<b>`, `<i>`, `<u>`, `<s>`, `<sup>`, `<sub>`, `<br>`, a style's name as a tag (`<accent>`), and `<span …>` with any range field as an attribute (`<span color="#D0202E" weight="800" highlight="#FFE600">`). Attribute values can be tokens (`color="$red"`).
- A `<` that doesn't open a known tag is text; `&lt;`, `&gt;`, `&amp;` and `&quot;` are entities.
- A `<span>` whose attributes don't parse is an error, not text.

## Reuse

### Tokens

`tokens` holds named values, and any field takes `"$name"` (letters, digits, `_`, `.` and `-`). The server remembers which fields came from which token, so changing a token in `layer_update` changes every field bound to it. Setting such a field to a value of its own unbinds it. `text` is a token only when it is wholly the name of one (`"text": "$headline"`, the way a template names its variables); otherwise it is never scanned, so `"$29"` stays text.

### Styles

`styles` hold any layer fields: a `card` style can carry fills, radius and shadows. A layer's `style` takes one name or a list; a later style wins where they overlap, and the layer's own fields win over all of them. Changing a style through `layer_update` changes every layer that uses it.

### Templates

A template is a scene file that `scene_create` loads by `url` or `path` ([tools.md](tools.md#scene_create)). It has a scene's fields (`sizes`, `background`, `tokens`, `styles`, `components`, `layers`, `duration` …), and its `assets` name files instead of hashes: a URL, or a path relative to the template.

```json
{
  "sizes": ["instagram-square", "iab-medium-rectangle"],
  "tokens": {"headline": "Spring sale", "price": "$29", "accent": "#D0202E"},
  "assets": {"photo": "photo.jpg", "logo": "https://example.com/logo.svg"},
  "layers": [
    {"type": "image", "asset": "photo", "width": "fill", "height": "fill"},
    {"type": "text", "text": "$headline", "fontSize": 64, "weight": 800, "color": "$accent", "place": "center"}
  ]
}
```

Its tokens are its variables: `scene_create` sets them with `tokens`, and `render` makes one file per row of them with `rows`. A layer whose `text` is exactly `"$name"` of a token takes the token's value; other text is never scanned. A template from a URL reads its images from the web only, never from local files; one from a path reads only inside the allowed folders. A scene keyline saved (its `assets` by `sha256`) loads as a template too.

### Components

```json
"components": {
  "candidate": {"type": "frame", "stack": {"dir": "column", "gap": 4, "align": "center"}, "children": [
    {"type": "text", "role": "name", "text": "{name}", "style": "name"},
    {"type": "text", "role": "office", "text": "{office}", "style": "office"}]}
}
```

```json
{"type": "use", "id": "c", "component": "candidate", "each": [
  {"name": "Dana Levi", "office": "Mayor"},
  {"name": "Omar Haddad", "office": "Council"},
  {"name": "Ruth Cohen", "office": "Council"}]}
```

- `{prop}` in any string of a component is filled from the instance's props: a string that is only `{prop}` takes the value as is (a number stays a number). `props` apply to every instance, and `each` places one instance per entry, in the parent's flow.
- The `use` layer's own fields (width, constraints, `at`…) apply to each instance's root.
- Instances stay linked: changing the component changes every instance. Their layers are named by the `use` id, the instance number and the inner layer's id or role, e.g. `c.1.name`. `layer_update` with `detach: true` turns a `use` into plain layers that no longer follow the component.
- Components may place other components, up to 8 levels deep.

## Motion

A scene with a `duration`, or made of [shots](#shots-and-transitions), moves. Only fields that don't change layout animate, so the layout is the same at every moment and every check about it holds throughout; a scene without motion fields is drawn at rest. `render` makes an animated PNG (`format: "apng"`), a GIF (`format: "gif"`), an MP4 or WebM video (`format: "mp4"`, `"webm"`, with ffmpeg), or a still at any moment (`time`).

| Scene field | Meaning | Default |
|---|---|---|
| `duration` | Length, seconds; its presence makes the scene move | none (a still), or where the last shot ends |
| `fps` | Frames per second | 30 |
| `loop` | The animation repeats forever | false (plays once) |

### Enter and exit

```json
{"type": "text", "text": "…", "in": "fade-up"}
{"type": "frame", "in": {"effect": "pop", "at": 1.2, "ease": "back.out"}, "out": {"effect": "fade", "at": 7}}
```

`in` and `out` take an effect name, or `{effect, at, duration, ease, distance}`. A layer is hidden before its `in` and gone after its `out`.

| Field | Meaning | Default |
|---|---|---|
| `effect` | `fade`, `fade-up`, `fade-down`, `fade-left`, `fade-right` (fade while moving `distance` into place), `pop` (grow from 0.6 with an overshoot), `zoom-in` (grow from 0.85), `zoom-out` (shrink from 1.15), `blur-in` (sharpen from a 12 px blur) | required |
| `at` | Start, seconds | `in`: 0; `out`: so it ends with the scene |
| `duration` | Seconds | 0.6 |
| `ease` | See [Easing](#easing) | `power2.out` entering (`back.out` for `pop`), `power2.in` leaving |
| `distance` | How far a directional fade travels, px | 40 |

### Keyframes

`animate` sets values over time, GSAP-style: one track or a list of them.

```json
"animate": {"scale": [1, 1.06, 1], "duration": 1.6, "repeat": -1, "ease": "sine.inOut"}
"animate": {"rotation": {"from": "random(-90, 90)"}, "offset": {"from": [0, -80]}, "duration": 0.8, "ease": "back.out"}
```

| Field | Meaning | Default |
|---|---|---|
| `opacity`, `scale`, `rotation`, `blur` | A list of numbers spread over `duration`, or `{from, to}` (a missing end is the layer's own value) | |
| `offset`, `skew` | The same, with `[x, y]` pairs | |
| `color` | The same, with colors: the layer's own color | |
| `times` | Where each listed value falls, 0–1 of `duration` | evenly spaced |
| `at` | Start, seconds | 0 |
| `duration` | One play, seconds | 1 |
| `ease` | Between each pair of values | `power1.inOut` |
| `repeat` | Extra plays; −1 repeats to the end | 0 |
| `yoyo` | Every other play runs backwards | false |

A number may be `"random(lo, hi)"`, as in GSAP: each target gets its own value (each layer, or each piece of split text), the same on every render. Values hold before a track starts and after it ends. Layout fields (`width`, `fontSize`, `text`, `padding`…) can't animate; to make something grow, animate `scale`.

### Easing

GSAP's names: `none`, `power1` … `power4`, `sine`, `expo`, `circ`, `back`, `elastic`, `bounce`, each with `.in`, `.out` or `.inOut` (a family alone is `.out`), and `steps(n)`. CSS's `ease`, `ease-in`, `ease-out` and `ease-in-out`, and `smooth`, `snappy` and `bouncy`, read as the nearest of those.

### Stagger and split text

`stagger` (seconds) on a frame or a `use` layer gives its `in` to its children or instances one after another instead of entering whole.

`split: "chars"` or `"words"` on a text layer makes its `in`, `out`, `animate` and `stagger` apply to each letter or word, GSAP's SplitText. The text is laid out once; each piece moves as a rigid part of it.

```json
{"type": "text", "text": "Animate Anything", "fontSize": 96, "split": "chars", "stagger": 0.05,
 "animate": {"offset": {"from": ["random(-400, 400)", "random(-250, 250)"]}, "rotation": {"from": "random(-180, 180)"},
             "opacity": {"from": 0}, "duration": 1.1, "ease": "back.out"}}
```

Frames clip their children, animated ones included: give a frame `clip: false` when its children move beyond its edges.

### Video

A `video` layer plays a clip added with `asset_add`, drawn like an image and under any layers above it: titles, captions, logos.

```json
{"type": "video", "asset": "beach", "width": "fill", "height": "fill", "start": 2, "speed": 0.5, "audio": false}
```

| Field | Meaning | Default |
|---|---|---|
| `asset` | A clip from `asset_add` (MP4, MOV, WebM…) | required |
| `fit`, `focus`, `crop`, `adjust` | As for [images](#images), applied to every frame | `fill`, center |
| `start` | Where in the clip to begin, seconds | 0 |
| `delay` | When the clip starts playing in the scene, seconds; before, its first frame holds | 0 |
| `speed` | Playback speed: 0.5 is slow motion | 1 |
| `loop` | Repeat the clip until the scene ends; otherwise its last frame holds | false |
| `audio` | Play the clip's own sound in MP4 and WebM output | true |

Each clip's sound plays with its pictures: from `start`, at `speed`, looping with it, and only while its shot is on; several clips' sounds are mixed. Stills (`time`, or a scene at rest) show the clip's frame at that moment. Decoding clips and writing MP4 or WebM needs [ffmpeg](https://ffmpeg.org), found on the PATH (or `KEYLINE_MCP_FFMPEG`) when a call needs it; everything else, APNG and GIF included, works without it.

### Shots and transitions

A top-level frame with `shot` is a shot: shots play one after another instead of stacking, each joined to the one before by a transition. Times inside a shot (`in`, `animate`, a clip's `delay`) count from the shot's own start. Layers that aren't shots, such as a logo or a caption bar, stay on across all of them. At rest, the first shot shows.

```json
{"id": "s1", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 3}, "children": ["…"]}
{"id": "s2", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 3, "transition": "push-left"}, "children": ["…"]}
```

| Field | Meaning | Default |
|---|---|---|
| `duration` | Seconds on screen, including its transitions | required |
| `transition` | How it enters from the shot before: a name, or `{type, duration, ease}` | `cut` |

Transitions: `cut`, `fade`, `slide-left`, `slide-right`, `slide-up`, `slide-down` (slides in over the last shot), `push-left` … `push-down` (pushes the last shot out), `wipe-left` … `wipe-down` (a moving edge reveals it) and `zoom` (the last shot grows and fades). A transition lasts 0.5 s with `power2.inOut` unless given, and overlaps the two shots, so each shot starts where the one before ends minus its transition. The scene's length is where the last shot ends unless `duration` says otherwise.

## Example

The reference ad from the end-to-end tests, in one `layer_add`: tokens, styles, two components placed with `each`, and a layout that adapts to a portrait post, a wide banner and a skyscraper without per-size positions.

```json
{
  "tokens": {"navy": "#1B2A5C", "red": "#D0202E", "grey": "#6B7280"},
  "styles": {
    "accent": {"color": "$red"},
    "name": {"fontSize": 36, "weight": 700, "color": "$navy", "align": "center"},
    "office": {"fontSize": 30, "weight": 500, "color": "$grey", "align": "center"}
  },
  "components": {
    "candidate": {"type": "frame", "stack": {"dir": "column", "gap": 4, "align": "center"}, "children": [
      {"type": "text", "role": "name", "text": "{name}", "style": "name"},
      {"type": "text", "role": "office", "text": "{office}", "style": "office"}]},
    "step": {"type": "frame", "width": "fill", "stack": {"dir": "row", "gap": 16, "align": "center"}, "children": [
      {"type": "image", "asset": "check", "width": 40, "height": 40},
      {"type": "text", "role": "step", "text": "{text}", "fontSize": 32, "weight": 500, "color": "$navy", "width": "fill"}]}
  },
  "layers": [{"id": "page", "type": "frame", "width": "fill", "height": "fill",
    "stack": {"dir": "column", "gap": 28, "padding": [48, 0]}, "children": [
    {"id": "headline", "type": "text", "width": "fill", "padding": [0, 40], "fontSize": 64, "weight": 800,
     "color": "$navy", "align": "center", "textWrap": "balance",
     "text": "Proven <accent>RESULTS</accent> for <accent>WILLOWMERE</accent> Families"},
    {"id": "photo", "type": "image", "asset": "photo", "width": "fill", "height": "fill", "minHeight": 120},
    {"id": "cands", "type": "frame", "width": "fill", "stack": {"dir": ["row", "column"], "justify": "evenly", "gap": 12},
     "children": [{"id": "c", "type": "use", "component": "candidate", "each": [
        {"name": "Dana Levi", "office": "Mayor"}, {"name": "Omar Haddad", "office": "Council"},
        {"name": "Ruth Cohen", "office": "Council"}]}]},
    {"id": "cta", "type": "frame", "width": "fill", "color": "$red",
     "stack": {"dir": "row", "gap": 16, "padding": 22, "justify": "center", "align": "center"}, "children": [
        {"type": "icon", "name": "mail", "color": "#FFFFFF", "width": 48, "height": 48},
        {"type": "text", "text": "VOTE BY MAIL", "fontSize": 48, "weight": 800, "color": "#FFFFFF"}]},
    {"id": "steps", "type": "frame", "width": "fill", "stack": {"dir": "column", "gap": 12, "padding": [0, 60]}, "children": [
        {"id": "s", "type": "use", "component": "step", "each": [
            {"text": "Request your ballot by October 20"}, {"text": "Fill it out at home"},
            {"text": "Mail it back by November 3"}]}]},
    {"id": "footer", "type": "text", "width": "fill", "text": "Paid for by Willowmere Forward · willowmereforward.org",
     "fontSize": 20, "color": "$grey", "align": "center", "at": {"tall": {"hidden": true}}}]}]
}
```

The photo takes whatever height is left at each size, the candidates switch to a column where a row doesn't fit, and the footer is dropped on tall sizes.
