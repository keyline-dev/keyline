# keyline

keyline is a design engine for AI agents. Claude describes a design once as a small JSON scene, and keyline renders it to PNG, JPEG, WebP, vector PDF, GIF, animated PNG, MP4 or WebM at every size you ask for: a square post, a landscape banner and a tall story from one layout. Before rendering, it measures the design and reports text that overflows, is cut off or has too little contrast, so Claude fixes problems without looking at pixels.

keyline is a design tool, not an image generator: no model draws pixels. The same scene always renders the same image, and photos and logos are your own files.

## Try it

- "Make an Instagram post and a 1200×628 banner for a summer cold-brew promo: headline, price, a Shop now button."
- "Design a speaker card for Dana Levi, talk title 'Shipping Rust on the GPU', in square and story sizes, as PNG."
- "Make a 6-second MP4 flyer for a Saturday farmers' market where the headline slides in and the date pulses, at 1080×1920 and 1080×1080."

## What the plugin runs

The plugin adds one MCP server, `keyline`, started by `scripts/launch.sh`:

- If `keyline-mcp` is on your PATH (Homebrew, `.deb`, or a release download), it runs that.
- Otherwise it downloads the release matching this plugin's version, once, from https://github.com/keyline-dev/keyline/releases into the plugin's data folder, checks it against the release's `SHA256SUMS`, and refuses to start it if the checksum doesn't match. The download is a compiled Rust binary built by the project's release workflow, with build provenance attestations; its source is https://github.com/keyline-dev/keyline.
- Releases exist for macOS on Apple silicon and Linux (x64 and Arm). The plugin doesn't run on Windows; there, install `keyline-mcp` from the main README and add it with `claude mcp add keyline -- keyline-mcp`.

## What it reads and sends

- Scenes, images and renders are files in keyline's local data folder. It reads other local files only in folders you allow with `--allow-read`.
- It downloads Google Fonts the first time a design uses one, and images or templates from URLs Claude passes it. Nothing else leaves your machine: no telemetry, no account.

Privacy policy: https://keyline.dev/privacy/ · Security and support: https://keyline.dev/security/ · Docs: https://keyline.dev/docs/

## Troubleshooting

- **"no release for …"**: your OS or CPU has no release; build `keyline-mcp` from source (`cargo build --release`) and put it on your PATH.
- **The checksum doesn't match**: the download was incomplete or altered; restart Claude Code to try again.
- **Video fails**: MP4 and WebM need `ffmpeg` on your PATH (`brew install ffmpeg`, `apt install ffmpeg`). Stills, GIF and animated PNG don't.

## License

Functional Source License 1.1, Apache 2.0 future (FSL-1.1-ALv2): https://keyline.dev/license/
