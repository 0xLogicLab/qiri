# QIRI Web MCP

General web tools, separate from the scientific literature catalog:

| Server | Enabled tools | Use |
| --- | --- | --- |
| `qiri_literature` | `search_literature` | OpenAlex/arXiv papers, authors, DOI, abstracts |
| `qiri_web` | `web_search`, `fetch_url` | Discover webpages, then read their text |

`web_search` integrates [DDGS](https://github.com/deedy5/ddgs) (MIT), an open source metasearch library. No API key or additional search server is required. DDGS selects from Yahoo, Brave, DuckDuckGo, Startpage, and Mojeek, the general web engines available in the pinned version. Search availability, time filtering, and freshness depend on those engines; this is live querying of search indexes, not guaranteed instant discovery. Errors and empty results are reported explicitly, never replaced with generated results. Proxy precedence is `DDGS_PROXY`, `https_proxy`, `HTTPS_PROXY`, `http_proxy`, `HTTP_PROXY`; the existing QIRI launcher already exports its proxy settings.

Inputs: `query`, `limit` (1–20, default 8), `region` (default `us-en`; e.g. `cn-zh`), optional `timelimit` (`d`, `w`, `m`, `y`), and `safesearch` (`on`, `moderate`, `off`). Locale, time, and safe-search options are passed to the engines; support varies by engine and they are not strict guarantees. Output includes title, URL, snippet, provider, and UTC retrieval time. Retrieval time is not the page publication time. Use `fetch_url` to verify sources; it retrieves static HTML/text and does not render JavaScript or parse PDFs.

Install from the repository:

```bash
bash tools/qiri-web/install.sh
```

Copy the `mcp_servers.qiri_web` table from `tools/qiri-launcher/config.toml` into your QIRI config. Set the literature server's `enabled_tools = ["search_literature"]` to expose `fetch_url` only once. Restart QIRI sessions after editing config.

The servers share the stdio MCP transport and existing webpage extractor in `tools/qiri-literature/server.py`. Running that script with `--web` exposes only `web_search` and `fetch_url`. The original invocation retains its old two-tool catalog for compatibility; the updated QIRI configuration selects only `search_literature` there. The web dependency is imported only during search, so literature search remains dependency-free.

Deterministic tests (no external queries):

```bash
python3 -m unittest discover -s tools/qiri-web -p 'test_*.py'
```

For a future deployment needing a dedicated search service, [SearXNG](https://github.com/searxng/searxng) (AGPL-3.0) is another option. It needs an instance with the JSON search API enabled; it is not deployed by this integration.

Example tool arguments:

```json
{"query": "人工智能 最新 新闻", "region": "cn-zh", "timelimit": "w", "limit": 5}
```

Local verification on 2026-11-09: the installed stdio MCP returned real results for Chinese AI news and English climate news with a week filter; `fetch_url` read the Python 3.14 release page. Some Python search queries returned upstream empty results, which correctly became MCP tool errors. Five deterministic tests cover protocol/catalog isolation, normalization/deduplication and proxy forwarding, invalid inputs, upstream failures, and private URL rejection. `qiri mcp list` lists both servers as enabled.
