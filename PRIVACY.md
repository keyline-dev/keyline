# Privacy

keyline runs on your machine and collects nothing: no accounts, no telemetry, no analytics, and no data sent to us.

## What stays on your machine

Scenes, the images you add, renders and the font cache are files in keyline's data folder (`~/.keyline-mcp`, or the folder given with `--data`; the Claude Code plugin uses its own data folder). They stay there until you delete them. keyline reads other local files only inside the folders you allow with `--allow-read`.

## What keyline fetches

keyline connects to the network only for these:

- **Google Fonts** (`fonts.googleapis.com`, `fonts.gstatic.com`): when a design uses a font that isn't installed, keyline downloads it once and caches it. The request names the font family; Google sees your IP address. [Google's privacy policy](https://policies.google.com/privacy) applies.
- **URLs the agent gives it**: an image, video, sound or template added by URL is downloaded from that address. Private and local addresses are refused.
- **GitHub** (`github.com`): the Claude Code plugin, when `keyline-mcp` isn't installed, downloads the release matching its version once from GitHub Releases and checks it against the release's `SHA256SUMS`. [GitHub's privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement) applies.

Nothing about your designs is sent anywhere: renders are drawn locally, and no model generates pixels.

## The website

keyline.dev is a static site on Cloudflare Pages, with no cookies and no analytics. Pages load fonts from Google Fonts, and the home page reads the latest release from GitHub's API.

## Contact

Questions about privacy: open an issue at https://github.com/keyline-dev/keyline/issues, or see [SECURITY.md](SECURITY.md) to report something privately.
