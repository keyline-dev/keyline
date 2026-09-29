# Setting up an MCP client

keyline is one program, `keyline-mcp`, that an MCP client starts and talks to over stdio. Install it first, then add it to your client with one of the blocks below; the [Claude Code plugin](#claude-code) installs it for you. Each block names the server `keyline`; the client shows its tools as `keyline`'s.

## Install

Download the file for your machine from the [latest release](https://github.com/keyline-dev/keyline/releases/latest):

- **Linux** (amd64 or arm64): the `.deb`, then `sudo apt install ./keyline-mcp_<version>-1_amd64.deb`. It puts `keyline-mcp` in `/usr/bin`.
- **macOS** (Apple silicon): the `macos-arm64` tarball; unpack it and put `keyline-mcp` on your PATH ([README](../README.md#quick-start) has the commands).

To check a download, compare it with the release's `SHA256SUMS`, or check where it was built with the [GitHub CLI](https://cli.github.com):

```sh
sha256sum --check --ignore-missing SHA256SUMS      # macOS: shasum -a 256 --check --ignore-missing SHA256SUMS
gh attestation verify keyline-mcp_<version>-1_amd64.deb --repo keyline-dev/keyline
```

Check that it runs: `keyline-mcp --help`. If your client can't find it, use its full path (`which keyline-mcp`) as the command below.

## Clients

Client config files move between versions, so each section links the client's own guide.

### Claude Code

Install the plugin, which needs no separate install: it uses `keyline-mcp` from the PATH if it's there, and otherwise downloads the matching release once and checks it against `SHA256SUMS`.

```text
/plugin marketplace add keyline-dev/keyline
/plugin install keyline@keyline
```

Or, with `keyline-mcp` installed, add the server yourself:

```sh
claude mcp add keyline -- keyline-mcp
```

Add `--scope user` to use it in every project. ([guide](https://code.claude.com/docs/en/mcp))

### Claude Desktop

Settings → Developer → Edit Config opens `claude_desktop_config.json`; add:

```json
{
  "mcpServers": {
    "keyline": { "command": "/usr/local/bin/keyline-mcp" }
  }
}
```

Claude Desktop doesn't search your shell's PATH, so give the full path. Restart it afterwards. ([guide](https://modelcontextprotocol.io/quickstart/user))

### Cursor

[Add to Cursor](cursor://anysphere.cursor-deeplink/mcp/install?name=keyline&config=eyJjb21tYW5kIjoia2V5bGluZS1tY3AifQ==), or add to `~/.cursor/mcp.json` (every project) or `.cursor/mcp.json` (one project):

```json
{
  "mcpServers": {
    "keyline": { "command": "keyline-mcp" }
  }
}
```

([guide](https://cursor.com/docs/context/mcp))

### VS Code

```sh
code --add-mcp '{"name":"keyline","command":"keyline-mcp"}'
```

Or add to `.vscode/mcp.json` in a project, or to your profile's with the command *MCP: Open User Configuration*. VS Code's key is `servers`, not `mcpServers`:

```json
{
  "servers": {
    "keyline": { "type": "stdio", "command": "keyline-mcp" }
  }
}
```

([guide](https://code.visualstudio.com/docs/copilot/customization/mcp-servers))

### Windsurf

Add to `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "keyline": { "command": "keyline-mcp" }
  }
}
```

([guide](https://docs.windsurf.com/windsurf/cascade/mcp))

### Cline

In Cline's MCP Servers panel, open the installed servers' settings (`cline_mcp_settings.json`) and add:

```json
{
  "mcpServers": {
    "keyline": { "command": "keyline-mcp" }
  }
}
```

([guide](https://docs.cline.bot/mcp/configuring-mcp-servers))

### Any other client

Any MCP client that starts stdio servers works: the command is `keyline-mcp`, with no environment variables. The server speaks MCP over stdio, so it runs where the client runs; to render on another machine, make the command `ssh that-machine keyline-mcp`.

## Options

Every setting is a flag, added after the command. In a JSON config they go in `args`; with `claude mcp add`, after `keyline-mcp`:

```json
{
  "mcpServers": {
    "keyline": {
      "command": "keyline-mcp",
      "args": ["--allow-read", "/Users/me/brand", "--data", "/Users/me/.keyline"]
    }
  }
}
```

The one most setups want is `--allow-read <folder>`: it lets the agent add images and templates by path, so their bytes never pass through the model. [tools.md](tools.md#server-configuration) lists every flag.
