# YZW Biology MCP

`server.py` is the domain MCP server for the Yazhou Bay agricultural and
biological breeding agent. It uses the verified internal services through the
local center51 SOCKS5 tunnel and exposes:

- agricultural literature search with TagP-first Graph v1 fallback;
- concept, gene, species, germplasm, and variation normalization;
- paper metadata, annotations, related entities, and Markdown resources;
- semantic similarity through the site's `bge-m3` vector service;
- TagP and vector-service health checks.

The server deliberately does not expose the currently unavailable Semantic
Scholar v2 routes, QA upstream, or disabled NER endpoint.

## Runtime configuration

```text
YZW_DATA_API_KEY       API key value (preferred for managed environments)
YZW_DATA_API_KEY_FILE  key file, default ~/.yzw/api.txt
YZW_API_ORIGIN         API origin, default http://10.1.16.2:8080
YZW_SOCKS_PROXY        SOCKS5 proxy, default 127.0.0.1:1080
YZW_API_TIMEOUT_SEC    request timeout, default 45
```

The SOCKS5 tunnel must be running before tools are called. Run the server
directly with `python3 tools/yzw-bio/server.py` or install it as
`yzw-bio-mcp` through `tools/yzw-launcher/install.sh`.
