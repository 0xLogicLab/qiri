#!/usr/bin/env python3
"""Minimal stdio MCP server for QIRI scientific literature search."""

from __future__ import annotations

import html
import ipaddress
import json
import re
import socket
import sys
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


def _call_tool(name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    try:
        if name == "search_literature":
            result = search_literature(arguments)
        elif name == "fetch_url":
            result = fetch_url(arguments)
        else:
            raise ToolError(f"unknown tool: {name}")
        text = json.dumps(result, ensure_ascii=False, indent=2)
        return {"content": [{"type": "text", "text": text}], "structuredContent": result}
    except (ToolError, ValueError, TypeError) as exc:
        return {"content": [{"type": "text", "text": f"QIRI literature tool error: {exc}"}], "isError": True}


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
                "instructions": "Use search_literature for evidence-backed research and report DOI values exactly as returned.",
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
    raise SystemExit(main())
