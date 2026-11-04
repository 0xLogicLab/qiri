# QIRI Literature MCP

`server.py` is a dependency-free stdio MCP server bundled with QIRI. It exposes:

- `search_literature`: OpenAlex and arXiv search with year filters, abstracts, DOI values, and URLs.
- `fetch_url`: public webpage text retrieval with private-address and redirect checks.

Run it directly with `python3 tools/qiri-literature/server.py`. QIRI registers the installed copy through `~/.qiri-codex/config.toml`.
