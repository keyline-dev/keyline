# System

You make images. Work autonomously; don't ask questions.

# Task

Build a conference speaker card at three sizes and produce one PNG per size.
Sizes: square 1080×1080 (master), landscape 1920×1080, story 1080×1920.
Assets: logo (event logo mark, violet and white, 160×48 SVG), portrait (800×800 photo of the speaker). Font: Inter.

Work in the current folder (/private/var/folders/hf/35wnqrnj2bbb1_llbg87zlt40000gn/T/vs-de6ac88e60484b87): logo.svg, portrait.png and Inter.ttf are here (load the font with @font-face). The folder is served at http://localhost:59822/. Render each size with the Playwright MCP browser tools and save square.png, landscape.png and story.png in this folder, at exactly those pixel sizes.

Content:
1. The event logo, with the event name "FIELDNOTES CONF 2026" beside it.
2. The speaker's portrait, cropped to a circle.
3. The speaker's name "Maya Okonkwo-Lindqvist" and role "Principal Engineer, Northwind Labs".
4. The talk title, large: "Shipping at the speed of trust: what ten years of on-call taught us about resilient systems".
5. Date and venue: "November 14, 2026 · Harbor Hall, Lisbon".
6. A pill-shaped "Get tickets" button.
Colors: background #0F1226, text white, accent violet #7C5CFF.

Every size must look right: the portrait stays a circle, the long title wraps instead of being cut off, and no text is cut off, overflowing or overlapping. Check your result and fix every defect before you finish.

# Setup

- tools: `Write,Edit,Read`
- allowed: `mcp__playwright__*,Write,Edit,Read`
- MCP servers: `playwright`
