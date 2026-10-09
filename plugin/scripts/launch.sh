#!/bin/sh
# Starts keyline-mcp for the Claude Code plugin: the one on the PATH if it's
# installed, else the release matching this plugin's version, downloaded
# once into the plugin's data folder and checked against the release's
# SHA256SUMS. Arguments pass through. stdout is the MCP channel, so every
# message goes to stderr.
set -eu

# Claude Code starts it in the project: that's the workspace, where designs
# and their renders go. Not the home folder or /, which would open the
# whole disk; there keyline uses ~/keyline.
case "$PWD" in
    / | "$HOME") ;;
    *) set -- --folder "$PWD" "$@" ;;
esac

if command -v keyline-mcp >/dev/null 2>&1; then
    exec keyline-mcp "$@"
fi

root=${CLAUDE_PLUGIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}
data=${CLAUDE_PLUGIN_DATA:-${XDG_CACHE_HOME:-$HOME/.cache}/keyline-mcp}
version=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$root/.claude-plugin/plugin.json" | head -n 1)
# ponytail: tests point this at a local copy of a release.
releases=${KEYLINE_RELEASES:-https://github.com/keyline-dev/keyline/releases/download}

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) target=linux-amd64 ;;
    Linux-aarch64 | Linux-arm64) target=linux-arm64 ;;
    Darwin-arm64) target=macos-arm64 ;;
    *)
        echo "keyline: no release for $(uname -s) $(uname -m); build keyline-mcp from source and put it on the PATH" >&2
        exit 1
        ;;
esac

bin=$data/$version/keyline-mcp
if [ ! -x "$bin" ]; then
    name=keyline-mcp-v$version-$target
    tmp=$(mktemp -d)
    echo "keyline: downloading $name" >&2
    curl -fsSL "$releases/v$version/$name.tar.gz" -o "$tmp/$name.tar.gz"
    curl -fsSL "$releases/v$version/SHA256SUMS" -o "$tmp/SHA256SUMS"
    if command -v sha256sum >/dev/null 2>&1; then sum="sha256sum"; else sum="shasum -a 256"; fi
    if ! (cd "$tmp" && grep " $name.tar.gz\$" SHA256SUMS >want && $sum -c want >/dev/null 2>&1); then
        echo "keyline: $name.tar.gz doesn't match the release's SHA256SUMS; not starting it" >&2
        rm -rf "$tmp"
        exit 1
    fi
    tar xzf "$tmp/$name.tar.gz" -C "$tmp"
    mkdir -p "$data/$version"
    mv "$tmp/$name/keyline-mcp" "$bin"
    rm -rf "$tmp"
fi
exec "$bin" "$@"
