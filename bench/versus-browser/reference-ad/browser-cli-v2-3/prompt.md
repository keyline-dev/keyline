# System

You make images. Work autonomously; don't ask questions.

# Task

Build a vote-by-mail flyer at three sizes and produce one PNG per size.
Sizes: portrait 1080×1350 (master), wide 1200×1000 with content at 0.85× the master's scale, sky 300×600 with content at 0.28× scale.
Assets: photo (1600×900 landscape photo), mail (white envelope icon, 56×44 SVG), check (red check-circle icon, 40×40 SVG). Font: Inter.

Work in the current folder (/private/var/folders/hf/35wnqrnj2bbb1_llbg87zlt40000gn/T/vs-24ffb34e3bcc1734): photo.png, mail.svg, check.svg and Inter.ttf are here (load the font with @font-face). Render each size with `playwright screenshot --viewport-size "W,H" file://<abs path> <size>.png`. Save portrait.png, wide.png and sky.png here, at exactly those pixel sizes.

Content, top to bottom:
1. Headline "Proven RESULTS for WILLOWMERE Families", navy #1B2A5C, with RESULTS and WILLOWMERE in red #D0202E.
2. A full-width photo band.
3. Three candidate columns, evenly spaced: Dana Levi (Mayor), Omar Haddad (Council), Ruth Cohen (Council).
4. A full-width red call-to-action bar with the mail icon and "VOTE BY MAIL" in white.
5. Three steps, each with the check icon: "Request your ballot by October 20", "Fill it out at home", "Mail it back by November 3".
6. Footer: "Paid for by Willowmere Forward · willowmereforward.org".

Every size must look right: bands stay full width, columns stay evenly spaced, the photo crops instead of distorting, and no text is cut off, overflowing or overlapping. Check your result and fix every defect before you finish.

# Setup

- tools: `Bash,Write,Edit,Read`
- allowed: `Bash,Write,Edit,Read`
- MCP servers: ``
