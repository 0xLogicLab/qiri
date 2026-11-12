# QIRI 前端流式 API

QIRI 通过双向 WebSocket JSON-RPC API 将 科研 Agent 提供给前端。
这里使用 WebSocket 而不是 MCP，是因为前端既要持续接收服务端事件，也要响应审批和
用户输入请求。MCP 仍作为 Agent 内部调用工具的协议。

服务端会流式发送分析摘要、助手回答、命令输出、MCP 工具进度和最终结果、文件变更、
审批请求以及本轮状态。

## 启动服务

使用需要对外提供的定制版本命令启动。启动器设置的 `CODEX_HOME`、模型、开发者指令和
MCP 配置都会自动生效。

```bash
# 通用 QIRI 配置
qiri serve

# 自定义本机端口和前端 Origin
qiri serve --port 9000 --allow-origin http://localhost:3000
```

默认地址：

- WebSocket：`ws://127.0.0.1:8765`
- 就绪检查：`http://127.0.0.1:8765/readyz`
- 健康检查：`http://127.0.0.1:8765/healthz`
- 默认允许来自 `localhost`、`127.0.0.0/8` 和 `[::1]` 的浏览器 Origin。
- 其他浏览器 Origin 必须通过可重复使用的 `--allow-origin` 参数显式添加。

监听公网或局域网地址时必须配置鉴权。使用固定 capability token 时，将随机 token 保存
到文件，再按以下方式启动：

```bash
qiri serve \
  --host 0.0.0.0 \
  --port 8765 \
  --allow-origin https://frontend.example.com \
  --ws-auth capability-token \
  --ws-token-file /run/secrets/qiri-websocket-token
```

非浏览器客户端通过 `Authorization: Bearer <token>` 发送 token。浏览器 WebSocket API
不能直接设置该请求头，因此生产环境应让 QIRI 只监听回环地址，并在前面配置同源、带
用户鉴权的反向代理。不要把 capability token 写进浏览器代码或 URL。

## 连接流程

每个文本帧包含一个 JSON-RPC 2.0 对象，请求 ID 由客户端生成。

1. 连接 WebSocket 地址。
2. 发送 `initialize` 并等待响应。
3. 发送 `initialized` 通知。
4. 发送 `thread/start`，保存响应中的 `result.thread.id`。
5. 使用该 thread ID 和用户输入发送 `turn/start`。
6. 持续处理通知，直到收到对应的 `turn/completed`。
7. 后续多轮对话继续使用同一个 thread ID。

最小初始化请求：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "clientInfo": {
      "name": "qiri-web",
      "title": "QIRI Web",
      "version": "1.0.0"
    },
    "capabilities": {
      "experimentalApi": true
    }
  }
}
```

收到响应后发送：

```json
{"jsonrpc":"2.0","method":"initialized"}
```

在指定工作目录中创建持久会话：

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "thread/start",
  "params": {
    "cwd": "/path/to/research-project",
    "approvalPolicy": "on-request",
    "sandbox": "workspace-write"
  }
}
```

使用请求 2 返回的 `result.thread.id` 提交第一条用户消息：

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "turn/start",
  "params": {
    "threadId": "THREAD_ID",
    "input": [
      {
        "type": "text",
        "text": "检索并分析水稻抗病相关研究",
        "textElements": []
      }
    ]
  }
}
```

## 流式事件

前端按 `method` 分发事件，并保留 `threadId`、`turnId` 和 `itemId`，确保增量更新应用
到正确的 UI 项目。

| Method | 前端用途 |
| --- | --- |
| `turn/started` | 将本轮标记为运行中。 |
| `item/started` | 创建分析、消息、命令、MCP 调用或文件变更项目。 |
| `item/reasoning/summaryTextDelta` | 追加用户可见的分析摘要。 |
| `item/reasoning/textDelta` | 当模型提供时，追加更详细的分析文本。 |
| `item/agentMessage/delta` | 追加助手回答文本。 |
| `item/commandExecution/outputDelta` | 追加命令的 stdout/stderr。 |
| `item/mcpToolCall/progress` | 显示 MCP 工具进度。 |
| `item/completed` | 使用权威的最终数据和工具结果替换对应项目。 |
| `item/fileChange/patchUpdated` | 更新文件变更预览。 |
| `turn/completed` | 停止本轮加载状态并读取最终状态或错误。 |

不要把单个 delta 片段独立当作 Markdown 渲染。应按 item ID 拼接后再渲染完整字符串。
如果 `item/completed` 与已累积的增量不同，以 `item/completed` 为准。

审批和输入请求是带 `id` 的服务端到客户端 JSON-RPC 请求，不是通知。前端必须展示请求
内容，并使用相同 ID 返回响应。常见方法包括：

- `item/commandExecution/requestApproval`
- `item/fileChange/requestApproval`
- `tool/requestUserInput`
- `mcpServer/elicitation/request`

## 浏览器示例

```javascript
const socket = new WebSocket("ws://127.0.0.1:8765");
let nextId = 1;
let threadId;
const pending = new Map();
const textByItem = new Map();

function request(method, params) {
  const id = nextId++;
  socket.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

function append(itemId, delta, channel) {
  const text = (textByItem.get(itemId) ?? "") + delta;
  textByItem.set(itemId, text);
  renderStreamItem({ itemId, channel, text });
}

socket.onmessage = (event) => {
  const message = JSON.parse(event.data);

  if (message.id !== undefined && !message.method) {
    const waiter = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) waiter?.reject(message.error);
    else waiter?.resolve(message.result);
    return;
  }

  const params = message.params ?? {};
  switch (message.method) {
    case "item/reasoning/summaryTextDelta":
    case "item/reasoning/textDelta":
      append(params.itemId, params.delta, "analysis");
      break;
    case "item/agentMessage/delta":
      append(params.itemId, params.delta, "answer");
      break;
    case "item/commandExecution/outputDelta":
      append(params.itemId, params.delta, "command");
      break;
    case "item/mcpToolCall/progress":
      renderToolProgress(params);
      break;
    case "item/completed":
      renderCompletedItem(params.item);
      break;
    case "turn/completed":
      renderTurnCompleted(params.turn);
      break;
    case "item/commandExecution/requestApproval":
      showCommandApproval(message).then((decision) => {
        socket.send(JSON.stringify({ jsonrpc: "2.0", id: message.id, result: decision }));
      });
      break;
  }
};

socket.onopen = async () => {
  await request("initialize", {
    clientInfo: { name: "qiri-web", title: "QIRI Web", version: "1.0.0" },
    capabilities: { experimentalApi: true },
  });
  socket.send(JSON.stringify({ jsonrpc: "2.0", method: "initialized" }));

  const started = await request("thread/start", {
    cwd: "/path/to/research-project",
    approvalPolicy: "on-request",
    sandbox: "workspace-write",
  });
  threadId = started.thread.id;
};

async function sendUserMessage(text) {
  return request("turn/start", {
    threadId,
    input: [{ type: "text", text, textElements: [] }],
  });
}
```

可以使用同一个二进制生成完整协议定义：

```bash
qiri app-server generate-json-schema --experimental --out ./qiri-app-server-schema
qiri app-server generate-ts --experimental --out ./qiri-app-server-types
```

生产客户端应使用生成的类型绑定，不要手工维护请求和事件类型副本。

