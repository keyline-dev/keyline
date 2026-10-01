# Scene format

A scene is one JSON document: a master size, the sizes it renders at, its assets, shared tokens, styles and components, and a tree of layers. This page is the reference for every field. The ideas behind it are in [concepts.md](concepts.md), and the tools that write and render it in [tools.md](tools.md).

```json
{
  "sizes": ["instagram-square", "iab-skyscraper"],
  "tokens": {"brand": "#D0202E"},
  "layers": [
    {"type": "rect", "width": "fill", "height": "fill", "fill": "#FFF4E0"},
    {"type": "text", "text": "Cold Brew <b>Season</b>", "fontSize": 96, "fontWeight": 800,
     "color": "{{brand}}", "width": "80%", "place": "center", "media": {"tall": {"fontSize": 64}}}
  ]
}
```

**Contents:** [Conventions](#conventions) · [Document](#document) · [Layers](#layers) · [Layout](#layout) · [Text](#text) · [Paint](#paint) · [Reuse](#reuse) · [Motion](#motion) · [Template files](#template-files) · [Validation and limits](#validation-and-limits) · [Example](#example) · [Names](#appendix-names)

## Conventions

Field names and values follow what models already know:

1. **CSS names and values wherever CSS has the concept**, camelCase as in React Native: `fontWeight`, `textAlign`, `justifyContent`, `alignItems`, `gap`, `padding`, `borderRadius`, `filter`. **Figma words for sizing, pinning and clipping** (`hug`, `fill`, `constraints`, `clipsContent`), **SVG** for shapes and strokes (`fill`, `stroke`, `markerEnd`), **GSAP** for motion, **HTML media** for video, and **SwiftUI** only where none of those has the concept (`firstFit`, `layoutPriority`, `minimumScaleFactor`).
2. **CSS names, keyline's own layout.** The names and enum values are CSS's, and colors are any CSS color (`rgba(0,0,0,.25)`, `hsl(…)`). The layout model is keyline's, documented on this page: close to flexbox and grid where it borrows from them, but not browser-exact.
3. **Short forms.** A compound field has a one-value short form: `fill: "#fff"`, `padding: 24`, `borderRadius: 12`, `stroke: "#000"`.
4. **Every field has a default, and defaults are omitted** in what the agent sends and in what the server stores.
5. **Unknown fields are rejected**, with the nearest known name suggested.

### Value types

The tables below use these types.

| Type | Values |
|---|---|
| px | A number of pixels at the master size; each size's `scale` scales it |
| Length | px, `"hug"` (as big as the content), `"fill"` (the free space), or `"40%"` of the parent ([Sizing](#sizing)) |
| Color | Any CSS color: `#RGB`, `#RRGGBBAA`, `rgb()`, `rgba()`, `hsl()` or a name |
| Paint | A Color, or a gradient, image, pattern or grain object ([Fills](#fills)) |
| Sides | px for all four, `[vertical, horizontal]`, `[top, horizontal, bottom]` or `[top, right, bottom, left]`, as in CSS |
| Point | `[x, y]`, each 0–1 of the box, from its top-left |
| Seconds | A number of seconds |
| Degrees | A number of degrees, clockwise |
| Ease | An easing name ([Easing](#easing)) |
| Token | `{{name}}` as a whole value or inside text ([Tokens](#tokens)) |

### Resolution order

A layer's final values are built in this order; each step wins over the one before:

1. **Tokens**, when the layer is written: every `{{name}}` becomes the token's value.
2. **Components**, for a `use` layer: `{{prop}}` is filled from the instance's props (a prop wins over a token of the same name; the component's other `{{name}}`s are tokens), and the `use` layer's own fields replace the component root's, field by field.
3. **Styles**, in the order listed; a later style wins. The layer's own fields win over all styles.
4. **`media`**, per size: aspect classes broadest first, then the size id.

`media` and `layer_update`'s `set` merge like a JSON merge patch: nested objects (an object `enter`, `constraints`) merge field by field, while lists (`fill`, `ranges`, `children`) and plain values are replaced whole, and `null` resets a field. Styles and a `use` layer's fields replace whole top-level fields, except `media`: a style's `media` and the layer's own merge, size by size and field by field, the layer's winning.

## Document

| Field | Type | Default | Meaning |
|---|---|---|---|
| `sizes` | list of Size | required | The sizes to render ([Sizes](#sizes)) |
| `width`, `height` | px | the first size's | The master size: the layers are written at this size |
| `background` | Color | `#FFFFFF` | Canvas color |
| `tokens` | object | none | Named values, used as `{{name}}` ([Tokens](#tokens)) |
| `styles` | object | none | Named sets of layer fields ([Styles](#styles)) |
| `components` | object | none | Named layer trees ([Components](#components)) |
| `assets` | object | none | Images, SVGs, video clips and sounds added by `asset_add`, by id: `{sha256, width, height}` (a sound's width and height are 0) |
| `layers` | list of Layer | none | The layer tree, bottom to top |
| `duration`, `fps`, `loop` | | a still | [Scene timing](#scene-timing) |
| `audio` | Soundtrack | none | Music under the video ([Soundtrack](#soundtrack)) |

### Sizes

A size is an object, a preset name, or `"WxH"` (its id is that string: `"300x600"`).

| Field | Type | Default | Meaning |
|---|---|---|---|
| `id` | string | required | Its name in replies, file names and `media`; letters, digits, `-` and `_` |
| `width`, `height` | px | required | Output size |
| `scale` | number | 1 | Shrinks everything, fonts included, before the layout adapts, like a design tool's Scale tool |
| `safeArea` | Sides | none | The part a platform covers, such as a story's UI bars; text there is reported as `!unsafe` |

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
| `a4-portrait` | 2480×3508 (300 dpi) | A PDF of it is an A4 page. Give it a `scale` for a screen-sized master (2.3 for a 1080 px wide one) |

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
| `width`, `height` | Length | by type: text and images size themselves, frames wrap their children, others (and empty frames) are 100 | Size ([Sizing](#sizing)) |
| `minWidth`, `maxWidth`, `minHeight`, `maxHeight` | px | none | Clamps on the size; a placed layer is placed by its clamped size |
| `aspectRatio` | number | none | Width ÷ height, kept when only one side is set |
| `constraints` | `{horizontal, vertical}` | `left`, `top` | How the layer follows its parent in free layout ([Free layout](#free-layout)) |
| `place` | spot | none | Pins the layer to a spot of its parent ([Free layout](#free-layout)) |
| `margin` | px or `[x, y]` | 0 | With `place`, its distance from the parent's edges ([Free layout](#free-layout)) |
| `hidden` | boolean | false | Not drawn and takes no space |
| `opacity` | 0–1 | 1 | The layer and its children as one |
| `blendMode` | name | `normal` | One of the [blend modes](#blend-modes) |
| `fill` | Paint, or a list | none (text, lines and icons: black) | [Fills](#fills) |
| `stroke` | Stroke, or a list | none | [Strokes](#strokes) |
| `shadow` | Shadow, or a list | none | [Shadows](#shadows) |
| `blur`, `backdropBlur` | px | 0 | [Blur](#blur) |
| `borderRadius` | px, `[tl, tr, br, bl]` or `"full"` | 0 | Corners; `"full"` is a capsule at every size |
| `mask` | Mask | none | [Masks](#masks) |
| `edges` | Edges | none | [Edges](#edges) |
| `rotate` | Degrees | 0 | About the box center |
| `scale`, `translate`, `skew`, `flipX`, `flipY` | number, `[x, y]` px, `[x, y]` Degrees, boolean, boolean | 1, [0, 0], [0, 0], false, false | Visual transforms after layout, about the box center; they never move other layers |
| `style` | string or list | none | [Styles](#styles) applied in order |
| `media` | object | none | Changes for one size or aspect class ([Per size](#per-size)) |
| `enter`, `exit`, `animate`, `stagger`, `split` | | none | [Motion](#motion) |

In a stack, children also take the [stack child fields](#stack-children); in a grid, the [grid child fields](#grid-children).

### frame

| Field | Type | Default | Meaning |
|---|---|---|---|
| `children` | list of Layer | none | Its layers, bottom to top |
| `clipsContent` | boolean | true | Clips children to the frame, animated ones included, as in Figma; false lets them show outside it (CSS's `overflow` reads as this) |
| `flexDirection`, `justifyContent`, `alignItems`, `flexWrap`, `gap`, `padding` | | | A row or column ([Stacks](#stacks)) |
| `gridTemplateColumns`, `gridTemplateRows`, `gridTemplateAreas`, `gap`, `padding` | | | A grid ([Grids](#grids)) |

With neither `flexDirection` nor a grid template, children are placed freely, by their own position.

`{"type": "shot", "duration": 3, "transition": "fade", …}` is a frame that plays in turn with the other shots ([Shots and transitions](#shots-and-transitions)).

### text

See [Text](#text).

### image

| Field | Type | Default | Meaning |
|---|---|---|---|
| `asset` | string | required | An asset id from `asset_add` |
| `fit` | `cover`, `contain`, `fill`, `tile` | `cover` | CSS `object-fit`: cover the box (cropping), contain it (letterboxed), stretch to it; or repeat |
| `focus` | Point | [0.5, 0.5] | The point that stays in view when `cover` crops |
| `crop` | `{x, y, width, height}`, 0–1 of the image | none | Show only that part |
| `tileScale` | number | 1 | Tile size for `tile`, × the image's size |
| `filter` | Filter | none | [Filters](#filters) |

SVGs are drawn at their drawn size, so they stay sharp.

### video

A clip added with `asset_add`, drawn like an image and under any layers above it: titles, captions, logos. Decoding it needs ffmpeg ([tools.md](tools.md#ffmpeg)).

```json
{"type": "video", "asset": "beach", "width": "fill", "height": "fill", "trimStart": 2, "playbackRate": 0.5, "muted": true}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `asset` | string | required | A clip from `asset_add` (MP4, MOV, WebM…) |
| `fit`, `focus`, `crop`, `filter` | | `cover`, center | As for [images](#image), applied to every frame |
| `trimStart` | Seconds | 0 | Where in the clip to begin |
| `delay` | Seconds | 0 | When the clip starts playing in the scene (in a shot, from the shot's start); before, its first frame holds |
| `playbackRate` | number | 1 | 0.01–100: 0.5 is slow motion |
| `loop` | boolean | false | Repeat the clip until the scene ends; otherwise its last frame holds |
| `muted` | boolean | false | Leaves the clip's own sound out of MP4 and WebM output |

A clip's sound plays with its pictures: from `trimStart`, at `playbackRate`, looping with it, and only while its shot is on; several clips' sounds are mixed. Stills (`time`, or a scene at rest) show the clip's frame at that moment.

### Shapes

| `type` | Field | Type | Default | Meaning |
|---|---|---|---|---|
| `rect` | | | | A rectangle; `borderRadius` rounds it |
| `ellipse` | `arc` | `{start, end, inner}` | 0, 360, 0 | Part of the ellipse, Degrees from the top; `inner` is a hole, 0–1 of the radius: a ring. Filled, it's a wedge; with only a stroke, an open arc (a progress ring) |
| `polygon` | `sides` | number ≥ 3 | 3 | A regular polygon in the box |
| | `innerRadius` | 0–1 | none | Makes a star: the inner points' share of the outer radius (0.38 classic, 0.8 starburst) |
| `path` | `d` | string | | SVG path data |
| | `shape` | name | | Or a [named shape](#appendix-names) |
| | `fillRule` | `nonzero`, `evenodd` | `nonzero` | Which regions are inside |
| | `fit` | `contain`, `fill` | `contain` | Scaled evenly and centered, or stretched to the box |
| `line` | | | | From the box's top-left by `width, height`, drawn by its `stroke` (black, 1 px); `color` and `strokeWidth` read as its stroke |

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
| `minLength` | px | 0 | Takes the leftover space in a stack, at least this much; `minHeight` or `minWidth` reads as this |

### firstFit

Draws the first of its `children` that fits its box at this size with nothing wrong anywhere inside it: no text overflowing, truncated or shrunk to fit, no stack squeezed. When none fits, it draws the last. Typical uses: a long and a short headline, a row CTA and a stacked CTA. `scene_describe` shows what was chosen at each size (`→ short`).

### use

| Field | Type | Default | Meaning |
|---|---|---|---|
| `component` | string | required | The component to place |
| `props` | object | none | Values for the component's `{{prop}}` placeholders, for every instance |
| `each` | list of objects | none | One instance per entry, in the parent's flow; each entry's values win over `props` |

See [Components](#components).

## Layout

A frame lays out its children in one of three ways: **free** (each child's position and constraints), **stack** (a row or column, CSS flexbox) or **grid** (CSS grid). Children of a stack or grid ignore `x`, `y` and `constraints` unless they set `position: "absolute"`, which places them like a free child (a badge over a card's corner).

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
| `constraints` | `{horizontal: left\|right\|center\|stretch\|scale, vertical: top\|bottom\|center\|stretch\|scale}` | `left`, `top` | How the layer follows its parent as it resizes, as in Figma |
| `place` | `top-left`, `top`, `top-right`, `left`, `center`, `right`, `bottom-left`, `bottom`, `bottom-right` | none | Pins the layer to that spot of its parent at every size |
| `margin` | px or `[x, y]` | 0 | Distance from the parent's edges for `place`; a placed `fill` size stops at it on both sides |

Free children use the parent's whole box; its padding doesn't apply. A `"N%"` `x` or `y` is that share of the parent at every size and moves the layer without resizing it. A free frame sized by a stack with `fill` or `%` places its children on the box the stack gave it.

### Stacks

A frame with `flexDirection` is a stack, like CSS flexbox (`display: "flex"` alone makes a row, as in CSS):

```json
{"type": "frame", "flexDirection": "row", "gap": 16, "padding": [24, 32], "alignItems": "center", "justifyContent": "space-between", "children": ["…"]}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `flexDirection` | `row`, `column`, `row-reverse`, `column-reverse`, or a list | required | Direction; a list is tried in order: `["row", "column"]` is a row where it fits, else a column |
| `gap` | px or `[rowGap, columnGap]` | 0 | Space between children |
| `padding` | Sides | 0 | Space inside the frame's edges |
| `alignItems` | `stretch`, `flex-start`, `center`, `flex-end`, `baseline` | `stretch` | Across the direction; `stretch` fills the cross axis unless a child has a size there (with `flexWrap`, each line's height, as in CSS) |
| `justifyContent` | `flex-start`, `center`, `flex-end`, `space-between`, `space-around`, `space-evenly` | `flex-start` | Along the direction |
| `flexWrap` | `nowrap`, `wrap` | `nowrap` | Wrap onto more lines when they don't fit |

`padding`, `gap`, `justifyContent`, `alignItems` and `flexWrap` on a frame without `flexDirection` or a grid template are an error that says so.

As in CSS, text and frames in a column never shrink below their content's height; a stack whose children don't fit even then is reported as `!overflow needs W×H`, the size it needs.

### Stack children

| Field | Type | Default | Meaning |
|---|---|---|---|
| `alignSelf` | as `alignItems` | the stack's `alignItems` | This child's own alignment (`baseline` in a column is `flex-start`) |
| `flexGrow` | number ≥ 0 | 1 for `fill`, else none | Its share of the free space when it `fill`s; above 0, it makes a child fill from its size along the stack, as CSS's flex-basis (from nothing when it has none: `width: 0, flexGrow: 1` shares a row evenly) |
| `layoutPriority` | number | 0 | When a row is too narrow, lower priorities give way first, as in SwiftUI; `"low"` and `"high"` read as −1 and 1, and CSS `flexShrink: 0` as 1 |
| `position` | `auto`, `absolute` | `auto` | `absolute` takes it out of the flow and places it like a free child |

### Grids

A frame with a grid template is a grid, like CSS grid:

```json
{"type": "frame", "gridTemplateColumns": "2fr 1fr", "gridTemplateRows": "2fr 1fr", "gridTemplateAreas": ["photo side", "cta side"], "gap": 24, "children": ["…"]}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `gridTemplateColumns` | CSS tracks | one `1fr` per area column, else one | `200px`, `1fr`, `auto`, `25%`, `repeat(3, 1fr)`; `repeat(auto-fill, minmax(160px, 1fr))` is as many equal columns as fit at 160 px or more |
| `gridTemplateRows` | CSS tracks | `auto` | As columns; rows beyond them are `auto` |
| `gridTemplateAreas` | list of strings | none | Named areas, one string per row and a name per column; `.` is empty. Each name must form a rectangle |
| `gap` | px or `[rowGap, columnGap]` | 0 | |
| `padding` | Sides | 0 | |

A size can rearrange the whole grid by changing only its templates in `media`.

### Grid children

| Field | Type | Default | Meaning |
|---|---|---|---|
| `gridArea` | string | none | The named area to fill |
| `gridRow`, `gridColumn` | `2`, `"1 / span 2"`, `"1 / 3"` or `"span 2"`, from 1 | the next free cell, row by row, one track | Where it starts and how far it spans; given one, it takes the first free cell in that row or column. An item spanning `auto` tracks grows them evenly to fit it, as in CSS |

### Per size

`media` changes a layer's fields for some sizes: `"media": {"sky": {"fontSize": 20}, "tall": {"hidden": true}}`. Its keys are size ids or [aspect classes](#aspect-classes), applied broadest first: `landscape`/`square`/`portrait`, then `wide`/`tall`, then the size id. Its values merge into the layer ([Resolution order](#resolution-order)). `media` can't change a layer's `id`, `type`, `children`, `media` or `style`; a key that isn't a size or class is an error.

## Text

### Fields

| Field | Type | Default | Meaning |
|---|---|---|---|
| `text` | string | required | The text, with optional [markup](#inline-markup); `\n` breaks lines |
| `fontSize` | px | 16 | The largest size when the text shrinks to fit |
| `minimumScaleFactor` | 0–1 | 0.5 | The smallest it shrinks to, × `fontSize`; 1 keeps its size and cuts it with an ellipsis |
| `fontWeight` | 100–900 in 100s, or `"bold"` | 400 | |
| `fontFamily` | string | Inter | Inter (bundled), any [Google Fonts](https://fonts.google.com) family (downloaded on first use), or a font the server loads |
| `color` | Color | black | Text color; `fill` can paint it with a gradient, image or pattern instead |
| `textAlign` | `left`, `center`, `right`, `justify` | `left` | |
| `textAlignVertical` | `top`, `center` (or `middle`), `bottom` | `center` in a box with a height, else `top` | Within the box |
| `lineHeight` | number | the font's own line spacing | × `fontSize`; a value above 4 is an error (px was likely meant) |
| `letterSpacing` | px | 0 | |
| `textTransform` | `uppercase`, `lowercase`, `capitalize` | none | |
| `fontStyle` | `normal`, `italic` | `normal` | The family's italic face (Google Fonts families come with theirs); one without an italic is drawn slanted |
| `textDecoration` | `underline`, `line-through` | none | |
| `textWrap` | `wrap`, `balance`, `pretty` | `wrap` | `balance` evens line lengths; `pretty` avoids a lone last word |
| `maxLines` | number | none | Lines before the ellipsis, in any box; cut text is reported `!truncated` |
| `trim` | `cap` | none | Trims the space above cap height and below the baseline, so text centers optically in pills and buttons |
| `padding` | Sides | 0 | Space around the text inside its box |
| `highlight` | Color or `{color, padding, borderRadius, shape: box\|brush}` | none | A box behind each line; CSS `background-color` on text reads as this. `padding` is one number, px (4): that much on the sides, half above and below |
| `curve` | px | none | Sets one line on a circular arc of this radius; negative bends down |
| `leader` | string | none | A character that fills each tab's gap: `"Espresso\t$3"` with `leader: "."` draws dot leaders, the price flush right. Each side keeps its markup, on one baseline; the letters take `color` (not `fill`, strokes or `knockout`) |
| `knockout` | boolean | false | The letters cut through their parent frame's fill, showing what's behind |
| `direction` | `auto`, `ltr`, `rtl` | `auto` | |
| `features` | object | none | OpenType features, e.g. `{"tnum": 1}` |
| `ranges` | list of Range | none | [Ranges](#ranges) |

`stroke` outlines the letters (`fill: []` with a stroke makes outlined text) and `shadow` follows their shapes.

### Fitting

The box decides how text fits:

- **Width and height:** the font shrinks until the text fits, down to `minimumScaleFactor`, then ends with an ellipsis.
- **Width only:** the text wraps and the box grows down. In a column, `alignItems: stretch` gives text the column's width, so it wraps.
- **Neither:** one line, as wide as the text.

Text that is cut is reported as `!truncated needs W×H`, with the box it needs; nothing changes silently.

### Inline markup

Models miscount character offsets, so text takes a small HTML subset instead:

```json
{"type": "text", "style": "h1", "text": "Proven <accent>RESULTS</accent> for <accent>WILLOWMERE</accent> Families"}
{"type": "text", "text": "<s>$49</s> <b>$29</b><sup>99</sup> today"}
```

- Tags: `<b>` (or `<strong>`), `<i>` (or `<em>`), `<u>`, `<s>`, `<sup>`, `<sub>`, `<br>`, a style's name as a tag (`<accent>`), and `<span style="…">` with CSS: `color`, `font-weight`, `font-style`, `font-size`, `font-family`, `text-decoration`, `background-color` (a highlight). Values can be tokens (`style="color:{{red}}"`). A style's name as a tag carries the style's `color`, `fontWeight`, `fontStyle`, `fontSize`, `fontFamily`, `textDecoration` and `highlight` (a brush too).
- A `<` that doesn't open a known tag is text; `&lt;`, `&gt;`, `&amp;` and `&quot;` are entities.
- A `<span>` whose style doesn't parse is an error, not text.

### Ranges

`ranges` styles parts of the displayed text by character offset; markup is usually easier.

| Field | Type | Meaning |
|---|---|---|
| `start`, `end` | number | Character offsets of the displayed text |
| `color`, `fontWeight`, `fontStyle`, `fontSize`, `fontFamily`, `textDecoration`, `highlight` | as for text | That part's own values |

## Paint

Frames, shapes, images, text and icons take the same paint fields.

### Fills

`fill` is one paint or a list, bottom to top; `[]` fills nothing (outlined text, for example). Every paint takes `opacity` (0–1, default 1) and `blendMode` (default `normal`).

| Paint | Example |
|---|---|
| Color | `"#D0202E"`, `"rgba(208, 32, 46, 0.5)"`, a CSS name, or `{"color": "{{red}}", "opacity": 0.5}` |
| Gradient | `{"gradient": {"type": "radial", "stops": ["#0000", "#000C"]}}`, or written flat: `{"type": "linear", "angle": 180, "stops": […]}`, or as a CSS string: `"linear-gradient(180deg, #fff 0%, #fff0 100%)"` (`radial-gradient` too, centered) |
| Image | `{"image": "photo", "fit": "cover", "focus": [0.5, 0.3], "filter": {"grayscale": 1}}`, with the [image](#image) fields |
| Pattern | `{"pattern": "dots", "color": "#0002", "size": 12}` |
| Grain | `{"noise": 0.08, "seed": 1}` |

An image fill works on any shape: a photo in a circle is `{"type": "ellipse", "fill": {"image": "photo"}}`.

### Gradients

| Field | Type | Default | Meaning |
|---|---|---|---|
| `type` | `linear`, `radial`, `conic` | `linear` | |
| `stops` | list of Colors, or of `{offset, color}` | required | Colors evenly spaced, or at `offset` (0–1 or `"55%"`) |
| `angle` | Degrees | 180 (top to bottom, as in CSS) | Linear: CSS angle, 0 = up, 90 = right. Conic: where it starts, from 12 o'clock |
| `from`, `to` | Point | [0.5, 0], [0.5, 1] | Linear, instead of `angle` |
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

`rays` are hard-edged sectors, so over a photo they cut across its detail and read much stronger than over a flat color: keep them to about 3–4% there (`"color": "#FFFFFF0A"` with `blendMode: "screen"`), or put them on the flat areas.

### Filters

`filter` on an image, video or image fill: CSS `filter` functions on their CSS scales:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `brightness`, `contrast`, `saturate` | number ≥ 0 | 1 | 1 leaves it unchanged: `brightness: 0.8` darkens, `1.2` lightens |
| `grayscale`, `sepia` | 0–1 | 0 | |
| `hueRotate` | Degrees | 0 | |
| `duotone` | `[dark, light]` Colors | none | Maps dark to light |
| `tint` | Color | none | Recolors every visible pixel, e.g. a logo in white |
| `halftone` | px | 0 | Redraws the image as black dots this far apart, larger where it's darker; over a dark background they barely show |

### Strokes

`stroke` is one stroke or a list, SVG-style; `"#000"` is a 1 px black stroke.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `width` | px, or `[top, right, bottom, left]` on rects | 1 | Line width; per side for borders |
| `color` | Color, or `{"gradient": …}` | black | Its paint |
| `align` | `inside`, `center`, `outside` | `inside` (`center` for lines) | Where it sits on the edge |
| `dash` | `[on, off]` px | solid | |
| `cap` | `butt`, `round`, `square` | `butt` | |
| `join` | `miter`, `round`, `bevel` | `miter` | |
| `markerStart`, `markerEnd` | `arrow`, `triangle`, `circle`, `diamond` | none | Markers on lines and paths |
| `roughness` | px | 0 | Hand-drawn wobble |
| `seed` | number | 0 | The wobble's random pattern |

### Shadows

`shadow` is one shadow or a list, like CSS `box-shadow`. A shadow follows the layer's shape: it hugs a cutout photo or the letters of a text. A glow is a shadow at `x: 0, y: 0`; a hard offset shadow has `blur: 0`.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `color` | Color | required | |
| `x`, `y` | px | 0 | Offset |
| `blur` | px | 0 | Blur radius, as in CSS |
| `spread` | px | 0 | Grows the shadow's shape |
| `inset` | boolean | false | An inner shadow |

### Blur

`blur` (px) blurs the layer; `backdropBlur` (px) blurs what's behind it within its shape (frosted glass).

### Masks

| `mask` | Shows |
|---|---|
| a gradient, flat (`{"angle": 180, "stops": ["#000", "#0000"]}`), as `{"gradient": …}`, or a CSS `linear-gradient(…)` | The layer faded by the gradient's alpha over its box (a photo fading out); nothing outside the box shows |
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

`tokens` holds named values, used as `{{name}}` (a letter or `_`, then letters, digits, `_`, `.` and `-`), as in Mustache and Handlebars:

```json
"tokens": {"brand": "#D0202E", "h1": 64, "name": "Mia", "photo": "cat"}
{"type": "text", "text": "Meet {{name}}, 7 months old", "fontSize": "{{h1}}", "color": "{{brand}}"}
{"type": "image", "asset": "{{photo}}"}
{"type": "text", "text": "Proven <span style=\"color:{{brand}}\">RESULTS</span>"}
```

- A string that is only `{{name}}` takes the token's value as it is: a number stays a number.
- Inside a string (a sentence, a markup attribute), the value is spliced into the text; a token spliced in is a string, number or boolean.
- The server remembers which fields came from which token, and a sentence's template, so changing a token in `layer_update` (or a render's `rows`) changes every field bound to it; setting such a field to a value of its own unbinds it.
- A string without `{{name}}` is never touched, so `"$29"` and `"{curly}"` stay text. An unknown name is an error that lists the tokens there are.
- `{{n}}` is the counting number ([Counting](#counting)), never a token.
- A whole value written `"$name"`, for a token that exists, reads as `{{name}}`. Text with `$name` in it is left alone (it may be a price), and the edit's reply adds `hint: did you mean {{name}}?`.

### Styles

`styles` hold any layer fields, `media` included: a `card` style can carry `fill`, `borderRadius` and `shadow`. A layer with its own `media` keeps the style's too: `{"style": "ink", "media": {"story": {"fontSize": 40}}}` still gets `ink`'s `a4-portrait` color, and where both set a field for one size, the layer's wins. A layer's `style` takes one name or a list; a later style wins where they overlap, and the layer's own fields win over all of them. Changing a style through `layer_update` changes every layer that uses it. A style can't set `id`, `text` or children; a `type` in it is dropped.

### Components

```json
"components": {
  "candidate": {"type": "frame", "flexDirection": "column", "gap": 4, "alignItems": "center", "children": [
    {"type": "text", "role": "name", "text": "{{name}}", "style": "name"},
    {"type": "text", "role": "office", "text": "{{office}}", "style": "office"}]}
}
```

```json
{"type": "use", "id": "c", "component": "candidate", "each": [
  {"name": "Dana Levi", "office": "Mayor"},
  {"name": "Omar Haddad", "office": "Council"}]}
```

- `{{prop}}` (or `{{ prop }}`) in any string of a component is filled from the instance's props, as in Mustache; a string that is only `{{prop}}` takes the value as is (a number stays a number). `{{n}}` stays the counting number, never a prop. A prop wins over a token of the same name; any other `{{name}}` in the component is a token.
- The `use` layer's own fields (width, constraints, `media`…) apply to each instance's root.
- Instances stay linked: changing the component changes every instance. Their layers are named by the `use` id, the instance number and the inner layer's id or role, e.g. `c.1.name`, in replies. To change one, target the component (`{"component": "candidate", "role": "name"}`), or `detach` the `use` layer into plain layers.
- Components may place other components, up to 8 levels deep.

## Motion

A scene with a `duration`, or made of [shots](#shots-and-transitions), moves. Only fields that don't change layout animate, so the layout is the same at every moment and every check holds throughout. A scene without motion fields is drawn at rest. A still of a moving scene (PNG, JPEG, WebP, PDF, without `time`) is also at rest: each layer as written, before its tracks (a `"scale": 1.12` written for a pan's room shows at 1.12), except `draw` and `count`, which show where they end. Output formats are in [tools.md](tools.md#output-formats).

### Scene timing

| Field | Type | Default | Meaning |
|---|---|---|---|
| `duration` | Seconds, 0.001–86,400 | none (a still), or where the last shot ends | Length; its presence makes the scene move |
| `fps` | 1–120 | 30 | Frames per second |
| `loop` | boolean | false | The animation repeats forever |

### Enter and exit

```json
{"type": "text", "text": "…", "enter": "fade-up"}
{"type": "frame", "enter": {"effect": "pop", "delay": 1.2, "ease": "back.out"}, "exit": {"effect": "fade", "delay": 7}}
```

`enter` and `exit` take an effect name, or an object. A layer is hidden before it enters and gone after it exits. A directional effect is named for the way the layer moves: `fade-left` moves left into place.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `effect` | name | required | `fade`, `fade-up`, `fade-down`, `fade-left`, `fade-right` (fade while moving `distance` into place), `pop` (grow from 0.6 with an overshoot), `zoom-in` (grow from 0.85), `zoom-out` (shrink from 1.15), `blur-in` (sharpen from a 12 px blur) |
| `delay` | Seconds | `enter`: 0; `exit`: so it ends with the scene | When it starts |
| `duration` | Seconds | 0.6 | |
| `ease` | Ease | `power2.out` entering (`back.out` for `pop`), `power2.in` leaving | |
| `distance` | px | 40 | How far a directional fade travels |

### Keyframes

`animate` sets values over time, GSAP-style: one track or a list of them.

```json
"animate": {"scale": [1, 1.06, 1], "duration": 1.6, "repeat": -1, "ease": "sine.inOut"}
"animate": {"rotate": {"from": "random(-90, 90)"}, "translate": {"from": [0, -80]}, "duration": 0.8, "ease": "back.out"}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `opacity`, `scale`, `rotate`, `blur` | list of numbers, or `{from, to}` | | Values spread over `duration`; a missing end is the layer's own value |
| `translate`, `skew` | the same, with `[x, y]` pairs | | GSAP's `x` and `y` read as `translate` |
| `color` | the same, with Colors | | The layer's own color |
| `draw` | list of 0–1, or `{from, to}` | 1 | The share of the layer's strokes drawn ([Drawing strokes](#drawing-strokes)); GSAP's `drawSVG` and After Effects' `trimPath` read as this |
| `count` | list of numbers, or `{from, to}` | 0 | The number a text's `{{n}}` shows ([Counting](#counting)) |
| `decimals` | 0–6 | 0 | With `count`: digits after the decimal point |
| `separator` | string | none | With `count`: put between thousands, e.g. `","`; with `"."` the decimal mark is a comma |
| `times` | list of 0–1 | evenly spaced | Where each listed value falls, of `duration` |
| `delay` | Seconds | 0 | When it starts |
| `duration` | Seconds | 1 | One play |
| `ease` | Ease | `power1.inOut` | Between each pair of values |
| `repeat` | number | 0 | Extra plays; −1 repeats to the end |
| `yoyo` | boolean | false | Every other play runs backwards |

A number may be `"random(lo, hi)"`, as in GSAP: each target (each layer, or each piece of split text) gets its own value, seeded from its id, the same on every render. Values hold before a track starts and after it ends. Layout fields (`width`, `fontSize`, `text`, `padding`…) can't animate; to make something grow, animate `scale`.

### Drawing strokes

`draw` draws a share of a layer's strokes, like After Effects' trim paths: a progress ring, a line or underline drawing itself, line art signing itself.

```json
{"type": "ellipse", "width": 120, "height": 120, "stroke": {"width": 10, "color": "#0AE448", "cap": "round"}, "animate": {"draw": [0, 0.72], "duration": 1.2, "ease": "power2.out"}}
{"type": "path", "d": "M0 40 C 40 0, 80 80, 120 40", "stroke": "#000", "animate": {"draw": {"from": 0}, "duration": 2}}
```

It works on every stroke: paths, shapes, lines, ellipses and text outlines. A line draws from its start to its end, an ellipse clockwise from the top, a path from its first point, and text letter by letter; dashes and end markers follow the drawn part, and an ellipse's dashes start at the top too. Stills at a `time` show the share drawn then; at rest, the share where the track ends (all of it without a `draw` track).

### Counting

`count` puts a number that counts into a text layer, wherever its text says `{{n}}`:

```json
{"type": "text", "text": "{{n}}+ teams", "animate": {"count": [0, 1250], "separator": ",", "duration": 1.5, "ease": "power2.out"}}
```

The text is measured with its widest value (usually the last), so its box holds still and nothing around it moves while the number changes; digits use the font's tabular figures when it has them. At rest, and in `scene_describe`, the text shows that value. A text that counts needs `{{n}}` in it.

### Easing

GSAP's names: `none`, `power1` … `power4`, `sine`, `expo`, `circ`, `back`, `elastic`, `bounce`, each with `.in`, `.out` or `.inOut` (a family alone is `.out`), and `steps(n)`. CSS's `ease`, `ease-in`, `ease-out` and `ease-in-out`, and `smooth`, `snappy` and `bouncy`, read as the nearest of those.

### Stagger and split

| Field | Type | On | Meaning |
|---|---|---|---|
| `stagger` | Seconds | a frame or `use` layer | Gives its `enter` to its children or instances one after another, this far apart, instead of entering whole |
| `split` | `chars`, `words` | a text layer | Its `enter`, `exit`, `animate` and `stagger` apply to each letter or word, like GSAP's SplitText. The text is laid out once; each piece moves as a rigid part of it. A `highlight` stays whole, each line's arriving with its first piece |

```json
{
  "type": "text",
  "text": "Animate Anything",
  "fontSize": 96,
  "split": "chars",
  "stagger": 0.05,
  "animate": {
    "translate": {
      "from": [
        "random(-400, 400)",
        "random(-250, 250)"
      ]
    },
    "rotate": {
      "from": "random(-180, 180)"
    },
    "opacity": {
      "from": 0
    },
    "duration": 1.1,
    "ease": "back.out"
  }
}
```

Frames clip their children, animated ones included: give a frame `clipsContent: false` when its children move beyond its edges.

### Soundtrack

`audio` puts music (or any sound) under an MP4 or WebM, mixed with the clips' own sound. It starts with the video and is cut to its length.

```json
{"asset": "song", "volume": 0.8, "trimStart": 12, "fadeIn": 0.5, "fadeOut": 1.5}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `asset` | asset id | required | An MP3, M4A or WAV added with `asset_add`, or a video clip whose sound plays |
| `volume` | number | 1 | Loudness; 1 is as recorded |
| `trimStart` | Seconds | 0 | Where in the sound to begin |
| `fadeIn` | Seconds | 0 | Rise from silence at the start |
| `fadeOut` | Seconds | 0 | Fall to silence at the video's end |

Set it with `layer_update`'s `{target: {scene: true}, set: {audio: …}}`; `music`, `soundtrack` or an asset id alone read as it, and `null` removes it. A sound shorter than the video ends in silence. Animated PNG and GIF have no sound; `render`'s `muted` leaves it out of video.

### Shots and transitions

A layer of `type: "shot"` is a shot: a full-size frame that plays in turn with the other shots instead of stacking. Times inside a shot (`enter`, `animate`, a clip's `delay`) count from the shot's own start. Layers that aren't shots, such as a logo or a caption bar, stay on across all of them. At rest, the first shot shows. Every shot is checked at every size; to keep one out of a size that never plays it (an A4 page of a video's first shot), hide it there: `"media": {"a4-portrait": {"hidden": true}}`. Its checks and facts then skip that size.

```json
{"id": "s1", "type": "shot", "duration": 3, "children": ["…"]}
{"id": "s2", "type": "shot", "duration": 3, "transition": "push-left", "children": ["…"]}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `duration` | Seconds | required | On screen, including its transitions |
| `transition` | name, or `{type, duration, ease}` | `cut` | How it enters from the shot before |
| `children`, and a frame's fields | | | Its layers and layout |

| Transition field | Type | Default | Meaning |
|---|---|---|---|
| `type` | name | required | `cut`, `fade`, `slide-left` … `slide-down` (slides in over the last shot), `push-left` … `push-down` (pushes the last shot out), `wipe-left` … `wipe-down` (a moving edge reveals it), `zoom` (the last shot grows as the new one fades in). A direction is the way the new shot moves |
| `duration` | Seconds | 0.5 | Overlaps the two shots; a cut has none |
| `ease` | Ease | `power2.inOut` | |

Each shot starts where the one before ends minus its transition. The scene's length is where the last shot ends unless `duration` says otherwise; a longer `duration` holds the last shot to the end. A transition can't be longer than either shot it joins.

## Template files

A template is a scene file that `scene_create` loads by URL or path ([tools.md](tools.md#templates-and-variants)). It has a scene's fields, and its `assets` name files instead of hashes: a URL, or a path relative to the template. Its `tokens` are its variables.

```json
{
  "sizes": ["instagram-square", "iab-medium-rectangle"],
  "tokens": {"headline": "Spring sale", "price": "$29", "accent": "#D0202E"},
  "assets": {"photo": "photo.jpg", "logo": "https://example.com/logo.svg"},
  "layers": [
    {"type": "image", "asset": "photo", "width": "fill", "height": "fill"},
    {"type": "text", "text": "{{headline}}", "fontSize": 64, "fontWeight": 800, "color": "{{accent}}", "place": "center"}
  ]
}
```

A scene keyline saved (its `assets` by `sha256`) loads as a template too.

## Validation and limits

A change that breaks any of these rules is refused whole, with a one-line error ([tools.md](tools.md#errors)); what the layout does at each size (overflow, clipping, contrast) is never refused, but reported as [problems](tools.md#problem-lines).

- Unknown fields, in layers and in every object inside them.
- Values of the wrong type or out of range: opacity 0–1, `fontWeight` 100–900 in 100s, `minimumScaleFactor` above 0 and at most 1, `flexGrow` ≥ 0, polygon `sides` ≥ 3, video `playbackRate` 0.01–100, sizes at least 1 px with `scale` above 0.
- Values whose unit was likely mistaken: a `lineHeight` above 4 (px, not × `fontSize`), a motion `duration` of 100 or more that's longer than the scene (ms, not seconds).
- Layout fields on a frame that isn't a stack or grid (`padding` without `flexDirection`), and `margin` without `place`.
- An unknown token, style, component, asset, parent frame or `media` key; a duplicate layer id.
- Shots that aren't top level, have no positive `duration`, or whose transition is longer than a shot it joins; `split` on anything but text; `count` on a layer whose text has no `{{n}}`; `draw` or `count` on split text.
- An `audio` asset without sound, or a negative `volume`, `trimStart`, `fadeIn` or `fadeOut`; a sound used as an image or video.

| Limit | Value |
|---|---|
| Grid tracks per axis, `repeat` count, `gridRow` and `gridColumn` values | 100 |
| Component nesting | 8 levels |
| `fps` | 1–120 |
| `duration` | 0.001–86,400 s |

Asset and file limits are in [tools.md](tools.md#limits).

## Example

The reference ad from the end-to-end tests, in one `layer_add`: tokens, styles, two components placed with `each`, and a layout that adapts to a portrait post, a wide banner and a skyscraper without per-size positions.

```json
{
  "tokens": {
    "navy": "#1B2A5C",
    "red": "#D0202E",
    "grey": "#6B7280"
  },
  "styles": {
    "accent": {
      "color": "{{red}}"
    },
    "name": {
      "fontSize": 36,
      "fontWeight": 700,
      "color": "{{navy}}",
      "textAlign": "center"
    },
    "office": {
      "fontSize": 30,
      "fontWeight": 500,
      "color": "{{grey}}",
      "textAlign": "center"
    }
  },
  "components": {
    "candidate": {
      "type": "frame",
      "flexDirection": "column",
      "gap": 4,
      "alignItems": "center",
      "children": [
        {
          "type": "text",
          "role": "name",
          "text": "{{name}}",
          "style": "name"
        },
        {
          "type": "text",
          "role": "office",
          "text": "{{office}}",
          "style": "office"
        }
      ]
    },
    "step": {
      "type": "frame",
      "width": "fill",
      "flexDirection": "row",
      "gap": 16,
      "alignItems": "center",
      "children": [
        {
          "type": "image",
          "asset": "check",
          "width": 40,
          "height": 40
        },
        {
          "type": "text",
          "role": "step",
          "text": "{{text}}",
          "fontSize": 32,
          "fontWeight": 500,
          "color": "{{navy}}",
          "width": "fill"
        }
      ]
    }
  },
  "layers": [
    {
      "id": "page",
      "type": "frame",
      "width": "fill",
      "height": "fill",
      "flexDirection": "column",
      "gap": 28,
      "padding": [
        48,
        0
      ],
      "alignItems": "flex-start",
      "children": [
        {
          "id": "headline",
          "type": "text",
          "width": "fill",
          "padding": [
            0,
            40
          ],
          "fontSize": 64,
          "fontWeight": 800,
          "color": "{{navy}}",
          "textAlign": "center",
          "textWrap": "balance",
          "text": "Proven <accent>RESULTS</accent> for <accent>WILLOWMERE</accent> Families"
        },
        {
          "id": "photo",
          "type": "image",
          "asset": "photo",
          "width": "fill",
          "height": "fill",
          "minHeight": 120
        },
        {
          "id": "cands",
          "type": "frame",
          "width": "fill",
          "flexDirection": [
            "row",
            "column"
          ],
          "gap": 12,
          "alignItems": "flex-start",
          "justifyContent": "space-evenly",
          "children": [
            {
              "id": "c",
              "type": "use",
              "component": "candidate",
              "each": [
                {
                  "name": "Dana Levi",
                  "office": "Mayor"
                },
                {
                  "name": "Omar Haddad",
                  "office": "Council"
                },
                {
                  "name": "Ruth Cohen",
                  "office": "Council"
                }
              ]
            }
          ]
        },
        {
          "id": "cta",
          "type": "frame",
          "width": "fill",
          "fill": "{{red}}",
          "flexDirection": "row",
          "gap": 16,
          "padding": 22,
          "alignItems": "center",
          "justifyContent": "center",
          "children": [
            {
              "type": "icon",
              "name": "mail",
              "color": "#FFFFFF",
              "width": 48,
              "height": 48
            },
            {
              "type": "text",
              "text": "VOTE BY MAIL",
              "fontSize": 48,
              "fontWeight": 800,
              "color": "#FFFFFF"
            }
          ]
        },
        {
          "id": "steps",
          "type": "frame",
          "width": "fill",
          "flexDirection": "column",
          "gap": 12,
          "padding": [
            0,
            60
          ],
          "alignItems": "flex-start",
          "children": [
            {
              "id": "s",
              "type": "use",
              "component": "step",
              "each": [
                {
                  "text": "Request your ballot by October 20"
                },
                {
                  "text": "Fill it out at home"
                },
                {
                  "text": "Mail it back by November 3"
                }
              ]
            }
          ]
        },
        {
          "id": "footer",
          "type": "text",
          "width": "fill",
          "text": "Paid for by Willowmere Forward · willowmereforward.org",
          "fontSize": 20,
          "color": "{{grey}}",
          "textAlign": "center",
          "media": {
            "tall": {
              "hidden": true
            }
          }
        }
      ]
    }
  ]
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
