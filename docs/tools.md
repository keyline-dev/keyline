# MCP tools reference

keyline-mcp exposes six tools. This page lists each tool's inputs and the exact shape of its replies, and how the server is configured. What the fields of a scene mean is in the [scene format reference](scene.md); the ideas behind both are in [concepts.md](concepts.md). The `layer_add` tool description carries a compact version of the scene format for the agent.

- [Conventions](#conventions)
- [Replies](#replies): problem lines, markers, facts, drawn text, output files
- The tools: [`scene_create`](#scene_create) · [`asset_add`](#asset_add) · [`layer_add`](#layer_add) · [`layer_update`](#layer_update) · [`scene_describe`](#scene_describe) · [`render`](#render)
- [Templates and variants](#templates-and-variants)
- [Output formats](#output-formats)
- [Server configuration](#server-configuration): flags, environment, data, fonts, ffmpeg
- [Security](#security) · [Limits](#limits) · [Errors](#errors)

| Tool | Does |
|---|---|
| [`scene_create`](#scene_create) | Starts a scene: sizes and background, or a template with its variables set |
| [`asset_add`](#asset_add) | Adds an image or video clip from a URL, a local path or base64 |
| [`layer_add`](#layer_add) | Adds layers, and shared styles, tokens and components |
| [`layer_update`](#layer_update) | Changes, deletes or detaches layers, styles and components; changes tokens |
| [`scene_describe`](#scene_describe) | Checks the design at every size, or lists every layer's box |
| [`render`](#render) | Writes image, animation or video files per size, a still of a moment, or one variant per row of variables |

## Conventions

- **Scenes are addressed by id.** `sceneId` comes from `scene_create`. Scenes, assets and renders live in the [data directory](#data-directory).
- **Batches are atomic.** A tool that takes a list applies all of it or none of it.
- **Every change bumps the scene's version**, shown in replies as `v3`.
- **Defaults are omitted** in both directions: leave a field out to get its default.
- **Replies are compact text, not JSON.** Their parts are described once, under [Replies](#replies).
- **Errors are one line** that names what failed and how to fix it, and change nothing ([Errors](#errors)).

## Replies

### Versions and ids

A mutation's first line names what it did, the ids it touched and the new version: `added headline,cta v2`, `changed cta v3`, `photo 864×530 v1`. It never echoes the scene.

### Problem lines

An edit's reply (`scene_create` from a template, `layer_add`, `layer_update`) ends its first line with `ok`, or is followed by one line per problem at each size. `scene_describe` gives the same lines on request. A problem line is the size id, the layer's line, and its markers:

```text
sky headline text 24,360 384×70 29px 2L !clipped by canvas: right 108px
```

A **layer line** is `id type x,y w×h`, in px at that size, then:

- **text:** the font size drawn, `(max N)` when it shrank to fit, and `2L` for the number of lines
- **image** and **video:** the fit, `crop N%w` / `N%h` for how much is cut, and `upscaled N×` when it's enlarged past its pixels
- **adaptive layouts:** `→ row` for the direction a stack chose, or `→ <id>` for the child a `firstFit` drew
- `rot N°` for a rotated layer

### Markers

| Marker | Kind | Means |
|---|---|---|
| `!overflow needs W×H (one line: W wide)` | defect | Text doesn't fit its box even at its smallest allowed size |
| `!truncated` | defect | Text was cut with an ellipsis |
| `!clipped by <frame or canvas>: <side> <px>` | defect | Part of the layer falls outside what shows |
| `!hidden` | defect | The layer is entirely outside what shows |
| `!overlaps <ids>` | defect | Text ink overlaps other text |
| `!unsafe` | defect | Text sits under the size's `safe` insets |
| `warn contrast R:1 (WCAG N)` | advisory | Text contrast against what's behind it is below the WCAG level for its size |

Defects need fixing; advisories need judgment ([concepts](concepts.md#checks-defects-advisories-facts)). In a scene of [shots](scene.md#shots-and-transitions), every shot is checked, each with the layers around it.

### Facts line

The last line of an edit's reply states facts with no threshold, per size: the smallest text and any image drawn larger than its pixels.

```text
smallest text: instagram-portrait 28px (cta), sky 11.3px (cta); upscaled: instagram-portrait photo 1.5x
```

### Drawn-text lines

Under each size, `render` lists every text that wrapped, shrank or was cut, as actually drawn, so wording and line breaks can be checked without looking at the image:

```text
instagram-portrait /…/renders/sfc5e3bbb5b/instagram-portrait-v3.jpg quality 11
 headline 72px: "Proven RESULTS for" / "Willowmere Families"
```

### Output files

`render` writes into `<data>/renders/<sceneId>/`, one file per size, named from the size id and the scene version:

| File | For |
|---|---|
| `<size>-v<n>.<ext>` | A render: `png`, `jpg`, `webp`, `pdf`, `gif`, `mp4`, `webm` |
| `<size>-v<n>.anim.png` | An animated PNG |
| `<size>-v<n>.at<time>s.<ext>` | A still at `time` seconds |
| `<size>-v<n>.r<row>.<ext>` | One row of `rows` |

## scene_create

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sizes` | array | required, or the template's | Target [sizes](scene.md#sizes): a size object, a preset name, or `"WxH"` |
| `width`, `height` | number | the first size's | The master size, px |
| `background` | color | `#FFFFFF` | Canvas color |
| `duration`, `fps`, `loop` | number, number, boolean | a still, 30, false | [Scene timing](scene.md#scene-timing) |
| `url` | string | | A [template](#templates-and-variants) at a public http(s) URL |
| `path` | string | | Or a template file in an allowed folder; offered only with [`--allow-read`](#command-line-flags) |
| `tokens` | object | | The template's variables to set, `{name: value}` |

Reply: the new scene's id and version. From a template, also its variables, then `ok` or [problem lines](#problem-lines), so a value that doesn't fit shows at once. When ffmpeg can't be found (and motion is on), a second line says so up front, so the agent doesn't plan a video it can't make:

```text
s5b0a42a5e v0 tokens: accent, headline, price ok
```

```text
sfc5e3bbb5b v0
video off: no ffmpeg (install it or set KEYLINE_MCP_FFMPEG); apng, gif work
```

## asset_add

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `url` | string | | Public http(s) URL of a PNG, JPEG, SVG or video clip |
| `path` | string | | Or a local file in an allowed folder; offered only with [`--allow-read`](#command-line-flags), and its description names the folders |
| `base64` | string | | Or a still image's bytes, base64. They pass through the model, so keep this for small files |
| `id` | string | generated | The id layers use to refer to it; an existing id is replaced |

Give exactly one of `url`, `path` or `base64`. Video clips (MP4, MOV, WebM…) need [ffmpeg](#ffmpeg) and can't come as base64. SVGs are rasterized at their drawn size, so they stay sharp.

Reply: the asset's id, its intrinsic size and the scene version; for a clip, also its length, frame rate and `sound` when it has any.

```text
photo 864×530 v1
beach 1920×1080 12.5s 30fps sound v2
```

## layer_add

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `layers` | array, required | | [Layer](scene.md#layers) objects, added on top in order. A layer with `parent` goes inside that frame |
| `styles` | object | | [Styles](scene.md#styles) to add or replace: `{name: {fields}}` |
| `tokens` | object | | [Tokens](scene.md#tokens) to add or replace: `{name: value}` |
| `components` | object | | [Components](scene.md#components) to add or replace |

Reply: `added`, the ids of the new top-level layers and the version; then `ok` on the same line, or [problem lines](#problem-lines); then the [facts line](#facts-line). The agent needs no `scene_describe` call after an edit.

```text
added headline,cta v1 ok
smallest text: instagram-portrait 40px (cta), 1200x628 40px (cta)
```

```text
added photo,headline,cta v2
instagram-portrait cta text 60,1180 300×60 28px (max 48) warn contrast 1.2:1 (WCAG 3)
sky headline text 24,360 384×70 29px 2L !clipped by canvas: right 108px
1200x628 headline text 60,900 960×174 72px 2L !hidden
smallest text: instagram-portrait 28px (cta), sky 11.3px (cta), 1200x628 28px (cta); upscaled: instagram-portrait photo 1.5x
```

## layer_update

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `ops` | array, required | | Changes, applied in order, all or none |
| `tokens` | object | | Tokens to change: every field bound to one follows it |

Each op has a `target` and exactly one action:

| Target | Picks |
|---|---|
| `{"id": "cta"}` | One layer |
| `{"role": "price"}` | Every layer with that role |
| `{"style": "title"}` | A named style; `set` creates or changes it, so every layer using it follows |
| `{"component": "card"}` | A component's tree; every instance follows |
| `{"component": "card", "role": "name"}` | One layer inside a component's tree |

| Action | Does |
|---|---|
| `"set": {fields}` | Merges the fields in; `null` resets a field to its default |
| `"delete": true` | Removes the target (and a layer's children) |
| `"detach": true` | With an `{id}` target of a `use` layer: turns its instances into plain layers that no longer follow the component |

Reply: like `layer_add`, starting with `changed` and the ids of every layer changed or deleted.

```text
changed cta v3
sky headline text 24,360 384×70 29px 2L !clipped by canvas: right 108px
smallest text: instagram-portrait 48px (cta), sky 19px (cta)
```

## scene_describe

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `size` | string | all sizes | One size id |
| `full` | boolean | false | Every layer's box, not just the problems |

Reply: `ok`, or one [problem line](#problem-lines) per problem. With `full`, the assets (a clip with its length and `sound`), then each size and every [layer line](#problem-lines) at it, indented by nesting:

```text
assets photo 864×530
instagram-portrait 1080×1350
 photo image 0,0 1080×810 fill crop 18%w upscaled 1.5x
 headline text 60,900 960×174 72px 2L
 cta text 60,1180 600×60 48px
```

## render

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `sizes` | array of strings | all sizes | Size ids to render |
| `format` | `png` \| `jpeg` \| `webp` \| `pdf` \| `apng` \| `gif` \| `mp4` \| `webm` | `png` | See [Output formats](#output-formats) |
| `quality` | 0–100 | 90 | JPEG and WebP quality |
| `maxKB` | number | | File-size cap: JPEG and WebP lower their quality, APNG and GIF their frame rate, until the file fits |
| `preview` | boolean | false | Also returns one small image of all sizes side by side |
| `time` | number | | Seconds into a moving scene: a still at that moment |
| `audio` | boolean | true | `false` leaves the clips' sound out of `mp4` and `webm` |
| `rows` | array of objects | | [Variants](#templates-and-variants): one render per row of token values |

Reply: per size, the size id and the file's path ([Output files](#output-files)), then its [drawn-text lines](#drawn-text-lines). With `maxKB`, the path is followed by `quality N` or `fps N` when it was lowered, or `!too-big N KB` when even the lowest setting doesn't fit. With `rows`, each line starts with `r<row>`. With `preview`, the reply also carries the preview as an image.

## Templates and variants

A template is a scene file ([its format](scene.md#template-files)) that `scene_create` loads by `url` or `path`, the same way `asset_add` loads an image. Its images are added as assets, `tokens` sets its variables, and any other `scene_create` input (`sizes`, `background` …) replaces the template's. Nothing else is kept: the template stays wherever it came from. A token named in `tokens` that the template doesn't have is an error that lists the ones it has.

`render` with `rows` makes variants: each row of token values is applied as `layer_update` with those `tokens` would, to a copy, and rendered; the saved scene doesn't change. A key that isn't one of the scene's tokens is an error naming the row and the tokens there are. `preview` shows the first row.

```json
{"sceneId": "s5b0a42a5e", "rows": [{"headline": "Fall", "price": "$19"}, {"headline": "Winter", "price": "$24"}]}
```

## Output formats

| Format | What it is |
|---|---|
| `png`, `jpeg`, `webp` | A still per size. A scene with motion is drawn at rest, or at `time` |
| `pdf` | Vector, one page per size (1 px = 1 pt). Refuses scenes with video |
| `apng` | Animated PNG: lossless, fully transparent, plays in browsers |
| `gif` | Animated GIF: plays everywhere, including email and chat, in 256 colors per frame |
| `mp4` | H.264 video, plays everywhere. Needs [ffmpeg](#ffmpeg) |
| `webm` | VP9 video. Needs [ffmpeg](#ffmpeg) |

The animated and video formats need a scene that moves (a `duration`, or shots); `time` can't be combined with them. Frames are drawn in memory, several at once, and APNG and GIF frames store only the part that changed. MP4 encodes on the GPU when ffmpeg has a hardware encoder that works on the machine (VideoToolbox on macOS; NVENC, Quick Sync or AMF elsewhere), else with `libx264`; `KEYLINE_MCP_ENCODER` picks one. The clips' own sound comes along, AAC in MP4 and Opus in WebM, unless `audio` is `false`.

## Server configuration

### Command-line flags

| Flag | Does |
|---|---|
| `--allow-read <folder>` | Lets `asset_add` and `scene_create` read local files inside this folder (repeatable). Without it, `path` isn't offered to the agent at all |
| `--no-motion` | Leaves motion out of the tools: `duration`, `fps`, `loop`, `time`, `audio`, video, shots and the motion fields. Fewer tokens per turn, for stills-only use |
| `-h`, `--help` | Prints the flags and environment variables |

### Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `KEYLINE_MCP_DATA` | `~/.keyline-mcp` | The [data directory](#data-directory) |
| `KEYLINE_MCP_FONTS` | none | An extra folder of `.ttf` and `.otf` fonts |
| `KEYLINE_MCP_RENDERER` | `gpu` | `gpu` renders on the GPU and falls back to the CPU; `cpu` always uses the CPU |
| `KEYLINE_MCP_FFMPEG` | `ffmpeg` on the PATH | The ffmpeg program |
| `KEYLINE_MCP_ENCODER` | `auto` | H.264 encoder: `auto` (a GPU encoder that works, else `libx264`), `software`, or an ffmpeg encoder name |

### Data directory

Scenes are saved as JSON under `<data>/scenes/`, assets under `<data>/assets/` by content hash, renders under `<data>/renders/<sceneId>/`, and downloaded fonts under `<data>/fonts/`.

### Fonts

Inter is bundled. A `fontFamily` that isn't installed is fetched from [Google Fonts](https://fonts.google.com) once and cached in `<data>/fonts/` (tracked in `fonts/index.json`); the reply then starts with `fetched font <family> (<n> files)`. Fonts in `<data>/fonts/` and `KEYLINE_MCP_FONTS` are loaded too. Fonts are never taken from the machine's system fonts, so renders don't vary with what's installed.

### ffmpeg

Video clips and MP4 and WebM output need [ffmpeg](https://ffmpeg.org) (`brew install ffmpeg`, `apt install ffmpeg`), run as a separate program. It's looked up on every call that needs it (`KEYLINE_MCP_FFMPEG`, else the PATH), so it can be installed without a restart. Without it, those calls are refused with how to install it, `scene_create` says `video off` up front, and everything else works, APNG and GIF included.

## Security

- **Local files** are read only with `--allow-read`. A `path` is resolved through every symlink and `..` first, then must lie inside an allowed folder and be a regular file, so a link inside the folder can't lead outside it.
- **URLs** are fetched only over http(s); private and local addresses are refused, and every redirect is checked again.
- **Templates** follow the same rules for their images: a template from a URL reads images from the web only, never local files; one from a path reads only inside the allowed folders.
- **Base64** is taken only for still images.

## Limits

| What | Limit |
|---|---|
| An image, by any source | 50 MB |
| A video clip, by `path` | 500 MB |
| A template file | 50 MB |
| `preview` | 384 px tall |

Limits of the scene itself (grid tracks, component depth, frame rates) are in [scene.md](scene.md#validation-and-limits).

## Errors

Errors come back as an MCP tool error (`isError: true`) with one line of text that names what failed and how to fix it. They leave the scene and its version unchanged; only a font fetched on the way stays cached. A batch error names the item that failed:

```text
/Users/me/secret.png is outside the folders the server may read (--allow-read)
layers[0]: unknown field(s) fontsize for text layer; did you mean fontsize → fontSize
ops[0]: no layer with id nope
text1: <span color="$blue">: bad color "$blue", want #RRGGBB
row 2: no token headlin; tokens: accent, headline
give exactly one of url, path or base64
```
