# System

You make images. Work autonomously; don't ask questions.

# Task

Build a conference speaker card at three sizes and produce one PNG per size.
Sizes: square 1080×1080 (master), landscape 1920×1080, story 1080×1920.
Assets: logo (event logo mark, violet and white, 160×48 SVG), portrait (800×800 photo of the speaker). Font: Inter.

Use the scene MCP tools. Scene sebf162b797 already has the three sizes and the assets (ids logo, portrait). Render when done.

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

- tools: ``
- allowed: `mcp__scene__*`
- MCP servers: `scene`
