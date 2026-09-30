# The keyline MCP server over stdio:
#   docker run -i --rm -v keyline:/data ghcr.io/keyline-dev/keyline-mcp
# The release workflow builds it from the binaries it already made, placed
# in bin/<arch>/ (amd64, arm64); nothing is compiled here. VIDEO=false
# leaves ffmpeg out (the -stills tags): about 220 MB instead of 760.
FROM debian:bookworm-slim

ARG VIDEO=true
# Fonts and text (fontconfig, FreeType), the Vulkan loader for a GPU passed
# through (without one, renders use the CPU), the CA certificates for web
# fonts and asset URLs, and ffmpeg for video.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates libfontconfig1 libfreetype6 libvulkan1 \
        $([ "$VIDEO" = true ] && echo ffmpeg) \
    && rm -rf /var/lib/apt/lists/*

ARG TARGETARCH
COPY bin/${TARGETARCH}/keyline-mcp /usr/local/bin/keyline-mcp

LABEL org.opencontainers.image.source="https://github.com/keyline-dev/keyline" \
      org.opencontainers.image.description="Design engine for AI agents: images and video at every size, no Chrome needed" \
      org.opencontainers.image.licenses="PolyForm-Shield-1.0.0" \
      io.modelcontextprotocol.server.name="io.github.keyline-dev/keyline"

VOLUME /data
ENTRYPOINT ["keyline-mcp", "--data", "/data"]
