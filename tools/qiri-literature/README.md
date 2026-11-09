# QIRI Literature MCP

`server.py` is a dependency-free stdio MCP server bundled with QIRI. It exposes:

- `search_literature`: OpenAlex and arXiv search with year filters, abstracts, DOI values, and URLs.
- `fetch_url`: public webpage text retrieval with private-address and redirect checks.

Run it directly with `python3 tools/qiri-literature/server.py`. QIRI registers the installed copy through `~/.qiri-codex/config.toml`.

The current configuration enables only `search_literature` on this server and exposes general `fetch_url` together with `web_search` on the separate `qiri_web` server. The original two-tool catalog remains available for backward compatibility. See [QIRI Web MCP](../qiri-web/README.md) for installation and details.
