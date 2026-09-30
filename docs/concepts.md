# Concepts

keyline is a design engine driven by an AI agent. The agent writes a design as a small JSON scene through six MCP tools; keyline lays it out at every size you need, checks it, and renders it. This page explains the ideas. The fields are in the [scene format reference](scene.md), and the calls and replies in the [tools reference](tools.md).

## What a scene is

A scene is one JSON document: a tree of layers (frames, text, images, shapes, icons, video), the assets they use, and shared tokens, styles and components. It is structured data, not pixels, so the agent edits it precisely instead of regenerating an image. The server keeps the scene; the agent changes it in batches of operations that apply whole or not at all, and every change bumps its version.

## Master size and sizes

A design is written once, at a **master size** (say 1080×1350), and lists the **sizes** it's rendered at: a square post, a wide banner and a 300×600 skyscraper from the same layers. Sizes can be named presets for common social and ad formats.

Each size may carry a **scale**, like a design tool's Scale tool: everything, fonts included, shrinks by that factor before the layout adapts. A size may also have a **safe area**, the part a platform covers (a story's top and bottom bars), where text is flagged.

### Aspect classes

Sizes fall into aspect classes: **landscape**, **square** or **portrait**, and within those **wide** or **tall** for the extremes. Any layer can change its fields for one class (`media`), so one `tall` entry covers every skyscraper the design is ever rendered at, including sizes added later. A change for one size id is the most specific and applies last. The thresholds are in [scene.md](scene.md#aspect-classes).

## How layout adapts

For each size, keyline scales the master by the size's scale, then fits it to the target: free-placed layers follow their constraints, and rows, columns and grids lay out again, recursively. There is no constraint solver; that's where layout systems get slow and agent-written layouts become undebuggable.

### Free, stack and grid

A frame lays out its children in one of three ways:

- **Free:** each child has a position and **constraints** that say how it follows the frame as it resizes (pinned left, stretched, centered…), as in Figma, or it pins to one of nine spots (`bottom-right`).
- **Stack:** a row or column with gap, padding, alignment and wrapping, like CSS flexbox and Figma auto layout. Children size themselves with `hug`, `fill` or a percentage.
- **Grid:** CSS-style tracks, named areas and spans.

### Choosing what fits

Some layouts pick between alternatives at each size. A stack can be a row where a row fits and a column elsewhere. A `firstFit` layer, like SwiftUI's `ViewThatFits`, draws the first of its children that fits with nothing wrong inside it: a long headline where there's room, a short one where there isn't.

### Text that fits its box

The box decides how text behaves. With a width and height, the font shrinks until the text fits, down to a floor, then ends with an ellipsis. With a width only, it wraps and the box grows down. With neither, it's one line. When text still doesn't fit, keyline says so; it never changes the design silently.

## Checks: defects, advisories, facts

The server measures; the agent decides. Every edit's reply checks the design at every size and reports three kinds of feedback:

| Kind | Example | The agent should |
|---|---|---|
| Defect | `!truncated needs 400×124`, `!overflow needs 1080×1400`, `!clipped by head: bottom 8px`, `!overlaps` | Fix it: it's objectively wrong |
| Advisory | `warn contrast 2.1:1 (WCAG 4.5)` | Judge it |
| Fact | `smallest text: sky 8.4px (footer)`, `upscaled: photo 1.5x` | Decide whether it suits where the design runs |

Defects carry the measurement that fixes them, so the agent corrects them in one step instead of guessing. Taste stays with the model: whether 8 px text is fine print or unreadable depends on the medium, so the server reports the size and doesn't judge it.

Previews are opt-in. They cost more tokens and catch less: in testing, a model approved 7 px text from a 512 px preview, which the measurements catch. `render` also reports how each wrapped, shrunk or cut text was actually drawn. The markers and reply lines are listed in [tools.md](tools.md#replies).

## Reuse

### Tokens and binding

A **token** is a named value (`navy: "#1B2A5C"`) that any field uses as `"{{navy}}"`, or inside text: `"Meet {{name}}"`. The server remembers which fields came from which token, so changing the token changes every field **bound** to it, sentences included. Setting such a field to a value of its own unbinds it.

### Styles and components

A **style** is a named set of layer fields, a look (`card`: fill, border radius, shadow) that layers pull in; changing it changes every layer using it. A **component** is a named layer tree placed by `use` layers, once or once per data row; its **instances** stay linked to it, so one change updates them all.

### Templates and variants

A **template** is a scene file, loaded from a URL or a local path the way an image is. Its tokens are its **variables**: the agent sets them when loading, and `render` can make one **variant** per row of values (a product list, a set of cities), each its own file. Nothing is stored for the template itself; it stays wherever its owner keeps it.

## Motion and video

### Motion that keeps the layout

A scene with a duration moves. Only fields that don't change layout animate (opacity, scale, rotation, offset, skew, blur, color), so the layout is the same at every moment and every check holds throughout. Output is an animated PNG, a GIF, an MP4 or WebM video, or a still of any moment.

### The timeline

Layers **enter** and **exit** with named effects (`fade-up`, `pop`…) and move with **keyframes**, GSAP-style, with GSAP's eases. A frame can **stagger** its children's entrances one after another, and text can **split** into letters or words that move on their own. Strokes can **draw** themselves (a progress ring, a signature), and a number in text can **count** up in a box that holds still.

### Video and shots

A video clip is a layer, drawn like an image under the titles and graphics above it, trimmed, slowed or looped, and its sound carried into the video, mixed with the scene's **soundtrack** if it has one. A scene can be a sequence of **shots**: top-level frames that play one after another, each joined to the one before by a **transition** (fade, slide, push, wipe, zoom) that overlaps the two. Times inside a shot count from its start; layers outside the shots, such as a logo, stay on throughout.

## How an agent works with it

A typical session is four calls:

1. `scene_create` with the sizes (or a template). The reply is the scene id.
2. `asset_add` for each photo or logo, by URL or local path. The reply is its size.
3. `layer_add` with the whole design in one batch. The reply is `ok`, or the defects to fix at each size.
4. `layer_update` to fix what was flagged, addressing layers by **role** so no read is needed first; then `render`.

Every tool is shaped to keep the agent's tokens low: few verb-shaped tools, batches instead of per-property calls, compact text replies that never echo the scene, defaults left out, and measurements instead of images. Icons and shapes come by name, so the model doesn't invent SVG paths.

## Determinism

No model generates pixels: the same scene renders the same design. On the CPU renderer, the same scene gives the same image on a given OS version; glyph edges differ slightly between OS versions. Random-looking effects repeat exactly: grain, torn edges and rough strokes take a `seed` (0 unless given), and `random()` in motion is seeded from each layer's id, so renaming a layer changes its values. The GPU renderer is faster but not bit-exact, so tests and reference images use the CPU.

## Glossary

| Term | Meaning |
|---|---|
| Master size | The size a scene's layers are written at |
| Size | An output the scene renders at: an id, width, height, and optional scale and safe area |
| Preset | A named size for a common format (`instagram-story`, `iab-skyscraper`) |
| Aspect class | `landscape`, `square`, `portrait`, `wide` or `tall`: a group of sizes a layer can change for |
| Safe area | The part of a size a platform covers, where text is flagged |
| Layer | One element of the tree: a frame, text, image, video, shape, icon, spacer, `firstFit` or `use` |
| Role | A semantic name on layers, so an edit can target every layer with it |
| Token | A named value used as `{{name}}` in any field or text |
| Binding | The link from a field to the token it came from |
| Style | A named set of layer fields layers pull in |
| Component | A named layer tree placed by `use` layers |
| Instance | One placement of a component, linked to it |
| Template | A scene file loaded by URL or path, whose tokens are its variables |
| Variant | One render of a scene with one row of token values |
| Defect | A check result that is objectively wrong: overflow, clipping, hidden or overlapping text |
| Advisory | A check result to judge: low contrast |
| Fact | A measurement with no threshold: smallest text, upscaled images |
| Shot | A top-level frame that plays in turn with the others |
| Transition | How a shot enters from the one before |
| Version | The scene's change counter, bumped by every edit |
