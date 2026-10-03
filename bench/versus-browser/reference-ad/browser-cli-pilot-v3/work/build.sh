#!/bin/zsh
set -e
cd "$(dirname "$0")"
D="$PWD"
PW=/Users/yuvalt/personal/visual-renderer/bench/versus-browser/tooling/node_modules/.bin/playwright

render() { # name W H scale
  sed "s/--s:1;/--s:$4;/" flyer.html > "$1.html"
  "$PW" screenshot --viewport-size "$2,$3" "file://$D/$1.html" "$1.png" >/dev/null 2>&1
}

render portrait 1080 1350 1
render wide     1200 1000 0.85
render sky       300  600 0.28

for f in portrait wide sky; do
  printf "%s: " "$f"
  python3 -c "
import struct,sys
d=open('$f.png','rb').read()
w,h=struct.unpack('>II',d[16:24]); print(w,'x',h)"
done
