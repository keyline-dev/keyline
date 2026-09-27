# Graphic Layer — Design Doc

Status: draft, input to the full scene format (schema v2) · updated 2026-09-26 after the MVP · Yuval Tal
Source: [Scene JSON + MCP — Design Spec](https://claude.ai/artifact/JA2qcvuxn3GgL1aTDkjMEk), sections "Graphic layer", "Fills", "Strokes and effects".

## Overview

In v1, the graphic layer becomes the only layer type for non-text visuals. It is a shape (`rect`, `ellipse`, `polygon`, `star`, `line`, `path`) painted by ordered `fills`, `strokes` and `effects`. It replaces the MVP `image` and `rect` layers, so a photo becomes an image *fill* on a rect and a colored bar becomes a solid fill. Swapping a photo, a background or a gradient is then one `fill_set` call.

This doc covers the JSON shape, how each field maps onto Skia, the migration from the MVP, and the remaining gaps. Text, group and mask layers appear only where they touch this layer.

Since this draft, the MVP gained `ellipse`, `line` and `icon` layer types, a gradient `mask` on any layer, an image `focus` point, row and column stacks on frames, named text styles and per-size changes (`at`). They shipped as separate types to measure them quickly on real-model benchmarks; v2 folds the shapes into this layer (see the migration table).

## Goals and non-goals

Goals:

- One layer type for photos, backgrounds, badges, dividers and icons.
- Every MVP `image` and `rect` layer converts to a graphic layer with no change in rendered pixels. This includes layers nested in frames and the MVP reference ad.
- Every field has a documented default. A typical `layer_add` for a graphic takes 4–6 fields.
- Output is deterministic: the same scene renders byte-identical PNGs on CPU.
- Built directly on Skia primitives wherever possible. Custom code only where Skia has no equivalent.

Non-goals (v1):

- Boolean operations (union, subtract) and vector networks (editable node graphs). The `path` kind takes an SVG `d` string instead.
- Per-vertex editing tools. Agents replace the whole `d`.
- Animation. Properties stay animatable later (see the spec's "Multi-size output").
- GPU rendering. It stays behind the CPU backend until golden-diffed.

## Layer shape

```json
{
  "id": "hero",
  "type": "graphic",
  "role": "background-photo",
  "x": 0, "y": 0, "width": 1080, "height": 1080,
  "constraints": { "h": "stretch", "v": "stretch" },
  "shape": { "kind": "rect", "cornerRadius": 24 },
  "fills": [
    { "type": "image", "asset": "a_9f3c…", "scaleMode": "fill" },
    { "type": "gradient", "gradientKind": "linear",
      "stops": [{ "at": 0, "color": "#00000000" }, { "at": 1, "color": "#000000AA" }],
      "transform": { "angle": 90 } }
  ],
  "strokes": [],
  "effects": [{ "type": "dropShadow", "x": 0, "y": 8, "blur": 24, "spread": 0, "color": "#00000040" }]
}
```

The common layer properties (`opacity`, `blendMode`, `rotation`, `flipH/V`, `mask`, constraints, overrides) are defined in the spec and apply unchanged. `shape` defaults to `{ "kind": "rect" }`. `fills`, `strokes` and `effects` default to `[]`, and an empty `fills` renders nothing.

## Shapes

All geometry is in the layer's own box (`0,0` to `width,height`), before rotation. The box's `x, y` are relative to the parent frame, as in the MVP. Under the Scale tool, `cornerRadius`, stroke widths, shadow offsets and blur radii scale with the size's `scale` factor, just like font sizes. The MVP already scales corner radii this way.

| `kind` | Fields (defaults) | Skia construction |
| --- | --- | --- |
| `rect` | `cornerRadius` number or `[tl,tr,br,bl]` (0), `cornerSmoothing` 0–1 (0) | `RRect::set_rect_radii`; smoothing > 0 → custom squircle path |
| `ellipse` | `arcStart` (0), `arcEnd` (360), `innerRadius` 0–1 (0) | `Path::add_oval`; arc → `add_arc`; ring → outer + inner oval, even-odd |
| `polygon` | `sides` ≥ 3 (3), `cornerRadius` (0) | Regular n-gon inscribed in the box; radius → `PathEffect::corner_path` |
| `star` | `points` (5), `innerRadius` 0–1 (0.38), `cornerRadius` (0) | 2 × `points` vertices, alternating outer/inner |
| `line` | `x2`, `y2` (`width`, 0) | `move_to(0,0)` / `line_to(x2,y2)`; fills ignored, strokes only |
| `path` | `d` SVG path (required), `fillRule` `nonzero` \| `evenodd` (`nonzero`) | `Path::from_svg` + `set_fill_type`; `d` is scaled from its bounds to the box |

The path is built once per layer per target size, after constraints and scale have fixed the box. Fills, strokes and effects all reuse it.

## Fills

Fills paint bottom to top in array order, each clipped to the shape path. Every fill has `opacity` (1), `blendMode` (`normal`) and `enabled` (true).

| `type` | Fields (defaults) | Skia construction |
| --- | --- | --- |
| `solid` | `color` (`#000000`) | `Paint::set_color` |
| `gradient` | `gradientKind` `linear` \| `radial` \| `angular` \| `diamond`, `stops[] {at, color}`, `transform {angle, scale, center}` (90°, 1, [0.5,0.5]) | `Shader::linear_gradient` / `radial_gradient` / `sweep_gradient`; `diamond` → SkSL runtime shader |
| `image` | `asset` (required), `scaleMode` `fill` \| `fit` \| `crop` \| `tile` \| `stretch` (`fill`), `crop {x,y,width,height}` 0–1, `tileScale` (1), `adjustments` | Image shader with a local matrix from `scaleMode` + `crop`; `tile` → `TileMode::Repeat` |
| `pattern` | `asset`, `spacing` ([0,0]), `offset` ([0,0]) | `Picture` recorded once, used as a repeating shader |

`transform` values are in the layer's normalized box (0–1). The agent never builds a matrix.

SVG assets (the MVP uses them for icons) are rasterized with resvg at the fill's final pixel size, after scale and constraints. They are never scaled up from a bitmap. The raster is cached by `(assetId, pixel size)` like any decoded image. `tile` and `pattern` rasterize one SVG tile and repeat it.

Image `scaleMode`:

- `fill`: cover, centered. This is the MVP `fill`.
- `fit`: contain, letterboxed. This is the MVP `fit`.
- `crop`: the `crop` rect of the source is stretched to the box.
- `tile`: repeat at intrinsic size × `tileScale`.
- `stretch`: the whole source is stretched to the box, ignoring aspect ratio.

Image `adjustments` (`brightness`, `contrast`, `saturation`, `temperature`, `tint`, `grayscale`, `sepia`, all 0 = no change) compose into a single 4×5 `ColorFilter` matrix. `blur` is an `image_filters::blur` on that fill only. Decoding and resampling happen once per asset and size through `image` + `fast_image_resize`, and the result is cached by `(assetId, pixel size)`.

## Strokes

A stroke has `color`, `width` (1), `align` `inside` | `center` | `outside` (`inside`), `style` `solid` | `dashed` | `dotted`, `dashPattern`, `cap`, `join`, `miterLimit` (4). Rects can also take per-side widths.

- `center` strokes the path directly.
- `inside` clips to the path and strokes at 2 × `width`. `outside` clips out the path and strokes at 2 × `width`.
- Open paths (`line`, unclosed `path`) always use `center`. Any other `align` is ignored and reported by `scene_validate`.
- Dashes use `PathEffect::dash`. `dotted` is a zero-length dash with a round cap.
- Per-side rect widths are drawn as four clipped edge strokes. Corners with non-zero radius fall back to the max width (see Open questions).

## Effects

Effects render in a fixed order around the layer, not in array order: `dropShadow` → fills → `innerShadow` → strokes → `layerBlur`. `backgroundBlur` applies first, to whatever is already under the shape.

| Effect | Fields | Skia construction |
| --- | --- | --- |
| `dropShadow` | `x, y, blur, spread, color` | `image_filters::drop_shadow_only` on the shape, spread by dilating the path |
| `innerShadow` | `x, y, blur, spread, color` | Inverse-filled offset path, blurred, clipped to the shape |
| `layerBlur` | `radius` | `save_layer` with `image_filters::blur` |
| `backgroundBlur` | `radius` | `SaveLayerRec` backdrop blur, clipped to the shape |
| `glow` (ext.) | `radius, color, intensity` | Drop shadow at 0,0 with dilation |
| `outline` (ext.) | `width, color` | Outside stroke on the silhouette, including fills' alpha |
| `duotone` (ext.) | `shadowColor, highlightColor` | Luminance → two-color gradient `ColorFilter` |

## Render order for one layer

| Step | What happens | Skia |
| --- | --- | --- |
| 1 | Resolve box (scale, constraints, overrides) and build the shape path | `Path` |
| 2 | Open the layer group if `opacity < 1`, `blendMode ≠ normal`, `mask`, or `layerBlur` | `save_layer` |
| 3 | Apply rotation and flips about the box center | `Canvas::concat` |
| 4 | Background blur, then drop shadows | backdrop filter, image filters |
| 5 | Fills bottom to top, each clipped to the path | `clip_path(anti_alias=true)` + shaders |
| 6 | Inner shadows, then strokes | clipped draws |
| 7 | Apply mask, close the group | `restore` |

The group in step 2 is skipped when not needed, which keeps plain layers cheap.

## Migration from MVP layers

The v1 loader upgrades MVP scenes on read, recursing into frame `children`. MVP JSON is not written back.

| MVP layer / field | v1 equivalent |
| --- | --- |
| `image` | `graphic`, `shape: { kind: "rect" }`, `fills: [{ type: "image", asset, scaleMode }]`, where `scaleMode` takes the MVP `fit` value (`fill` \| `fit`) |
| `image.width, height` omitted | The box takes the image fill's intrinsic size (SVG: its viewBox size) |
| `rect` | `graphic`, `shape: { kind: "rect", cornerRadius }`, `fills: [{ type: "solid", color }]`; no `color` → `fills: []` |
| `image.focus` | The image fill's `focus` (0–1 per axis, default center) |
| `ellipse` | `graphic`, `shape: { kind: "ellipse" }`, its `color`/`gradient` → one fill, `stroke` → one stroke |
| `line` | `graphic`, `shape: { kind: "line", x2: width, y2: height }`, `color` and `strokeWidth` → one center stroke |
| `icon` | Unchanged: a named, one-color icon is shorter for agents than an SVG fill, and the benchmarks show naming icons is what keeps runs cheap |
| `frame` | Container with `children` and `clip` (default true). Its own `color` and `cornerRadius` → a solid fill on a rect shape, drawn under the children and used as the clip path. `stack` stays on the frame. |
| everything else (`mask`, `at`, `styles`, text) | unchanged |

A golden test renders each MVP fixture before and after the upgrade and requires byte-identical PNGs. The fixtures include the reference ad in all three sizes.

The MCP surface follows the same change: `layer_add` accepts `{ type: "graphic", fills: [...] }`. Two shorthands keep the MVP's short calls working: `{ type: "graphic", image: "<assetId>" }` expands to a rect with one image fill, and `{ type: "graphic", color: "#D0202E" }` expands to a rect with one solid fill. The agent's common case stays at 4 fields.

## Alternatives considered

| Option | Why not |
| --- | --- |
| Keep `image` as its own layer type | Background swap becomes delete + add + reorder, not one fill edit. Gradients over photos would need an extra layer. |
| One layer type per shape (`rect`, `ellipse`, …) | More types for agents to learn and more tool branches. `shape.kind` keeps a single `graphic` type. |
| Vector networks | Hard for LLMs to author. SVG `d` is widely known and `Path::from_svg` parses it. |
| Gradient as a raw 2×3 matrix | Agents get matrices wrong. `{angle, scale, center}` covers practical cases. |
| Effects applied in array order | Order-dependent output surprises agents. A fixed order matches common design tools and keeps `effect_set` simple. |
| tiny-skia instead of Skia | No backdrop blur, sweep gradient or runtime shaders. The spec already accepts Skia's weight. |

## Open questions

- Resolved by the MVP: `frame` stays a container with its own fill, clip and `children`, the model shared by Figma and SwiftUI. Stacks (auto layout) already attach to it.
- `cornerSmoothing`: implement a squircle (smoothed-corner) path ourselves, or defer it and treat any value as 0?
- `diamond` gradient: an SkSL runtime shader is simple, but is it deterministic across CPU and GPU backends?
- Resolved by the MVP: a line runs from its box's top-left by `width, height`, so constraints move and stretch it like any box.
- `path` scaling: stretch `d` to the box (proposed) or keep its aspect ratio and fit it?
- Per-side rect stroke widths with rounded corners: fall back to the max width, or interpolate across the corner?
- `outline` and `glow` on image fills with transparency: use the fill's alpha silhouette or only the shape path?
- Should `scene_describe` report fill details (e.g. "photo: cover, cropped 12% top") for every image fill, or only for the first?
