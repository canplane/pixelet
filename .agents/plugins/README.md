# Plugins

A Claude Code marketplace of the repository's own plugins.

- `deno-lsp` reads TypeScript through `deno lsp`, with both workspaces' import
  maps (`ts/`, `xpute/ts/`) resolved.

Claude Code takes them from a checkout's own `.claude/settings.json`, which
registers the marketplace (`./.agents/plugins`) and enables `deno-lsp@pixelet`
with `typescript-lsp@claude-plugins-official` off. A session picks up a language
server only when it starts.
