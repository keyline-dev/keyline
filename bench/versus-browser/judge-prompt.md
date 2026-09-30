These PNGs in the current folder are one design at three sizes:

{files}

The brief the design was made from:

{content}

Open each file with Read and fill in this checklist for each one. Judge only what you see in the image; don't guess what was meant.

- `items`: for each content item 1–6, `present` (it appears in the image) and `readable` (its text can be read at this size; for a picture, it can be recognised).
- `cutOrOverflowing`: the numbers of the items that are cut off by an edge or by another element, or that run out of their area.
- `overlapping`: the numbers of the items that overlap another item where they shouldn't.
- `smallestTextLegible`: whether the smallest text in the image can still be read.
{checks}

Answer with one JSON object and nothing else, keyed by file name, in this shape:

{"<file>": {"items": {"1": {"present": true, "readable": true}, "2": {…}, "3": {…}, "4": {…}, "5": {…}, "6": {…}}, "cutOrOverflowing": [], "overlapping": [], "smallestTextLegible": true, {extra}}}
