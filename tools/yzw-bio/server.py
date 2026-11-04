#!/usr/bin/env python3
"""MCP tools for Yazhou Bay agricultural and biological breeding research."""

from __future__ import annotations

import http.client
import ipaddress
import json
import math
import os
import socket
import struct
import sys
from pathlib import Path
from typing import Any
from urllib.parse import quote, urlencode, urlsplit


SERVER_NAME = "yzw-bio"
SERVER_VERSION = "0.1.0"
PROTOCOL_VERSION = "2025-06-18"
DEFAULT_API_ORIGIN = "http://10.1.16.2:8080"
DEFAULT_SOCKS_PROXY = "127.0.0.1:1080"
MAX_RESPONSE_BYTES = 8_000_000


class ToolError(Exception):
    pass


def _read_exact(sock: socket.socket, count: int) -> bytes:
    result = bytearray()
    while len(result) < count:
        chunk = sock.recv(count - len(result))
        if not chunk:
            raise ToolError("SOCKS5 proxy closed the connection")
        result.extend(chunk)
    return bytes(result)


def _parse_proxy(value: str) -> tuple[str, int] | None:
    value = value.strip()
    if not value or value.lower() in {"none", "direct"}:
        return None
    if "://" in value:
        parsed = urlsplit(value)
        if parsed.scheme not in {"socks5", "socks5h"} or not parsed.hostname:
            raise ToolError("YZW_SOCKS_PROXY must use socks5:// or socks5h://")
        return parsed.hostname, parsed.port or 1080
    host, separator, port = value.rpartition(":")
    if not separator or not host:
        raise ToolError("YZW_SOCKS_PROXY must be host:port")
    try:
        return host, int(port)
    except ValueError as exc:
        raise ToolError("YZW_SOCKS_PROXY port must be an integer") from exc


def _open_socks_connection(
    proxy: tuple[str, int], target_host: str, target_port: int, timeout: float
) -> socket.socket:
    sock = socket.create_connection(proxy, timeout=timeout)
    try:
        sock.sendall(b"\x05\x01\x00")
        if _read_exact(sock, 2) != b"\x05\x00":
            raise ToolError("SOCKS5 proxy rejected no-authentication mode")

        try:
            ip = ipaddress.ip_address(target_host)
        except ValueError:
            encoded = target_host.encode("idna")
            if len(encoded) > 255:
                raise ToolError("target hostname is too long")
            address = b"\x03" + bytes([len(encoded)]) + encoded
        else:
            if ip.version == 4:
                address = b"\x01" + ip.packed
            else:
                address = b"\x04" + ip.packed

        sock.sendall(b"\x05\x01\x00" + address + struct.pack("!H", target_port))
        version, reply, _, address_type = _read_exact(sock, 4)
        if version != 5 or reply != 0:
            messages = {
                1: "general failure",
                2: "connection not allowed",
                3: "network unreachable",
                4: "host unreachable",
                5: "connection refused",
                6: "TTL expired",
                7: "command not supported",
                8: "address type not supported",
            }
            raise ToolError(f"SOCKS5 connection failed: {messages.get(reply, f'reply {reply}')}")
        if address_type == 1:
            _read_exact(sock, 4)
        elif address_type == 3:
            _read_exact(sock, _read_exact(sock, 1)[0])
        elif address_type == 4:
            _read_exact(sock, 16)
        else:
            raise ToolError(f"SOCKS5 proxy returned unknown address type {address_type}")
        _read_exact(sock, 2)
        return sock
    except Exception:
        sock.close()
        raise


class _SocksHTTPConnection(http.client.HTTPConnection):
    def __init__(
        self,
        host: str,
        port: int,
        proxy: tuple[str, int],
        timeout: float,
    ) -> None:
        super().__init__(host, port=port, timeout=timeout)
        self.proxy = proxy

    def connect(self) -> None:
        self.sock = _open_socks_connection(
            self.proxy, self.host, self.port or 80, float(self.timeout or 45)
        )


def _load_api_key() -> str:
    value = os.environ.get("YZW_DATA_API_KEY", "").strip()
    if value:
        return value
    key_file = Path(
        os.environ.get("YZW_DATA_API_KEY_FILE", "~/.yzw/api.txt")
    ).expanduser()
    try:
        value = key_file.read_text(encoding="utf-8").strip()
    except OSError as exc:
        raise ToolError(
            f"Yazhou Bay API key not found; set YZW_DATA_API_KEY or create {key_file}"
        ) from exc
    if not value:
        raise ToolError(f"Yazhou Bay API key file is empty: {key_file}")
    return value


class ApiClient:
    def __init__(self) -> None:
        origin = os.environ.get("YZW_API_ORIGIN", DEFAULT_API_ORIGIN).rstrip("/")
        parsed = urlsplit(origin)
        if parsed.scheme != "http" or not parsed.hostname:
            raise ToolError("YZW_API_ORIGIN must be an absolute http:// URL")
        self.host = parsed.hostname
        self.port = parsed.port or 80
        self.base_path = parsed.path.rstrip("/")
        self.proxy = _parse_proxy(
            os.environ.get("YZW_SOCKS_PROXY", DEFAULT_SOCKS_PROXY)
        )
        self.timeout = float(os.environ.get("YZW_API_TIMEOUT_SEC", "45"))

    def request(
        self,
        method: str,
        path: str,
        *,
        query: dict[str, Any] | None = None,
        body: dict[str, Any] | None = None,
        expect_json: bool = True,
    ) -> Any:
        if query:
            clean_query = {
                key: value
                for key, value in query.items()
                if value is not None and value != ""
            }
            if clean_query:
                path = f"{path}?{urlencode(clean_query)}"
        path = f"{self.base_path}{path}"
        headers = {
            "Accept": "application/json, text/plain;q=0.9, */*;q=0.1",
            "User-Agent": "YZW/0.1 (Yazhou Bay breeding research agent)",
            "X-API-Key": _load_api_key(),
        }
        payload = None
        if body is not None:
            payload = json.dumps(body, ensure_ascii=False).encode("utf-8")
            headers["Content-Type"] = "application/json"

        if self.proxy is None:
            connection: http.client.HTTPConnection = http.client.HTTPConnection(
                self.host, self.port, timeout=self.timeout
            )
        else:
            connection = _SocksHTTPConnection(
                self.host, self.port, self.proxy, self.timeout
            )
        try:
            connection.request(method, path, body=payload, headers=headers)
            response = connection.getresponse()
            data = response.read(MAX_RESPONSE_BYTES + 1)
            if len(data) > MAX_RESPONSE_BYTES:
                raise ToolError(f"response exceeded {MAX_RESPONSE_BYTES} bytes")
            text = data.decode(response.headers.get_content_charset() or "utf-8", errors="replace")
            if response.status < 200 or response.status >= 300:
                detail = text.strip().replace("\n", " ")[:1200]
                raise ToolError(f"HTTP {response.status} from {path}: {detail}")
            content_type = response.headers.get_content_type()
            if expect_json or content_type == "application/json":
                try:
                    return json.loads(text)
                except json.JSONDecodeError as exc:
                    raise ToolError(f"invalid JSON from {path}: {exc}") from exc
            return {
                "content_type": content_type,
                "text": text,
                "truncated": len(data) == MAX_RESPONSE_BYTES + 1,
            }
        except (OSError, http.client.HTTPException) as exc:
            raise ToolError(f"request failed for {path}: {exc}") from exc
        finally:
            connection.close()


_CLIENT: ApiClient | None = None


def _client() -> ApiClient:
    global _CLIENT
    if _CLIENT is None:
        _CLIENT = ApiClient()
    return _CLIENT


def _bounded_int(
    arguments: dict[str, Any], name: str, default: int, minimum: int, maximum: int
) -> int:
    try:
        value = int(arguments.get(name, default))
    except (TypeError, ValueError) as exc:
        raise ToolError(f"{name} must be an integer") from exc
    if not minimum <= value <= maximum:
        raise ToolError(f"{name} must be between {minimum} and {maximum}")
    return value


def _required_text(arguments: dict[str, Any], name: str) -> str:
    raw_value = arguments.get(name)
    value = "" if raw_value is None else str(raw_value).strip()
    if not value:
        raise ToolError(f"{name} must not be empty")
    return value


def _path_segment(value: str) -> str:
    return quote(value, safe="")


def search_papers(arguments: dict[str, Any]) -> dict[str, Any]:
    query = _required_text(arguments, "query")
    page = _bounded_int(arguments, "page", 1, 1, 100_000)
    page_size = _bounded_int(arguments, "page_size", 10, 1, 50)
    combine = str(arguments.get("combine", "union"))
    sort = str(arguments.get("sort", "relevance"))
    expand = str(arguments.get("expand", "auto"))
    if combine not in {"union", "intersect"}:
        raise ToolError("combine must be union or intersect")
    if sort not in {"relevance", "year"}:
        raise ToolError("sort must be relevance or year")
    if expand not in {"auto", "fulltext"}:
        raise ToolError("expand must be auto or fulltext")
    params = {
        "query": query,
        "combine": combine,
        "page": page,
        "page_size": page_size,
        "year_from": arguments.get("year_from"),
        "year_to": arguments.get("year_to"),
        "journal": arguments.get("journal"),
        "sort": sort,
        "expand": expand,
    }
    try:
        result = _client().request("GET", "/tagp/v1/search", query=params)
        return {"source": "tagp", **result}
    except ToolError as tagp_error:
        if not arguments.get("fallback", True):
            raise
        fallback = _client().request(
            "GET",
            "/graph/v1/paper/search",
            query={
                "query": query,
                "limit": page_size,
                "offset": (page - 1) * page_size,
            },
        )
        return {
            "source": "graph_v1_fallback",
            "tagp_error": str(tagp_error),
            "data": fallback,
        }


def resolve_entity(arguments: dict[str, Any]) -> dict[str, Any]:
    entity_type = str(arguments.get("entity_type", "concept")).strip().lower()
    allowed = {"concept", "gene", "species", "germplasm", "variation"}
    if entity_type not in allowed:
        raise ToolError(f"entity_type must be one of {', '.join(sorted(allowed))}")
    query = _required_text(arguments, "query")
    params: dict[str, Any] = {
        "query": query,
        "limit": _bounded_int(arguments, "limit", 10, 1, 50),
    }
    if entity_type == "concept":
        vocabulary = str(arguments.get("vocabulary", "all"))
        if vocabulary not in {"all", "MeSH", "AGROVOC", "PBO"}:
            raise ToolError("vocabulary must be all, MeSH, AGROVOC, or PBO")
        params["vocabulary"] = vocabulary
    return _client().request(
        "GET", f"/tagp/v1/resolve/{entity_type}", query=params
    )


def get_paper(arguments: dict[str, Any]) -> dict[str, Any]:
    paper_id = _required_text(arguments, "paper_id")
    encoded_id = _path_segment(paper_id)
    try:
        result = _client().request("GET", f"/tagp/v1/paper/{encoded_id}")
        return {"source": "tagp", **result}
    except ToolError as tagp_error:
        if not arguments.get("fallback", True):
            raise
        result = _client().request("GET", f"/graph/v1/paper/{encoded_id}")
        return {
            "source": "graph_v1_fallback",
            "tagp_error": str(tagp_error),
            "data": result,
        }


def get_paper_annotations(arguments: dict[str, Any]) -> dict[str, Any]:
    paper_id = _required_text(arguments, "paper_id")
    return _client().request(
        "GET",
        f"/tagp/v1/paper/{_path_segment(paper_id)}/annotations",
        query={
            "types": arguments.get("types"),
            "limit": _bounded_int(arguments, "limit", 200, 1, 2000),
            "page": _bounded_int(arguments, "page", 1, 1, 100_000),
        },
    )


def find_related_entities(arguments: dict[str, Any]) -> dict[str, Any]:
    confidence = str(arguments.get("confidence", "存疑"))
    if confidence not in {"存疑", "准确"}:
        raise ToolError("confidence must be 存疑 or 准确")
    return _client().request(
        "GET",
        "/tagp/v1/relations",
        query={
            "e1": _required_text(arguments, "e1"),
            "e2": arguments.get("e2"),
            "type": arguments.get("relation_type"),
            "confidence": confidence,
            "refresh": str(bool(arguments.get("refresh", False))).lower(),
            "organism": arguments.get("organism"),
            "page": _bounded_int(arguments, "page", 1, 1, 100_000),
            "page_size": _bounded_int(arguments, "page_size", 20, 1, 50),
        },
    )


def get_paper_resource(arguments: dict[str, Any]) -> dict[str, Any]:
    paper_id = _required_text(arguments, "paper_id")
    resource_format = str(arguments.get("format", "list"))
    if resource_format not in {"list", "md"}:
        raise ToolError("format must be list or md; PDF binary download is not exposed to MCP")
    return _client().request(
        "GET",
        f"/tagp/v1/paper/{_path_segment(paper_id)}/resource",
        query={"format": resource_format},
    )


def semantic_similarity(arguments: dict[str, Any]) -> dict[str, Any]:
    texts = arguments.get("texts")
    if not isinstance(texts, list) or not 2 <= len(texts) <= 20:
        raise ToolError("texts must be an array containing 2 to 20 strings")
    normalized_texts = [str(text).strip() for text in texts]
    if any(not text for text in normalized_texts):
        raise ToolError("texts must not contain empty values")
    result = _client().request(
        "POST",
        "/graph/v1/vector/embed",
        body={
            "texts": normalized_texts,
            "model_name": str(arguments.get("model_name", "bge-m3")),
            "batch_size": min(len(normalized_texts), 32),
            "normalize_embeddings": True,
        },
    )
    embeddings = result.get("embeddings")
    if not isinstance(embeddings, list) or len(embeddings) != len(normalized_texts):
        raise ToolError("vector service returned an unexpected embeddings payload")

    def cosine(left: list[float], right: list[float]) -> float:
        numerator = sum(a * b for a, b in zip(left, right, strict=True))
        left_norm = math.sqrt(sum(value * value for value in left))
        right_norm = math.sqrt(sum(value * value for value in right))
        if left_norm == 0 or right_norm == 0:
            return 0.0
        return numerator / (left_norm * right_norm)

    matrix = [
        [round(cosine(left, right), 6) for right in embeddings]
        for left in embeddings
    ]
    return {
        "model_name": result.get("model_name"),
        "dimension": result.get("dimension"),
        "texts": normalized_texts,
        "similarity_matrix": matrix,
    }


def service_health(arguments: dict[str, Any]) -> dict[str, Any]:
    del arguments
    tagp = _client().request("GET", "/tagp/health")
    vector = _client().request("GET", "/graph/v1/vector/ready")
    return {"tagp": tagp, "vector": vector}


TOOLS = [
    {
        "name": "search_papers",
        "description": "优先使用崖州湾 TagP 检索农业与生物育种文献；TagP 失败时可回退到 Graph v1。支持自由文本、实体 ID、DOI、PMID 和年份/期刊过滤。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "page": {"type": "integer", "minimum": 1, "default": 1},
                "page_size": {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
                "combine": {"type": "string", "enum": ["union", "intersect"], "default": "union"},
                "year_from": {"type": "integer"},
                "year_to": {"type": "integer"},
                "journal": {"type": "string"},
                "sort": {"type": "string", "enum": ["relevance", "year"], "default": "relevance"},
                "expand": {"type": "string", "enum": ["auto", "fulltext"], "default": "auto"},
                "fallback": {"type": "boolean", "default": True},
            },
            "required": ["query"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": False},
    },
    {
        "name": "resolve_entity",
        "description": "将农业或生物术语解析为规范实体 ID。支持 concept、gene、species、germplasm、variation；进行关系检索前应先调用此工具。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "entity_type": {"type": "string", "enum": ["concept", "gene", "species", "germplasm", "variation"], "default": "concept"},
                "query": {"type": "string"},
                "vocabulary": {"type": "string", "enum": ["all", "MeSH", "AGROVOC", "PBO"], "default": "all"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
            },
            "required": ["query"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": False},
    },
    {
        "name": "get_paper",
        "description": "按雪花 ID、PMID 或 DOI 获取文献标题、作者、摘要、年份和期刊等元数据。优先使用 TagP。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "paper_id": {"type": "string"},
                "fallback": {"type": "boolean", "default": True},
            },
            "required": ["paper_id"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": False},
    },
    {
        "name": "get_paper_annotations",
        "description": "获取单篇文献中的概念、基因、物种、种质和变异标注，用于证据抽取和知识图谱分析。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "paper_id": {"type": "string"},
                "types": {"type": "string", "description": "逗号分隔，如 concept,gene"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 2000, "default": 200},
                "page": {"type": "integer", "minimum": 1, "default": 1},
            },
            "required": ["paper_id"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": False},
    },
    {
        "name": "find_related_entities",
        "description": "查询实体间的同句共现关系。只传 e1 时发现相关实体，同时传 e1/e2 时验证两者关联；可按物种过滤。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "e1": {"type": "string"},
                "e2": {"type": "string"},
                "relation_type": {"type": "string"},
                "confidence": {"type": "string", "enum": ["存疑", "准确"], "default": "存疑"},
                "refresh": {"type": "boolean", "default": False},
                "organism": {"type": "string", "description": "如 水稻,rice,Oryza"},
                "page": {"type": "integer", "minimum": 1, "default": 1},
                "page_size": {"type": "integer", "minimum": 1, "maximum": 50, "default": 20},
            },
            "required": ["e1"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": False},
    },
    {
        "name": "get_paper_resource",
        "description": "获取文献可用全文资源列表或 Markdown 全文。为避免向模型传输大二进制，不直接返回 PDF。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "paper_id": {"type": "string"},
                "format": {"type": "string", "enum": ["list", "md"], "default": "list"},
            },
            "required": ["paper_id"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": False},
    },
    {
        "name": "semantic_similarity",
        "description": "使用站点 bge-m3 向量服务计算 2 到 20 段文本的余弦相似度矩阵，适合品种性状、摘要和研究主题聚类前分析。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "texts": {"type": "array", "minItems": 2, "maxItems": 20, "items": {"type": "string"}},
                "model_name": {"type": "string", "default": "bge-m3"},
            },
            "required": ["texts"],
            "additionalProperties": False,
        },
        "annotations": {"readOnlyHint": True, "openWorldHint": False},
    },
    {
        "name": "service_health",
        "description": "检查崖州湾 TagP 与向量服务是否可用。排查数据工具失败时调用。",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
        "annotations": {"readOnlyHint": True, "openWorldHint": False},
    },
]


TOOL_HANDLERS = {
    "search_papers": search_papers,
    "resolve_entity": resolve_entity,
    "get_paper": get_paper,
    "get_paper_annotations": get_paper_annotations,
    "find_related_entities": find_related_entities,
    "get_paper_resource": get_paper_resource,
    "semantic_similarity": semantic_similarity,
    "service_health": service_health,
}


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


def _call_tool(name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    try:
        handler = TOOL_HANDLERS.get(name)
        if handler is None:
            raise ToolError(f"unknown tool: {name}")
        result = handler(arguments)
        text = json.dumps(result, ensure_ascii=False, indent=2)
        return {"content": [{"type": "text", "text": text}], "structuredContent": result}
    except (ToolError, ValueError, TypeError, KeyError) as exc:
        return {
            "content": [{"type": "text", "text": f"YZW biology tool error: {exc}"}],
            "isError": True,
        }


def _handle(message: dict[str, Any]) -> dict[str, Any] | None:
    request_id = message.get("id")
    method = message.get("method")
    params = message.get("params") or {}
    if "id" not in message:
        return None
    if method == "initialize":
        return _json_response(
            request_id,
            {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {"listChanged": False}},
                "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                "instructions": (
                    "Prefer TagP for agricultural and breeding literature. Resolve entity IDs "
                    "before relation analysis, preserve paper identifiers, and distinguish "
                    "retrieved evidence from model inference."
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
            return _json_error(
                request_id, -32602, "tools/call requires a string name and object arguments"
            )
        return _json_response(request_id, _call_tool(name, arguments))
    return _json_error(request_id, -32601, f"unknown method: {method}")


def main() -> int:
    for raw_line in sys.stdin.buffer:
        if not raw_line.strip():
            continue
        message: Any = None
        try:
            message = json.loads(raw_line)
            if not isinstance(message, dict) or message.get("jsonrpc") != "2.0":
                raise ValueError("expected a JSON-RPC 2.0 object")
            response = _handle(message)
        except (json.JSONDecodeError, UnicodeDecodeError, ValueError) as exc:
            response = _json_error(None, -32700, f"parse error: {exc}")
        except Exception as exc:
            request_id = message.get("id") if isinstance(message, dict) else None
            response = _json_error(request_id, -32603, f"internal error: {exc}")
        if response is not None:
            _emit(response)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
