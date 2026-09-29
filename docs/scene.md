# Scene format

A scene is one JSON document: a master size, the sizes it renders at, its assets, shared tokens, styles and components, and a tree of layers. This page is the reference for every field. The ideas behind it are in [concepts.md](concepts.md), and the tools that write and render it in [tools.md](tools.md).

```json
{
  "sizes": ["instagram-square", "iab-skyscraper"],
  "tokens": {"brand": "#D0202E"},
  "layers": [
    {"type": "rect", "width": "fill", "height": "fill", "color": "#FFF4E0"},
    {"type": "text", "text": "Cold Brew <b>Season</b>", "fontSize": 96, "weight": 800,
     "color": "$brand", "width": "80%", "place": "center", "at": {"tall": {"fontSize": 64}}}
  ]
}
```

**Contents:** [Conventions](#conventions) · [Document](#document) · [Layers](#layers) · [Layout](#layout) · [Text](#text) · [Paint](#paint) · [Reuse](#reuse) · [Motion](#motion) · [Template files](#template-files) · [Validation and limits](#validation-and-limits) · [Example](#example) · [Names](#appendix-names)

## Conventions

Field names follow what models already know:

1. **CSS words where CSS has the concept** (`gap`, `padding`, `justify`, `minWidth`, `aspectRatio`, `letterSpacing`, `textWrap`, `blur`, `backdropBlur`), **Figma words for sizing and pinning** (`hug`, `fill`, `constraints`), and **SwiftUI words only where CSS has none** (`firstFit`, `priority`, `minFontScale`). Where two vocabularies differ, both are read: `fit: fill|fit` also takes `cover|contain`, `verticalAlign: center` also takes `middle`.
2. **Short forms.** A compound field has a one-token short form: `fills: "#fff"`, `padding: 24`, `radius: 12`, `stroke: "#000"`.
3. **Every field has a default, and defaults are omitted** in what the agent sends and in what the server stores.
4. **Effects are arrays, never `…Enabled` flags.** Absent means off.
5. **Unknown fields are rejected**, with the nearest known name suggested.

### Value types

The tables below use these types.

| Type | Values |
|---|---|
| px | A number of pixels at the master size; each size's `scale` scales it |
| Length | px, `"hug"` (as big as the content), `"fill"` (the free space), or `"40%"` of the parent ([Sizing](#sizing)) |
| Color | `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` or a CSS color name |
| Paint | A Color, or a gradient, image, pattern or grain object ([Fills](#fills)) |
| Sides | px for all four, `[vertical, horizontal]`, or `[top, right, bottom, left]` |
| Point | `[x, y]`, each 0–1 of the box, from its top-left |
| Seconds | A number of seconds |
| Degrees | A number of degrees, clockwise |
| Ease | An easing name ([Easing](#easing)) |
| Token | `"$name"` in place of any value ([Tokens](#tokens)) |

### Resolution order

A layer's final values are built in this order; each step wins over the one before:

1. **Tokens**, when the layer is written: every `"$name"` becomes the token's value. `text` is replaced only when it is wholly a token's name.
2. **Components**, for a `use` layer: `{prop}` is filled from the instance's props, and the `use` layer's own fields replace the component root's, field by field.
3. **Styles**, in the order listed; a later style wins. The layer's own fields win over all styles.
4. **`at`**, per size: aspect classes broadest first, then the size id.

`at` and `layer_update`'s `set` merge like a JSON merge patch: nested objects (`stack`, `grid`, an object `in`) merge field by field, while lists (`fills`, `ranges`, `children`) and plain values are replaced whole, and `null` resets a field. Styles and a `use` layer's fields replace whole top-level fields.

## Document

| Field | Type | Default | Meaning |
|---|---|---|---|
| `sizes` | list of Size | required | The sizes to render ([Sizes](#sizes)) |
| `width`, `height` | px | the first size's | The master size: the layers are written at this size |
| `background` | Color | `#FFFFFF` | Canvas color |
| `tokens` | object | none | Named values, used as `"$name"` ([Tokens](#tokens)) |
| `styles` | object | none | Named sets of layer fields ([Styles](#styles)) |
| `components` | object | none | Named layer trees ([Components](#components)) |
| `assets` | object | none | Images, SVGs and video clips added by `asset_add`, by id: `{sha256, width, height}` |
| `layers` | list of Layer | none | The layer tree, bottom to top |
| `duration`, `fps`, `loop` | | a still | [Scene timing](#scene-timing) |

### Sizes

A size is an object, a preset name, or `"WxH"` (its id is that string: `"300x600"`).

| Field | Type | Default | Meaning |
|---|---|---|---|
| `id` | string | required | Its name in replies, file names and `at`; letters, digits, `-` and `_` |
| `width`, `height` | px | required | Output size |
| `scale` | number | 1 | Shrinks everything, fonts included, before the layout adapts, like a design tool's Scale tool |
| `safe` | Sides | none | The part a platform covers, such as a story's UI bars; text there is reported as `!unsafe` |

### Presets

| Preset | Size | Safe area |
|---|---|---|
| `instagram-portrait` | 1080×1350 | |
| `instagram-square` | 1080×1080 | |
| `instagram-story` | 1080×1920 | 250 top, 340 bottom |
| `facebook-feed` | 1200×628 | |
| `linkedin-post` | 1200×627 | |
| `x-post` | 1600×900 | |
| `youtube-thumbnail` | 1280×720 | |
| `iab-medium-rectangle` | 300×250 | |
| `iab-leaderboard` | 728×90 | |
| `iab-skyscraper` | 160×600 | |
| `iab-half-page` | 300×600 | |
| `a4-portrait` | 2480×3508 (300 dpi) | |

A preset's id is its name.

### Aspect classes

With r = width ÷ height of the size:

| Class | When | Examples |
|---|---|---|
| `landscape` | r > 1.1 | 1200×628 |
| `square` | 0.9 ≤ r ≤ 1.1 | 1080×1080 |
| `portrait` | r < 0.9 | 1080×1350 |
| `wide` | r ≥ 2 | 728×90 |
| `tall` | r ≤ 0.5 | 300×600, 160×600 |

A size is in one of the first three and may also be `wide` or `tall`.

## Layers

Every layer has a `type`, the [common fields](#common-fields), and its type's own fields.

| `type` | What it is |
|---|---|
| [`frame`](#frame) | A container with free, stack or grid layout |
| [`text`](#text) | Text in a box |
| [`image`](#image) | An image in a box |
| [`video`](#video) | A video clip in a box, playing in a moving scene |
| [`rect`, `ellipse`, `polygon`, `path`, `line`](#shapes) | Shapes |
| [`icon`](#icon) | A named icon |
| [`spacer`](#spacer) | Flexible empty space in a stack |
| [`firstFit`](#firstfit) | Draws the first child that fits |
| [`use`](#use) | Instances of a component |

### Common fields

| Field | Type | Default | Meaning |
|---|---|---|---|
| `id` | string | generated (`text1`, `rect2`, …) | Stable id |
| `role` | string | none | Semantic name; an edit can target every layer with a role |
| `parent` | string | top level | (`layer_add` only) The frame to add the layer into |
| `x`, `y` | px or `"N%"` | 0 | Position in the parent; ignored in a stack or grid |
| `width`, `height` | Length | by type: text and images size themselves, stack and grid frames hug, others are 100 | Size ([Sizing](#sizing)) |
| `minWidth`, `maxWidth`, `minHeight`, `maxHeight` | px | none | Clamps, applied last |
| `aspectRatio` | number | none | Width ÷ height, kept when only one side is set |
| `constraints` | `{h, v}` | `left`, `top` | How the layer follows its parent in free layout ([Free layout](#free-layout)) |
| `place`, `inset` | spot, px or `[x, y]` | none | Pins the layer to a spot of its parent ([Free layout](#free-layout)) |
| `hidden` | boolean | false | Not drawn and takes no space |
| `opacity` | 0–1 | 1 | The layer and its children as one |
| `blendMode` | name | `normal` | One of the [blend modes](#blend-modes) |
| `fills`, `color`, `gradient` | Paint, or a list | by type | [Fills](#fills); `color` and `gradient` are short forms of one fill |
| `strokes`, `stroke` | Stroke, or a list | none | [Strokes](#strokes) |
| `shadows` | Shadow, or a list | none | [Shadows](#shadows) |
| `blur`, `backdropBlur` | px | 0 | [Blur](#blur) |
| `radius` | px, `[tl, tr, br, bl]` or `"full"` | 0 | Corners; `"full"` is a capsule at every size |
| `mask` | Mask | none | [Masks](#masks) |
| `edges` | Edges | none | [Edges](#edges) |
| `rotation` | Degrees | 0 | About the box center |
| `scale`, `offset`, `skew`, `flipX`, `flipY` | number, `[x, y]` px, `[x, y]` Degrees, boolean, boolean | 1, [0, 0], [0, 0], false, false | Visual transforms after layout, about the box center; they never move other layers |
| `style` | string or list | none | [Styles](#styles) applied in order |
| `at` | object | none | Changes for one size or aspect class ([Per size](#per-size)) |
| `in`, `out`, `animate`, `stagger`, `split` | | none | [Motion](#motion) |
| `shot` | Shot | none | Makes a top-level frame a [shot](#shots-and-transitions) |

In a stack, children also take the [stack child fields](#stack-children); in a grid, the [grid child fields](#grid-children).

A layer with no `fills` draws no fill, except text, lines and icons, which are black.

### frame

| Field | Type | Default | Meaning |
|---|---|---|---|
| `children` | list of Layer | none | Its layers, bottom to top |
| `clip` | boolean | true | Clips children to the frame, animated ones included |
| `stack` | Stack | none | Lays the children out as a row or column ([Stacks](#stacks)) |
| `grid` | Grid | none | Lays them out as a grid ([Grids](#grids)) |

With neither `stack` nor `grid`, children are placed freely.

### text

See [Text](#text).

### image

| Field | Type | Default | Meaning |
|---|---|---|---|
| `asset` | string | required | An asset id from `asset_add` |
| `fit` | `fill`, `fit`, `tile` | `fill` | Cover the box (cropping), contain it (letterboxed), or repeat |
| `focus` | Point | [0.5, 0.5] | The point that stays in view when `fill` crops |
| `crop` | `{x, y, width, height}`, 0–1 of the image | none | Show only that part |
| `tileScale` | number | 1 | Tile size for `tile`, × the image's size |
| `adjust` | Adjust | none | [Image adjustments](#image-adjustments) |

SVGs are drawn at their drawn size, so they stay sharp.

### video

A clip added with `asset_add`, drawn like an image and under any layers above it: titles, captions, logos. Decoding it needs ffmpeg ([tools.md](tools.md#ffmpeg)).

```json
{"type": "video", "asset": "beach", "width": "fill", "height": "fill", "start": 2, "speed": 0.5, "audio": false}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `asset` | string | required | A clip from `asset_add` (MP4, MOV, WebM…) |
| `fit`, `focus`, `crop`, `adjust` | | `fill`, center | As for [images](#image), applied to every frame |
| `start` | Seconds | 0 | Where in the clip to begin |
| `delay` | Seconds | 0 | When the clip starts playing in the scene (in a shot, from the shot's start); before, its first frame holds |
| `speed` | number | 1 | Playback speed, 0.01–100: 0.5 is slow motion |
| `loop` | boolean | false | Repeat the clip until the scene ends; otherwise its last frame holds |
| `audio` | boolean | true | Play the clip's own sound in MP4 and WebM output |

A clip's sound plays with its pictures: from `start`, at `speed`, looping with it, and only while its shot is on; several clips' sounds are mixed. Stills (`time`, or a scene at rest) show the clip's frame at that moment.

### Shapes

| `type` | Field | Type | Default | Meaning |
|---|---|---|---|---|
| `rect` | | | | A rectangle; `radius` rounds it |
| `ellipse` | `arc` | `{start, end, inner}` | 0, 360, 0 | Part of the ellipse, Degrees from the top; `inner` is a hole, 0–1 of the radius: a ring |
| `polygon` | `sides` | number ≥ 3 | 3 | A regular polygon in the box |
| | `innerRadius` | 0–1 | none | Makes a star: the inner points' share of the outer radius (0.38 classic, 0.8 starburst) |
| `path` | `d` | string | | SVG path data |
| | `shape` | name | | Or a [named shape](#appendix-names) |
| | `fillRule` | `nonzero`, `evenodd` | `nonzero` | Which regions are inside |
| | `fitPath` | `contain`, `stretch` | `contain` | Scaled evenly and centered, or stretched to the box |
| `line` | `color`, `strokeWidth` | Color, px | black, 1 | From the box's top-left by `width, height` |

A named shape is a real path, so every paint applies: a photo in a blob, a gradient ribbon, a dashed speech bubble.

### icon

| Field | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | required | The icon's name in its set |
| `set` | `lucide`, `solid`, `regular`, `brands` | `lucide` | [Lucide](https://lucide.dev) outline icons, or [Font Awesome Free](https://fontawesome.com) |
| `color` | Color | black | |
| `strokeWidth` | number | 2 | Lucide icons' line width, in the icon's 24-unit grid |

An icon is 24 px tall unless sized.

### spacer

| Field | Type | Default | Meaning |
|---|---|---|---|
| `minLength` | px | 0 | Takes the leftover space in a stack, at least this much |

### firstFit

Draws the first of its `children` that fits its box at this size with nothing wrong inside it: no overflow or truncated text, no text shrunk below its `minFontScale`. When none fits, it draws the last. Typical uses: a long and a short headline, a row CTA and a stacked CTA. `scene_describe` shows what was chosen at each size (`→ short`).

### use

| Field | Type | Default | Meaning |
|---|---|---|---|
| `component` | string | required | The component to place |
| `props` | object | none | Values for the component's `{prop}` placeholders, for every instance |
| `each` | list of objects | none | One instance per entry, in the parent's flow; each entry's values win over `props` |

See [Components](#components).

## Layout

A frame lays out its children in one of three ways: **free** (each child's position and constraints), **stack** (a row or column) or **grid**. Children of a stack or grid ignore `x`, `y` and `constraints` unless they set `position: "absolute"`, which places them like a free child (a badge over a card's corner).

### Sizing

| Value | Meaning | Figma | SwiftUI | CSS |
|---|---|---|---|---|
| `320` | Fixed px (scaled by the size's `scale`) | Fixed | `.frame(width:)` | `320px` |
| `"hug"` | As big as the content | Hug | ideal size | `fit-content` |
| `"fill"` | The free space in a stack; in free layout, the rest of the parent from the layer's position | Fill | `maxWidth: .infinity` | `flex: 1` |
| `"40%"` | Share of the parent: of a stack's content box (inside its padding), or of a free parent's whole box | | `containerRelativeFrame` | `40%` |

In a grid, a child with a px size keeps it and sits at its cell's start; otherwise it fills its cell. Min and max clamps apply last.

### Free layout

| Field | Type | Default | Meaning |
|---|---|---|---|
| `constraints` | `{h: left\|right\|center\|stretch\|scale, v: top\|bottom\|center\|stretch\|scale}` | `left`, `top` | How the layer follows its parent as it resizes, as in Figma |
| `place` | `top-left`, `top`, `top-right`, `left`, `center`, `right`, `bottom-left`, `bottom`, `bottom-right` | none | Pins the layer to that spot of its parent at every size |
| `inset` | px or `[x, y]` | 0 | Distance from the parent's edges for `place` |

Free children use the parent's whole box; its padding doesn't apply.

### Stacks

```json
"stack": {"dir": "row", "gap": 16, "padding": [24, 32], "align": "center", "justify": "between", "wrap": true}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `dir` | `row`, `column`, `row-reverse`, `column-reverse`, or a list | required | Direction; a list is tried in order: `["row", "column"]` is a row where it fits, else a column |
| `gap` | px or `[rowGap, columnGap]` | 0 | Space between children |
| `padding` | Sides | 0 | Space inside the frame's edges |
| `align` | `start`, `center`, `end`, `stretch`, `baseline` | `start` | Across the direction |
| `justify` | `start`, `center`, `end`, `between`, `around`, `evenly` | `start` | Along the direction |
| `wrap` | boolean | false | Wrap onto more lines when they don't fit |

### Stack children

| Field | Type | Default | Meaning |
|---|---|---|---|
| `alignSelf` | as `align` | the stack's `align` | This child's own alignment (`baseline` in a column is `start`) |
| `grow` | number ≥ 0 | 1 | Its share of the free space when it `fill`s |
| `priority` | number | 0 | When a row is too narrow, lower priorities give way first, like SwiftUI's `layoutPriority` |
| `position` | `auto`, `absolute` | `auto` | `absolute` takes it out of the flow and places it like a free child |

### Grids

```json
"grid": {"columns": "2fr 1fr", "rows": "2fr 1fr", "gap": 24, "areas": ["photo side", "cta side"]}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `columns` | tracks, a count, or `{"min": px}` | one `1fr` per `areas` column, else one | CSS tracks (`200px`, `1fr`, `auto`, `25%`, `repeat(3, 1fr)`); `3` is three `1fr`; `{"min": 160}` is as many equal columns as fit at that width or more |
| `rows` | tracks | `auto` | As `columns`; rows beyond them are `auto` |
| `gap` | px or `[rowGap, columnGap]` | 0 | |
| `padding` | Sides | 0 | |
| `areas` | list of strings | none | Named areas, one string per row and a name per column; `.` is empty. Each name must form a rectangle |

A size can rearrange the whole grid by changing only `columns`, `rows` and `areas` in `at`.

### Grid children

| Field | Type | Default | Meaning |
|---|---|---|---|
| `area` | string | none | The named area to fill |
| `cell` | `[row, column]`, from 1 | the next free cell, row by row | Where it starts |
| `span` | `[rows, columns]` | [1, 1] | How many cells it covers |

### Per size

`at` changes a layer's fields for some sizes: `"at": {"sky": {"fontSize": 20}, "tall": {"hidden": true}}`. Its keys are size ids or [aspect classes](#aspect-classes), applied broadest first: `landscape`/`square`/`portrait`, then `wide`/`tall`, then the size id. Its values merge into the layer ([Resolution order](#resolution-order)). `at` can't change a layer's `id`, `type`, `children` or `at`; a key that isn't a size or class is an error.

## Text

### Fields

| Field | Type | Default | Meaning |
|---|---|---|---|
| `text` | string | required | The text, with optional [markup](#inline-markup); `\n` breaks lines |
| `fontSize` | px | 16 | The largest size when the text shrinks to fit |
| `minFontScale` | 0–1 | 0.5 | The smallest it shrinks to, × `fontSize` |
| `resize` | `fixed`, `truncate` | inferred from the box | Overrides [fitting](#fitting): `fixed` keeps the size and lets it overflow; `truncate` keeps it and cuts with an ellipsis |
| `weight` | 100–900, in 100s | 400 | |
| `fontFamily` | string | Inter | Inter (bundled), any [Google Fonts](https://fonts.google.com) family (downloaded on first use), or a font the server loads |
| `color` | Color | black | Text color; `fills` can paint it with a gradient, image or pattern instead |
| `align` | `left`, `center`, `right`, `justify` | `left` | |
| `verticalAlign` | `top`, `center`, `bottom` | `center` for a fixed box, else `top` | Within the box |
| `lineHeight` | number | the font's own line spacing | × the font size |
| `letterSpacing` | px | 0 | |
| `textCase` | `upper`, `lower`, `capitalize` | none | |
| `italic` | boolean | false | |
| `decoration` | `underline`, `strike` | none | |
| `textWrap` | `wrap`, `balance`, `pretty` | `wrap` | `balance` evens line lengths; `pretty` avoids a lone last word |
| `maxLines` | number | none | Lines before the ellipsis |
| `trim` | `cap` | none | Trims the space above cap height and below the baseline, so text centers optically in pills and buttons |
| `padding` | Sides | 0 | Space around the text inside its box |
| `highlight` | Color or `{color, padding, radius, style: box\|brush}` | none | A box behind each line |
| `curve` | px | none | Sets one line on a circular arc of this radius; negative bends down |
| `leader` | string | none | A character that fills each tab's gap: `"Espresso\t$3"` with `leader: "."` draws dot leaders, the price flush right |
| `knockout` | boolean | false | The letters cut through their parent frame's fill, showing what's behind |
| `direction` | `auto`, `ltr`, `rtl` | `auto` | |
| `features` | object | none | OpenType features, e.g. `{"tnum": 1}` |
| `ranges` | list of Range | none | [Ranges](#ranges) |

`strokes` outline the letters (`fills: []` with a stroke makes outlined text) and `shadows` follow their shapes.

### Fitting

The box decides how text fits:

- **Width and height:** the font shrinks until the text fits, down to `minFontScale`, then ends with an ellipsis.
- **Width only:** the text wraps and the box grows down.
- **Neither:** one line, as wide as the text.

When text still doesn't fit, the checks report `!overflow` or `!truncated`; nothing changes silently.

### Inline markup

Models miscount character offsets, so text takes a small HTML subset instead:

```json
{"type": "text", "style": "h1", "text": "Proven <accent>RESULTS</accent> for <accent>WILLOWMERE</accent> Families"}
{"type": "text", "text": "<s>$49</s> <b>$29</b><sup>99</sup> today"}
```

- Tags: `<b>`, `<i>`, `<u>`, `<s>`, `<sup>`, `<sub>`, `<br>`, a style's name as a tag (`<accent>`), and `<span …>` with any [range](#ranges) field as an attribute (`<span color="#D0202E" weight="800" highlight="#FFE600">`). Attribute values can be tokens (`color="$red"`).
- A `<` that doesn't open a known tag is text; `&lt;`, `&gt;`, `&amp;` and `&quot;` are entities.
- A `<span>` whose attributes don't parse is an error, not text.

### Ranges

`ranges` styles parts of the displayed text by character offset; markup is usually easier.

| Field | Type | Meaning |
|---|---|---|
| `start`, `end` | number | Character offsets of the displayed text |
| `color`, `weight`, `italic`, `fontSize`, `fontFamily`, `decoration`, `highlight` | as for text | That part's own values |

## Paint

Frames, shapes, images, text and icons take the same paint fields.

### Fills

`fills` is one paint or a list, bottom to top; `[]` fills nothing (outlined text, for example). Every paint takes `opacity` (0–1, default 1) and `blendMode` (default `normal`).

| Paint | Example |
|---|---|
| Color | `"#D0202E"`, `"#D0202E80"`, a CSS name, or `{"color": "$red", "opacity": 0.5}` |
| Gradient | `{"gradient": {"type": "radial", "stops": ["#0000", "#000C"]}}`, or written flat: `{"type": "linear", "angle": 180, "stops": […]}` |
| Image | `{"image": "photo", "fit": "fill", "focus": [0.5, 0.3], "adjust": {"grayscale": 1}}`, with the [image](#image) fields |
| Pattern | `{"pattern": "dots", "color": "#0002", "size": 12}` |
| Grain | `{"noise": 0.08, "seed": 1}` |

An image fill works on any shape: a photo in a circle is `{"type": "ellipse", "fills": {"image": "photo"}}`.

### Gradients

| Field | Type | Default | Meaning |
|---|---|---|---|
| `type` | `linear`, `radial`, `conic` | `linear` | |
| `stops` | list of Colors, or of `{at, color}` | required | Colors evenly spaced, or at `at` (0–1 or `"55%"`; `offset`, `position` and `pos` also read) |
| `angle` | Degrees | left to right | Linear: CSS angle, 0 = up, 90 = right. Conic: where it starts, from 12 o'clock |
| `from`, `to` | Point | [0, 0.5], [1, 0.5] | Linear, instead of `angle` |
| `center` | Point | [0.5, 0.5] | Radial and conic |
| `radius` | Point | [0.5, 0.5] | Radial: horizontal and vertical radius, 0–1 of the box |

### Patterns and grain

| Paint | Field | Type | Default | Meaning |
|---|---|---|---|---|
| Pattern | `pattern` | `dots`, `stripes`, `grid`, `checker`, `zigzag`, `rays` | required | |
| | `color` | Color | `#00000033` | |
| | `size` | px | 12 | Repeat length |
| | `angle` | Degrees | 0 | Rotation |
| Grain | `noise` | 0–1 | required | Film grain strength |
| | `size` | px | 1 | Grain size |
| | `seed` | number | 0 | Its random pattern |

### Image adjustments

`adjust` on an image, video or image fill:

| Field | Type | Meaning |
|---|---|---|
| `brightness`, `contrast`, `saturate` | −1…1 | |
| `grayscale`, `sepia` | 0…1 | |
| `hue` | Degrees | Hue rotation |
| `duotone` | `[dark, light]` Colors | Maps dark to light |
| `tint` | Color | Recolors every visible pixel, e.g. a logo in white |
| `halftone` | px | Redraws the image as black dots this far apart, larger where it's darker |

### Strokes

`strokes` is one stroke or a list; `"#000"` is a 1 px black stroke.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `width` | px, or `[top, right, bottom, left]` on rects | required | Line width; per side for borders |
| `color`, `gradient` | Color, gradient | black | Its paint |
| `align` | `inside`, `center`, `outside` | `inside` (`center` for lines) | Where it sits on the edge |
| `dash` | `[on, off]` px | solid | |
| `cap` | `butt`, `round`, `square` | `butt` | |
| `join` | `miter`, `round`, `bevel` | `miter` | |
| `start`, `end` | `arrow`, `triangle`, `circle`, `diamond` | none | Markers on lines and paths |
| `rough` | px | 0 | Hand-drawn wobble |
| `seed` | number | 0 | The wobble's random pattern |

### Shadows

`shadows` is one shadow or a list, like CSS `box-shadow`. A shadow follows the layer's shape: it hugs a cutout photo or the letters of a text. A glow is a shadow at `x: 0, y: 0`; a hard offset shadow has `blur: 0`.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `color` | Color | required | |
| `x`, `y` | px | 0 | Offset |
| `blur` | px | 0 | |
| `spread` | px | 0 | Grows the shadow's shape |
| `inset` | boolean | false | An inner shadow |

### Blur

`blur` (px) blurs the layer; `backdropBlur` (px) blurs what's behind it within its shape (frosted glass).

### Masks

| `mask` | Shows |
|---|---|
| a gradient | The layer faded by the gradient's alpha (a photo fading out) |
| `"ellipse"` or a [named shape](#appendix-names) (`"blob-3"`) | The layer inside that shape |
| `{"path": "M…"}` | The layer inside that path |
| `{"layer": "logo"}` | The layer where another layer is; the mask layer isn't drawn itself |
| `{"image": "torn-edge"}` | The layer where an image is opaque |

| Field | Type | Default | Meaning |
|---|---|---|---|
| `mode` | `alpha`, `luminance` | `alpha` | What of the mask counts: its opacity or its brightness |
| `invert` | boolean | false | Reverses the mask |

### Edges

| Field | Type | Default | Meaning |
|---|---|---|---|
| `sides` | list of `top`, `right`, `bottom`, `left` | all four | Which sides of the box tear, like ripped paper |
| `depth` | px | 12 | How far the tears cut in |
| `seed` | number | 0 | The tears' random pattern |

### Blend modes

`normal`, `multiply`, `screen`, `overlay`, `darken`, `lighten`, `color-dodge`, `color-burn`, `hard-light`, `soft-light`, `difference`, `exclusion`, `hue`, `saturation`, `color`, `luminosity`.

## Reuse

### Tokens

`tokens` holds named values; any field takes `"$name"` (a letter or `_`, then letters, digits, `_`, `.` and `-`). The server remembers which fields came from which token, so changing a token in `layer_update` changes every field bound to it; setting such a field to a value of its own unbinds it. `text` is a token only when it is wholly the name of one (`"text": "$headline"`, the way a template names its variables); other text is never scanned, so `"$29"` stays text. In markup, attribute values can be tokens. An unknown token elsewhere is an error that lists the tokens there are.

### Styles

`styles` hold any layer fields: a `card` style can carry fills, radius and shadows. A layer's `style` takes one name or a list; a later style wins where they overlap, and the layer's own fields win over all of them. Changing a style through `layer_update` changes every layer that uses it.

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
  {"name": "Omar Haddad", "office": "Council"}]}
```

- `{prop}` in any string of a component is filled from the instance's props; a string that is only `{prop}` takes the value as is (a number stays a number).
- The `use` layer's own fields (width, constraints, `at`…) apply to each instance's root.
- Instances stay linked: changing the component changes every instance. Their layers are named by the `use` id, the instance number and the inner layer's id or role, e.g. `c.1.name`, in replies. To change one, target the component (`{"component": "candidate", "role": "name"}`), or `detach` the `use` layer into plain layers.
- Components may place other components, up to 8 levels deep.

## Motion

A scene with a `duration`, or made of [shots](#shots-and-transitions), moves. Only fields that don't change layout animate, so the layout is the same at every moment and every check holds throughout. A scene without motion fields is drawn at rest. Output formats are in [tools.md](tools.md#output-formats).

### Scene timing

| Field | Type | Default | Meaning |
|---|---|---|---|
| `duration` | Seconds, 0.001–86,400 | none (a still), or where the last shot ends | Length; its presence makes the scene move |
| `fps` | 1–120 | 30 | Frames per second |
| `loop` | boolean | false | The animation repeats forever |

### Enter and exit

```json
{"type": "text", "text": "…", "in": "fade-up"}
{"type": "frame", "in": {"effect": "pop", "at": 1.2, "ease": "back.out"}, "out": {"effect": "fade", "at": 7}}
```

`in` and `out` take an effect name, or an object. A layer is hidden before its `in` and gone after its `out`.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `effect` | name | required | `fade`, `fade-up`, `fade-down`, `fade-left`, `fade-right` (fade while moving `distance` into place), `pop` (grow from 0.6 with an overshoot), `zoom-in` (grow from 0.85), `zoom-out` (shrink from 1.15), `blur-in` (sharpen from a 12 px blur) |
| `at` | Seconds | `in`: 0; `out`: so it ends with the scene | Start |
| `duration` | Seconds | 0.6 | |
| `ease` | Ease | `power2.out` entering (`back.out` for `pop`), `power2.in` leaving | |
| `distance` | px | 40 | How far a directional fade travels |

### Keyframes

`animate` sets values over time, GSAP-style: one track or a list of them.

```json
"animate": {"scale": [1, 1.06, 1], "duration": 1.6, "repeat": -1, "ease": "sine.inOut"}
"animate": {"rotation": {"from": "random(-90, 90)"}, "offset": {"from": [0, -80]}, "duration": 0.8, "ease": "back.out"}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `opacity`, `scale`, `rotation`, `blur` | list of numbers, or `{from, to}` | | Values spread over `duration`; a missing end is the layer's own value |
| `offset`, `skew` | the same, with `[x, y]` pairs | | |
| `color` | the same, with Colors | | The layer's own color |
| `times` | list of 0–1 | evenly spaced | Where each listed value falls, of `duration` |
| `at` | Seconds | 0 | Start |
| `duration` | Seconds | 1 | One play |
| `ease` | Ease | `power1.inOut` | Between each pair of values |
| `repeat` | number | 0 | Extra plays; −1 repeats to the end |
| `yoyo` | boolean | false | Every other play runs backwards |

A number may be `"random(lo, hi)"`, as in GSAP: each target (each layer, or each piece of split text) gets its own value, seeded from its id, the same on every render. Values hold before a track starts and after it ends. Layout fields (`width`, `fontSize`, `text`, `padding`…) can't animate; to make something grow, animate `scale`.

### Easing

GSAP's names: `none`, `power1` … `power4`, `sine`, `expo`, `circ`, `back`, `elastic`, `bounce`, each with `.in`, `.out` or `.inOut` (a family alone is `.out`), and `steps(n)`. CSS's `ease`, `ease-in`, `ease-out` and `ease-in-out`, and `smooth`, `snappy` and `bouncy`, read as the nearest of those.

### Stagger and split

| Field | Type | On | Meaning |
|---|---|---|---|
| `stagger` | Seconds | a frame or `use` layer | Gives its `in` to its children or instances one after another, this far apart, instead of entering whole |
| `split` | `chars`, `words` | a text layer | Its `in`, `out`, `animate` and `stagger` apply to each letter or word, like GSAP's SplitText. The text is laid out once; each piece moves as a rigid part of it |

```json
{"type": "text", "text": "Animate Anything", "fontSize": 96, "split": "chars", "stagger": 0.05,
 "animate": {"offset": {"from": ["random(-400, 400)", "random(-250, 250)"]}, "rotation": {"from": "random(-180, 180)"},
             "opacity": {"from": 0}, "duration": 1.1, "ease": "back.out"}}
```

Frames clip their children, animated ones included: give a frame `clip: false` when its children move beyond its edges.

### Shots and transitions

A top-level frame with `shot` is a shot: shots play one after another instead of stacking. Times inside a shot (`in`, `animate`, a clip's `delay`) count from the shot's own start. Layers that aren't shots, such as a logo or a caption bar, stay on across all of them. At rest, the first shot shows.

```json
{"id": "s1", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 3}, "children": ["…"]}
{"id": "s2", "type": "frame", "width": "fill", "height": "fill", "shot": {"duration": 3, "transition": "push-left"}, "children": ["…"]}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `duration` | Seconds | required | On screen, including its transitions |
| `transition` | name, or `{type, duration, ease}` | `cut` | How it enters from the shot before |

| Transition field | Type | Default | Meaning |
|---|---|---|---|
| `type` | name | required | `cut`, `fade`, `slide-left`, `slide-right`, `slide-up`, `slide-down` (slides in over the last shot), `push-left` … `push-down` (pushes the last shot out), `wipe-left` … `wipe-down` (a moving edge reveals it), `zoom` (the last shot grows as the new one fades in) |
| `duration` | Seconds | 0.5 | Overlaps the two shots; a cut has none |
| `ease` | Ease | `power2.inOut` | |

Each shot starts where the one before ends minus its transition. The scene's length is where the last shot ends unless `duration` says otherwise. A transition can't be longer than either shot it joins.

## Template files

A template is a scene file that `scene_create` loads by URL or path ([tools.md](tools.md#templates-and-variants)). It has a scene's fields, and its `assets` name files instead of hashes: a URL, or a path relative to the template. Its `tokens` are its variables.

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

A scene keyline saved (its `assets` by `sha256`) loads as a template too.

## Validation and limits

A change that breaks any of these rules is refused whole, with a one-line error ([tools.md](tools.md#errors)); what the layout does at each size (overflow, clipping, contrast) is never refused, but reported as [problems](tools.md#problem-lines).

- Unknown fields, in layers and in every object inside them.
- Values of the wrong type or out of range: opacity 0–1, weight 100–900 in 100s, `minFontScale` above 0 and at most 1, `grow` ≥ 0, polygon `sides` ≥ 3, video `speed` 0.01–100, sizes at least 1 px with `scale` above 0.
- An unknown token, style, component, asset, parent frame or `at` key; a duplicate layer id.
- Shots that aren't top-level frames, have no positive `duration`, or whose transition is longer than a shot it joins; `split` on anything but text.

| Limit | Value |
|---|---|
| Grid tracks per axis, `repeat` count, `cell` and `span` values | 100 |
| Component nesting | 8 levels |
| `fps` | 1–120 |
| `duration` | 0.001–86,400 s |

Asset and file limits are in [tools.md](tools.md#limits).

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

## Appendix: names

| Kind | Names |
|---|---|
| Named shapes (`path` `shape`, masks) | `ribbon`, `ribbon-banner`, `bubble`, `bubble-round`, `arrow`, `arrow-curved`, `chevron`, `tag`, `arch`, `shield`, `heart`, `cloud`, `wave`, `burst`, `blob-1` … `blob-6`, `brush-stroke` |
| Icons | About 5,000: [Lucide](https://lucide.dev) (`lucide`, about 2,100) and [Font Awesome Free](https://fontawesome.com) (`solid` about 2,000, `regular` about 270, `brands` about 610) |
| Patterns | `dots`, `stripes`, `grid`, `checker`, `zigzag`, `rays` |
| Enter and exit effects | `fade`, `fade-up`, `fade-down`, `fade-left`, `fade-right`, `pop`, `zoom-in`, `zoom-out`, `blur-in` |
| Transitions | `cut`, `fade`, `slide-*`, `push-*`, `wipe-*` (each `left`, `right`, `up`, `down`), `zoom` |
| Eases | `none`, `power1`–`power4`, `sine`, `expo`, `circ`, `back`, `elastic`, `bounce` (`.in`, `.out`, `.inOut`), `steps(n)` |
| Blend modes | See [Blend modes](#blend-modes) |
