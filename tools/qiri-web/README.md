# QIRI Web MCP

General web tools, separate from the scientific literature catalog:

| Server | Enabled tools | Use |
| --- | --- | --- |
| `qiri_literature` | `search_literature` | OpenAlex/arXiv papers, authors, DOI, abstracts |
| `qiri_web` | `web_search`, `fetch_url` | Discover webpages, then read their text |

`web_search` uses the authenticated [Keenable Search API](https://docs.keenable.ai/api-reference/search.md), `POST https://api.keenable.ai/v1/search`. It uses Python's standard library; no DDGS package or search scraping is needed. Requests disable Keenable's generated cards and return ordinary source webpages. Errors and empty results are reported explicitly, never replaced with generated results.

## Install and configure

```bash
bash tools/qiri-web/install.sh
```

Copy the `mcp_servers.qiri_web` table from `tools/qiri-launcher/config.toml` into your QIRI config. Set the literature server's `enabled_tools = ["search_literature"]` to expose `fetch_url` only once. Restart QIRI after editing config.

Set `KEENABLE_API_KEY` in the launching environment (the QIRI launcher also exports values from `~/.qiri/.env`), or configure it in your private local MCP configuration:

```toml
[mcp_servers.qiri_web.env]
KEENABLE_API_KEY = "YOUR_API_KEY"
```

Keep credential files readable only by their owner and outside the repository. The key is required: missing credentials produce a clear error, with no fallback to the public API. The supplied template's `env_vars` explicitly forwards `KEENABLE_API_KEY` and HTTP(S) proxy settings. MCP does **not** inherit these variables by default. HTTP requests honor standard Python/urllib proxy variables (`https_proxy`/`HTTPS_PROXY`, `http_proxy`/`HTTP_PROXY`, `no_proxy`/`NO_PROXY`); there is no hardcoded proxy port.

The API request timeout is 30 seconds; the MCP tool timeout is 60 seconds. HTTP errors include status, provider error code and a bounded, credential-redacted message. Authentication failures (401/403), exhausted credits (402), rate limits (429, including `Retry-After`) and network timeouts are distinguished. The integration performs no automatic retries or fallback requests; callers can respect the reported delay for a retry.

## Search arguments and results

- `query`: required search text.
- `limit`: 1–20, default 8.
- `timelimit`: optional `d`, `w`, `m`, `y`, mapped to `published_after` values `1d`, `7d`, `1mo`, `1y`. These filter **publication** dates and exclude pages with unknown publication dates.
- `site`: optional site restriction, such as `nvidia.com`.
- `mode`: `realtime` (default, fast) or `pro` (deeper retrieval). Modes are billed according to Keenable's pricing.

Keenable has no `region` or `safesearch` API filters. Those options are removed from the advertised schema. Calls using an older schema remain accepted, with supplied unsupported options explicitly listed in `unsupported_parameters`; they are not sent to Keenable.

Output includes title, URL, snippet, provider (`keenable`), served mode and UTC retrieval time, plus publication/indexing timestamps when supplied by Keenable. Retrieval time is not the publication time. `fetch_url` retains the existing static HTML/text extractor; it does not render JavaScript or parse PDFs.

```json
{"query": "黄仁勋 最新新闻", "timelimit": "d", "limit": 3}
```

The servers share the stdio MCP transport and webpage extractor in `tools/qiri-literature/server.py`. Running it with `--web` exposes only `web_search` and `fetch_url`. The original invocation retains its old two-tool catalog for compatibility.

## Verification

```bash
python3 -m unittest discover -s tools/qiri-web -p 'test_*.py'
```

Tests cover the MCP protocol/catalog, authenticated request shape, publication filters, normalization/deduplication, legacy options, malformed responses, missing credentials, HTTP errors, credential redaction, network errors, API redirect rejection and private webpage URL rejection.

Local verification on 2026-11-09: all 10 deterministic tests passed. The installed stdio MCP, launched with the local MCP credential/proxy configuration, returned 3 results for a Chinese Jensen Huang news query with the last-day publication filter in approximately 0.49 seconds (`provider: keenable`, `mode: realtime`).
