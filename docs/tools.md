# MCP tools reference

keyline-mcp exposes six tools. This page lists each tool's inputs and the exact shape of its replies. The fields of a layer (text, image, frame, fills and so on) are the scene format, described in [scene.md](scene.md); the `layer_add` tool description carries a compact version of them for the agent.

Conventions for every tool:

- **Scenes are addressed by id.** `sceneId` comes from `scene_create`. Scenes, assets and renders live in the data directory (`KEYLINE_MCP_DATA`, default `~/.keyline-mcp`). The only argument that can be a file path is `asset_add`'s `path`, and only in folders the server was started with (`--allow-read`).
- **Replies are compact text, not JSON.** Every change bumps the scene's version, shown as `v3`.
- **Defaults are omitted** in both directions: leave a field out to get its default.
- **Errors are one line** that names what failed and how to fix it, and change nothing: a batch is applied whole or not at all.
- **Fonts:** a `fontFamily` that isn't installed is fetched from Google Fonts once and cached. The reply then starts with `fetched font <family> (<n> files)`.

| Tool | Does |
|---|---|
| [`scene_create`](#scene_create) | Starts a scene: master size, target sizes, background |
| [`asset_add`](#asset_add) | Adds an image from a URL or base64 |
| [`layer_add`](#layer_add) | Adds layers, and shared styles, tokens and components |
| [`layer_update`](#layer_update) | Changes, deletes or detaches layers, styles and components; changes tokens |
| [`scene_describe`](#scene_describe) | Checks the design at every size, or lists every layer's box |
| [`render`](#render) | Writes PNG, JPEG, WebP, PDF, animated PNG or GIF files, or a still of a moment, optionally with a preview image |

## scene_create

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sizes` | array, required | | Target sizes. Each is `{id, width, height, scale, safe}`, a preset name, or `"WxH"` |
| `width`, `height` | number | the first size's | The master size, px: the size the design is written at |
| `background` | color | `#FFFFFF` | Canvas color |
| `duration` | number | none | Seconds: makes the scene move ([Motion](scene.md#motion)) |
| `fps` | number | 30 | Frames per second of animated output |
| `loop` | boolean | false | The animation repeats forever |

A size's `scale` (default 1) shrinks everything, fonts included, before the layout adapts to the size. `safe` is `[top, right, bottom, left]` px that the platform covers (a story's UI, for example); text under it is reported as `!unsafe`.

Presets: `instagram-portrait`, `instagram-square`, `instagram-story`, `facebook-feed`, `linkedin-post`, `x-post`, `youtube-thumbnail`, `iab-medium-rectangle`, `iab-leaderboard`, `iab-skyscraper`, `iab-half-page`, `a4-portrait`. A preset's id is its name; a `"WxH"` size's id is that string.

Reply: the new scene's id and version.

```text
sfc5e3bbb5b v0
```

## asset_add

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `url` | string | | Public http(s) URL of a PNG, JPEG or SVG |
| `path` | string | | Or a local file, inside a folder the server was started with (`--allow-read <folder>`) |
| `base64` | string | | Or the file's bytes, base64. They pass through the model, so keep this for small files |
| `id` | string | generated | The id layers use to refer to it |

Give exactly one of `url`, `path` or `base64`. The limit is 50 MB. A `path` is resolved through every symlink and `..` first, then must lie inside an allowed folder and be a regular file; without `--allow-read`, paths are refused. SVGs are rasterized at their drawn size, so they stay sharp.

Reply: the asset's id, its intrinsic size and the scene version.

```text
photo 864×530 v1
```

## layer_add

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `layers` | array, required | | Layer objects, added on top in order. A layer with `parent` goes inside that frame |
| `styles` | object | | Named styles to add or replace: `{name: {fields}}`. A layer's `style` pulls them in |
| `tokens` | object | | Named values to add or replace: `{name: value}`, used as `"$name"` in any field |
| `components` | object | | Named layer trees to add or replace, placed by `use` layers |

Reply, first line: `added` with the ids of the new top-level layers and the version. Then either `ok` on the same line, or the problems at every size as `scene_describe` reports them. The last line is facts: the smallest text and any upscaled image, per size.

A clean edit:

```text
added headline,cta v1 ok
smallest text: instagram-portrait 40px (cta), 1200x628 40px (cta)
```

An edit with problems:

```text
added photo,headline,cta v2
instagram-portrait cta text 60,1180 300×60 28px (max 48) warn contrast 1.2:1 (WCAG 3)
sky headline text 24,360 384×70 29px 2L !clipped by canvas: right 108px
1200x628 headline text 60,900 960×174 72px 2L !hidden
smallest text: instagram-portrait 28px (cta), sky 11.3px (cta), 1200x628 28px (cta); upscaled: instagram-portrait photo 1.5x, 1200x628 photo 1.4x
```

The agent needs no `scene_describe` call after an edit: the reply already says what's wrong.

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
| `{"component": "card"}` | A component's template; every instance follows |
| `{"component": "card", "role": "name"}` | One layer inside a component's template |

| Action | Does |
|---|---|
| `"set": {fields}` | Merges the fields in; `null` resets a field to its default |
| `"delete": true` | Removes the target (and a layer's children) |
| `"detach": true` | With an `{id}` target of a `use` layer: turns its instances into plain layers that no longer follow the component |

Reply: like `layer_add`, starting with `changed` and the ids of every layer changed or deleted.

```text
changed cta v3
sky headline text 24,360 384×70 29px 2L !clipped by canvas: right 108px
1200x628 headline text 60,900 960×174 72px 2L !hidden
1200x628 cta text 60,1180 600×60 48px !hidden
smallest text: instagram-portrait 48px (cta), sky 19px (cta), 1200x628 48px (cta); upscaled: instagram-portrait photo 1.5x, 1200x628 photo 1.4x
```

## scene_describe

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `size` | string | all sizes | One size id |
| `full` | boolean | false | Every layer's box, not just the problems |

Without `full`, the reply is `ok` or one line per problem: the size, the layer's line (below), and its markers.

| Marker | Kind | Means |
|---|---|---|
| `!overflow needs W×H (one line: W wide)` | defect | Text doesn't fit its box even at its smallest allowed size |
| `!truncated` | defect | Text was cut with an ellipsis |
| `!clipped by <frame or canvas>: <side> <px>` | defect | Part of the layer falls outside what shows |
| `!hidden` | defect | The layer is entirely outside what shows |
| `!overlaps <ids>` | defect | Text ink overlaps other text |
| `!unsafe` | defect | Text sits under the size's `safe` insets |
| `warn contrast R:1 (WCAG N)` | advisory | Text contrast against what's behind it is below the WCAG level for its size |

Defects need fixing; advisories need judgment.

With `full`, the reply lists assets, then each size and every layer at it, indented by nesting:

```text
assets photo 864×530
instagram-portrait 1080×1350
 photo image 0,0 1080×810 fill crop 18%w upscaled 1.5x
 headline text 60,900 960×174 72px 2L
 cta text 60,1180 600×60 48px
```

A layer's line is `id type x,y w×h`, in px at that size, then:

- **text:** the font size drawn, `(max N)` when it shrank to fit, and `2L` for the number of lines
- **image:** the fit, `crop N%w` / `N%h` for how much is cut, and `upscaled N×` when it's enlarged past its pixels
- **adaptive layouts:** `→ row` for the direction a stack chose, or `→ <id>` for the child a `firstFit` drew
- `rot N°` for a rotated layer

## render

| Input | Type | Default | Meaning |
|---|---|---|---|
| `sceneId` | string, required | | The scene |
| `sizes` | array of strings | all sizes | Size ids to render |
| `format` | `png` \| `jpeg` \| `webp` \| `pdf` \| `apng` \| `gif` | `png` | File format; PDF is vector, one page per size (1 px = 1 pt); `apng` and `gif` animate a moving scene |
| `quality` | 0–100 | 90 | JPEG and WebP quality |
| `maxKB` | number | | File-size cap: JPEG and WebP lower their quality until the file fits |
| `preview` | boolean | false | Also returns one small image (384 px tall) of all sizes side by side |
| `time` | number | | Seconds into a moving scene: a still at that moment, saved as `<size>-v<n>.at<time>s.<ext>` |

`format: "apng"` renders a moving scene as an animated PNG (`<size>-v<n>.anim.png`: lossless, fully transparent, plays in browsers), and `format: "gif"` as an animated GIF (plays everywhere, including email and chat, in 256 colors per frame). Frames are drawn in memory, several at once, and each stores only the part that changed; `maxKB` halves the frame rate until it fits. Scenes without a `duration` refuse both. `--no-motion` leaves `duration`, `fps`, `loop`, `time` and the motion fields out of the tools.

Reply: per size, the size id and the file's path. With `maxKB`, the path is followed by `quality N` when the quality was lowered, or `!too-big N KB` when even the lowest quality (or a lossless format) doesn't fit. Under each size, every text that wrapped, shrank or was cut, as actually drawn, so wording and line breaks can be checked without looking at the image:

```text
instagram-portrait /…/renders/sfc5e3bbb5b/instagram-portrait-v3.jpg quality 11
 headline 72px: "Proven RESULTS for" / "Willowmere Families"
```

With `preview`, the reply also carries the preview as an image.

## Errors

Errors are one line and leave the scene unchanged. A batch error names the item that failed:

```text
/Users/me/secret.png is outside the folders the server may read (--allow-read)
layers[0]: unknown field(s) fontsize for text layer; did you mean fontsize → fontSize
ops[0]: no layer with id nope
text1: <span color="$blue">: bad color "$blue", want #RRGGBB
give exactly one of url or base64
```
