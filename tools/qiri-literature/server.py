#!/usr/bin/env python3
"""QIRI literature MCP server and shared --web catalog/transport."""

from __future__ import annotations

import html
import ipaddress
import json
import os
import re
import socket
import sys
from datetime import datetime, timezone
import xml.etree.ElementTree as ET
from html.parser import HTMLParser
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode, urljoin, urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener


SERVER_NAME = "qiri-literature"
SERVER_VERSION = "0.1.0"
PROTOCOL_VERSION = "2025-06-18"
USER_AGENT = "QIRI/0.1 (scientific literature research)"
MAX_FETCH_BYTES = 1_000_000
MAX_FETCH_TEXT = 30_000
WEB_MODE = False


class ToolError(Exception):
    pass


class _TextExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self.hidden_depth = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag in {"script", "style", "noscript", "svg"}:
            self.hidden_depth += 1
        elif tag in {"p", "br", "div", "li", "section", "article", "h1", "h2", "h3"}:
            self.parts.append("\n")

    def handle_endtag(self, tag: str) -> None:
        if tag in {"script", "style", "noscript", "svg"} and self.hidden_depth:
            self.hidden_depth -= 1
        elif tag in {"p", "div", "li", "section", "article"}:
            self.parts.append("\n")

    def handle_data(self, data: str) -> None:
        if not self.hidden_depth:
            self.parts.append(data)

    def text(self) -> str:
        value = html.unescape("".join(self.parts))
        value = re.sub(r"[ \t\r\f\v]+", " ", value)
        value = re.sub(r"\n\s*\n+", "\n\n", value)
        return value.strip()


def _json_response(request_id: Any, result: dict[str, Any]) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "id": request_id, "result": result}


def _json_error(request_id: Any, code: int, message: str) -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "error": {"code": code, "message": message},
    }


def _emit(message: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(message, ensure_ascii=False, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def _http_get(url: str, params: dict[str, Any] | None = None) -> tuple[bytes, str, str]:
    if params:
        url = f"{url}?{urlencode(params)}"
    request = Request(url, headers={"User-Agent": USER_AGENT, "Accept": "application/json, application/xml, text/xml, text/html, text/plain;q=0.9, */*;q=0.1"})
    try:
        with build_opener().open(request, timeout=45) as response:
            data = response.read(MAX_FETCH_BYTES + 1)
            if len(data) > MAX_FETCH_BYTES:
                raise ToolError(f"Response exceeded {MAX_FETCH_BYTES} bytes")
            return data, response.headers.get_content_type(), response.geturl()
    except HTTPError as exc:
        raise ToolError(f"HTTP {exc.code} from {url}") from exc
    except URLError as exc:
        raise ToolError(f"Request failed for {url}: {exc.reason}") from exc


def _reconstruct_abstract(index: dict[str, list[int]] | None) -> str | None:
    if not index:
        return None
    words: list[tuple[int, str]] = []
    for word, positions in index.items():
        words.extend((position, word) for position in positions)
    return " ".join(word for _, word in sorted(words))


def _normalize_doi(value: str | None) -> str | None:
    if not value:
        return None
    return re.sub(r"^https?://(?:dx\.)?doi\.org/", "", value.strip(), flags=re.I) or None


def _openalex(query: str, limit: int, year_from: int | None, year_to: int | None) -> list[dict[str, Any]]:
    filters: list[str] = []
    if year_from is not None:
        filters.append(f"from_publication_date:{year_from}-01-01")
    if year_to is not None:
        filters.append(f"to_publication_date:{year_to}-12-31")
    params: dict[str, Any] = {
        "search": query,
        "per-page": limit,
        "select": "id,doi,title,publication_year,publication_date,type,authorships,primary_location,cited_by_count,abstract_inverted_index,open_access",
    }
    if filters:
        params["filter"] = ",".join(filters)
    data, _, _ = _http_get("https://api.openalex.org/works", params)
    try:
        results = json.loads(data).get("results", [])
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ToolError(f"Could not parse OpenAlex response: {exc}") from exc

    works: list[dict[str, Any]] = []
    for item in results:
        authors = [
            authorship.get("author", {}).get("display_name")
            for authorship in item.get("authorships", [])[:12]
        ]
        location = item.get("primary_location") or {}
        source = location.get("source") or {}
        doi = _normalize_doi(item.get("doi"))
        abstract = _reconstruct_abstract(item.get("abstract_inverted_index"))
        works.append(
            {
                "source": "OpenAlex",
                "title": item.get("title"),
                "authors": [author for author in authors if author],
                "year": item.get("publication_year"),
                "date": item.get("publication_date"),
                "type": item.get("type"),
                "venue": source.get("display_name"),
                "doi": doi,
                "url": location.get("landing_page_url") or (f"https://doi.org/{doi}" if doi else item.get("id")),
                "open_access_url": location.get("pdf_url"),
                "cited_by_count": item.get("cited_by_count"),
                "abstract": re.sub(r"\s+", " ", abstract).strip()[:3000] if abstract else None,
            }
        )
    return works


def _arxiv(query: str, limit: int, year_from: int | None, year_to: int | None) -> list[dict[str, Any]]:
    data, _, _ = _http_get(
        "https://export.arxiv.org/api/query",
        {
            "search_query": f"all:{query}",
            "start": 0,
            "max_results": limit,
            "sortBy": "relevance",
            "sortOrder": "descending",
        },
    )
    try:
        root = ET.fromstring(data)
    except ET.ParseError as exc:
        raise ToolError(f"Could not parse arXiv response: {exc}") from exc
    ns = {
        "a": "http://www.w3.org/2005/Atom",
        "arxiv": "http://arxiv.org/schemas/atom",
    }
    works: list[dict[str, Any]] = []
    for entry in root.findall("a:entry", ns):
        published = entry.findtext("a:published", default="", namespaces=ns)
        year = int(published[:4]) if published[:4].isdigit() else None
        if year_from is not None and (year is None or year < year_from):
            continue
        if year_to is not None and (year is None or year > year_to):
            continue
        entry_id = entry.findtext("a:id", default="", namespaces=ns)
        authors = [
            author.findtext("a:name", default="", namespaces=ns)
            for author in entry.findall("a:author", ns)
        ]
        pdf_url = None
        for link in entry.findall("a:link", ns):
            if link.attrib.get("title") == "pdf":
                pdf_url = link.attrib.get("href")
        doi = _normalize_doi(entry.findtext("arxiv:doi", default="", namespaces=ns))
        works.append(
            {
                "source": "arXiv",
                "title": " ".join(entry.findtext("a:title", default="", namespaces=ns).split()),
                "authors": [author for author in authors if author],
                "year": year,
                "date": published[:10] or None,
                "type": "preprint",
                "venue": "arXiv",
                "doi": doi,
                "url": entry_id,
                "open_access_url": pdf_url,
                "cited_by_count": None,
                "abstract": " ".join(entry.findtext("a:summary", default="", namespaces=ns).split())[:3000],
            }
        )
    return works


def search_literature(arguments: dict[str, Any]) -> dict[str, Any]:
    query = str(arguments.get("query", "")).strip()
    if not query:
        raise ToolError("query must not be empty")
    source = str(arguments.get("source", "both")).lower()
    if source not in {"openalex", "arxiv", "both"}:
        raise ToolError("source must be openalex, arxiv, or both")
    try:
        limit = min(max(int(arguments.get("limit", 8)), 1), 25)
        year_from = int(arguments["year_from"]) if arguments.get("year_from") is not None else None
        year_to = int(arguments["year_to"]) if arguments.get("year_to") is not None else None
    except (TypeError, ValueError) as exc:
        raise ToolError("limit and years must be integers") from exc
    for year in (year_from, year_to):
        if year is not None and not 1000 <= year <= 2100:
            raise ToolError("years must be between 1000 and 2100")
    if year_from is not None and year_to is not None and year_from > year_to:
        raise ToolError("year_from must not be later than year_to")

    works: list[dict[str, Any]] = []
    if source in {"openalex", "both"}:
        works.extend(_openalex(query, limit, year_from, year_to))
    if source in {"arxiv", "both"}:
        works.extend(_arxiv(query, limit, year_from, year_to))

    deduplicated: list[dict[str, Any]] = []
    seen: set[str] = set()
    for work in works:
        key = (work.get("doi") or work.get("title") or work.get("url") or "").casefold()
        if key and key in seen:
            continue
        if key:
            seen.add(key)
        deduplicated.append(work)
    return {
        "query": query,
        "source": source,
        "year_from": year_from,
        "year_to": year_to,
        "count": len(deduplicated),
        "works": deduplicated,
    }


def _validate_public_url(url: str) -> None:
    parsed = urlsplit(url)
    if parsed.scheme not in {"http", "https"} or not parsed.hostname:
        raise ToolError("url must be an absolute http or https URL")
    if parsed.username or parsed.password:
        raise ToolError("credential-bearing URLs are not allowed")
    port = parsed.port or (443 if parsed.scheme == "https" else 80)
    try:
        addresses = socket.getaddrinfo(parsed.hostname, port, type=socket.SOCK_STREAM)
    except socket.gaierror as exc:
        raise ToolError(f"could not resolve {parsed.hostname}: {exc}") from exc
    for address in {item[4][0] for item in addresses}:
        ip = ipaddress.ip_address(address.split("%", 1)[0])
        if not ip.is_global:
            raise ToolError(f"url resolves to a non-public address: {ip}")


class _SafeRedirectHandler(HTTPRedirectHandler):
    def redirect_request(self, req: Request, fp: Any, code: int, msg: str, headers: Any, newurl: str) -> Request | None:
        absolute = urljoin(req.full_url, newurl)
        _validate_public_url(absolute)
        return super().redirect_request(req, fp, code, msg, headers, absolute)


def fetch_url(arguments: dict[str, Any]) -> dict[str, Any]:
    url = str(arguments.get("url", "")).strip()
    _validate_public_url(url)
    request = Request(url, headers={"User-Agent": USER_AGENT, "Accept": "text/html, text/plain, application/json, application/xml;q=0.9"})
    try:
        with build_opener(_SafeRedirectHandler()).open(request, timeout=45) as response:
            data = response.read(MAX_FETCH_BYTES + 1)
            if len(data) > MAX_FETCH_BYTES:
                raise ToolError(f"Response exceeded {MAX_FETCH_BYTES} bytes")
            content_type = response.headers.get_content_type()
            if not (
                content_type.startswith("text/")
                or content_type in {"application/json", "application/xml", "application/xhtml+xml"}
            ):
                raise ToolError(f"unsupported content type: {content_type}")
            charset = response.headers.get_content_charset() or "utf-8"
            text = data.decode(charset, errors="replace")
            if content_type in {"text/html", "application/xhtml+xml"}:
                parser = _TextExtractor()
                parser.feed(text)
                text = parser.text()
            else:
                text = text.strip()
            return {
                "url": url,
                "final_url": response.geturl(),
                "content_type": content_type,
                "truncated": len(text) > MAX_FETCH_TEXT,
                "text": text[:MAX_FETCH_TEXT],
            }
    except HTTPError as exc:
        raise ToolError(f"HTTP {exc.code} from {url}") from exc
    except URLError as exc:
        raise ToolError(f"Request failed for {url}: {exc.reason}") from exc


class _KeenableRedirectHandler(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # Never forward the API key to a redirect destination.
        raise ToolError("Keenable API returned an unexpected redirect")


def _keenable_search(payload: dict[str, Any]) -> dict[str, Any]:
    api_key = os.environ.get("KEENABLE_API_KEY", "").strip()
    if not api_key:
        raise ToolError("KEENABLE_API_KEY is not configured for the qiri_web MCP server")
    request = Request(
        "https://api.keenable.ai/v1/search",
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"X-API-Key": api_key, "X-Keenable-Title": "QIRI",
                 "Content-Type": "application/json", "Accept": "application/json",
                 "User-Agent": USER_AGENT},
        method="POST",
    )
    try:
        with build_opener(_KeenableRedirectHandler()).open(request, timeout=30) as response:
            data = response.read(MAX_FETCH_BYTES + 1)
            if len(data) > MAX_FETCH_BYTES:
                raise ToolError("Keenable response exceeded the size limit")
    except HTTPError as exc:
        try:
            error = json.loads(exc.read(65536))
        except (ValueError, UnicodeError):
            error = {}
        if not isinstance(error, dict):
            error = {}
        code = str(error.get("code", ""))[:100]
        detail = str(error.get("message") or error.get("error") or exc.reason)[:500]
        hints = {
            401: "Check KEENABLE_API_KEY in the MCP server configuration.",
            403: "Check the API key and account permissions.",
            402: "Keenable credits exhausted; add credits before retrying.",
            429: "Rate limited; retry after the indicated delay.",
        }
        retry_after = exc.headers.get("Retry-After") if exc.headers else None
        message = f"Keenable HTTP {exc.code} ({code}): {detail}. {hints.get(exc.code, '')}"
        if retry_after:
            message += f" Retry-After: {retry_after}."
        # Do not expose credentials, even if an upstream error echoes a header.
        message = re.sub(r"keen_[A-Za-z0-9_]+", "[redacted]", message.replace(api_key, "[redacted]"))
        raise ToolError(message.strip()) from exc
    except (TimeoutError, socket.timeout) as exc:
        raise ToolError("Keenable connection/request timed out after 30 seconds; check network and proxy settings") from exc
    except URLError as exc:
        if isinstance(exc.reason, TimeoutError):
            raise ToolError("Keenable connection/request timed out after 30 seconds; check network and proxy settings") from exc
        raise ToolError("Cannot connect to Keenable; check DNS, TLS and the MCP proxy environment") from exc
    except OSError as exc:
        raise ToolError("Keenable network request failed; check the MCP proxy environment") from exc
    try:
        response = json.loads(data)
    except (ValueError, UnicodeError) as exc:
        raise ToolError("Keenable returned invalid JSON") from exc
    if not isinstance(response, dict) or not isinstance(response.get("results"), list):
        raise ToolError("Keenable returned an invalid search response (missing results array)")
    return response


def web_search(arguments: dict[str, Any]) -> dict[str, Any]:
    query = arguments.get("query")
    if not isinstance(query, str) or not query.strip():
        raise ToolError("query must be a non-empty string")
    limit = arguments.get("limit", 8)
    if type(limit) is not int or not 1 <= limit <= 20:
        raise ToolError("limit must be an integer between 1 and 20")
    timelimit = arguments.get("timelimit")
    if timelimit not in (None, "d", "w", "m", "y"):
        raise ToolError("timelimit must be d, w, m, or y")
    mode = arguments.get("mode", "realtime")
    if mode not in ("pro", "realtime"):
        raise ToolError("mode must be pro or realtime")
    site = arguments.get("site")
    if site is not None and (not isinstance(site, str) or not site.strip()):
        raise ToolError("site must be a non-empty string")
    # Accept calls made with the old tool schema, but report unsupported options.
    region = arguments.get("region")
    if region is not None and (not isinstance(region, str) or not re.fullmatch(r"[a-z]{2}-[a-z]{2}", region)):
        raise ToolError("region must be a region code such as cn-zh or us-en")
    if arguments.get("safesearch", "moderate") not in ("on", "moderate", "off"):
        raise ToolError("safesearch must be on, moderate, or off")
    payload = {"query": query.strip(), "max_results": limit, "mode": mode,
               "snippet_max_length": 3000, "disable_cards": True}
    if timelimit:
        payload["published_after"] = {"d": "1d", "w": "7d", "m": "1mo", "y": "1y"}[timelimit]
    if site is not None:
        payload["site"] = site.strip()
    response = _keenable_search(payload)
    results = []
    seen: set[str] = set()
    for item in response["results"]:
        if not isinstance(item, dict):
            continue
        url = item.get("url", "")
        if not isinstance(url, str):
            continue
        try:
            parsed = urlsplit(url)
        except ValueError:
            continue
        if parsed.scheme not in {"http", "https"} or not parsed.hostname or parsed.username or parsed.password:
            continue
        if url in seen:
            continue
        seen.add(url)
        result = {"title": str(item.get("title") or "")[:1000],
                  "url": url, "snippet": str(item.get("snippet") or "")[:3000]}
        for field in ("published_at", "acquired_at"):
            if isinstance(item.get(field), str):
                result[field] = item[field]
        results.append(result)
        if len(results) >= limit:
            break
    if not results:
        raise ToolError("Keenable returned no usable results; try different terms or a wider publication window")
    result = {
        "query": query.strip(), "provider": "keenable", "mode": response.get("mode", mode),
        "timelimit": timelimit, "retrieved_at": datetime.now(timezone.utc).isoformat(),
        "count": len(results), "results": results,
        "note": "Search snippets are not verified page contents. Use fetch_url to read sources. Publication filters exclude pages with unknown publication dates; retrieval time is not publication time.",
    }
    ignored = [key for key in ("region", "safesearch") if key in arguments]
    if ignored:
        result["unsupported_parameters"] = ignored
        result["note"] += " Keenable does not support region or safesearch; supplied legacy options were not applied."
    return result


TOOLS = [
    {
        "name": "search_literature",
        "description": "Search real scientific literature in OpenAlex and arXiv. Returns titles, authors, years, abstracts, DOI values, and URLs. Use this before making literature-backed claims.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "English search terms work best."},
                "source": {"type": "string", "enum": ["openalex", "arxiv", "both"], "default": "both"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 25, "default": 8, "description": "Maximum results per source."},
                "year_from": {"type": "integer", "minimum": 1000, "maximum": 2100},
                "year_to": {"type": "integer", "minimum": 1000, "maximum": 2100},
            },
            "required": ["query"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": True},
    },
    {
        "name": "fetch_url",
        "description": "Fetch readable text from a public HTTP(S) webpage. Private, loopback, link-local, and credential-bearing URLs are rejected.",
        "inputSchema": {
            "type": "object",
            "properties": {"url": {"type": "string"}},
            "required": ["url"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": True},
    },
]

WEB_SEARCH_TOOL = {
    "name": "web_search",
    "description": "Search the live web for general information, current events, official documentation, and websites. Returns titles, URLs, and snippets, not full page text. Use fetch_url to read sources; use search_literature for scientific papers and DOI metadata. Uses Keenable; freshness depends on its index.",
    "inputSchema": {
        "type": "object",
        "properties": {
            "query": {"type": "string", "minLength": 1, "description": "Describe the information to find; use the site parameter to restrict a domain."},
            "limit": {"type": "integer", "minimum": 1, "maximum": 20, "default": 8},
            "timelimit": {"type": "string", "enum": ["d", "w", "m", "y"], "description": "Pages published within the last day/week/month/year; excludes pages without a known publication date."},
            "site": {"type": "string", "minLength": 1, "description": "Optional site restriction, e.g. nvidia.com."},
            "mode": {"type": "string", "enum": ["realtime", "pro"], "default": "realtime", "description": "realtime for fast searches; pro for deeper retrieval."},
        },
        "required": ["query"],
        "additionalProperties": False,
    },
    "annotations": {"readOnlyHint": True, "openWorldHint": True},
}


def configure_web_mode() -> None:
    """Reuse the transport and fetch implementation, with a separate MCP catalog."""
    global WEB_MODE, SERVER_NAME, TOOLS
    WEB_MODE = True
    SERVER_NAME = "qiri-web"
    TOOLS = [WEB_SEARCH_TOOL, next(tool for tool in TOOLS if tool["name"] == "fetch_url")]


def _call_tool(name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    try:
        if name not in {tool["name"] for tool in TOOLS}:
            raise ToolError(f"unknown tool: {name}")
        if name == "search_literature":
            result = search_literature(arguments)
        elif name == "fetch_url":
            result = fetch_url(arguments)
        elif name == "web_search":
            result = web_search(arguments)
        else:
            raise ToolError(f"unknown tool: {name}")
        text = json.dumps(result, ensure_ascii=False, indent=2)
        return {"content": [{"type": "text", "text": text}], "structuredContent": result}
    except (ToolError, ValueError, TypeError) as exc:
        return {"content": [{"type": "text", "text": f"{SERVER_NAME} tool error: {exc}"}], "isError": True}


def _handle(message: dict[str, Any]) -> dict[str, Any] | None:
    request_id = message.get("id")
    method = message.get("method")
    params = message.get("params") or {}
    if "id" not in message:
        return None
    if method == "server/discover":
        return _json_error(request_id, -32601, "server/discover is not supported; use MCP 2025-06-18")
    if method == "initialize":
        return _json_response(
            request_id,
            {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {"listChanged": False}},
                "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                "instructions": (
                    "Use web_search to find webpages, then fetch_url to read and verify sources. Cite source URLs. Search snippets alone do not verify a claim."
                    if WEB_MODE else
                    "Use search_literature for evidence-backed research and report DOI values exactly as returned."
                ),
            },
        )
    if method == "ping":
        return _json_response(request_id, {})
    if method == "tools/list":
        return _json_response(request_id, {"tools": TOOLS})
    if method == "resources/list":
        return _json_response(request_id, {"resources": []})
    if method == "resources/templates/list":
        return _json_response(request_id, {"resourceTemplates": []})
    if method == "prompts/list":
        return _json_response(request_id, {"prompts": []})
    if method == "tools/call":
        name = params.get("name")
        arguments = params.get("arguments", {})
        if not isinstance(name, str) or not isinstance(arguments, dict):
            return _json_error(request_id, -32602, "tools/call requires a string name and object arguments")
        return _json_response(request_id, _call_tool(name, arguments))
    return _json_error(request_id, -32601, f"unknown method: {method}")


def main() -> int:
    for raw_line in sys.stdin.buffer:
        if not raw_line.strip():
            continue
        try:
            message = json.loads(raw_line)
            if not isinstance(message, dict) or message.get("jsonrpc") != "2.0":
                raise ValueError("expected a JSON-RPC 2.0 object")
            response = _handle(message)
        except (json.JSONDecodeError, UnicodeDecodeError, ValueError) as exc:
            response = _json_error(None, -32700, f"parse error: {exc}")
        except Exception as exc:
            response = _json_error(message.get("id") if isinstance(message, dict) else None, -32603, f"internal error: {exc}")
        if response is not None:
            _emit(response)
    return 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--web"]:
        configure_web_mode()
    elif sys.argv[1:]:
        raise SystemExit("Usage: qiri-literature-mcp [--web]")
    raise SystemExit(main())
