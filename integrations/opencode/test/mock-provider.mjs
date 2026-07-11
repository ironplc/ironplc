// A deterministic, OpenAI-compatible chat-completions endpoint that stands in
// for a real model.
//
// It always drives exactly one `ironplc_check` tool call and then ends the
// turn. Pointing OpenCode at it (via the same `provider.<id>.options.baseURL`
// knob the real lane uses for Ollama) exercises OpenCode's real tool-call
// plumbing — catalog read, argument serialization, MCP `tools/call`, result
// handling — with zero model latency and zero flakiness.
//
// Why a fake alongside the real-agent lane: the real lane proves a genuine LLM
// can drive the tool (fidelity); this lane proves the wiring deterministically
// (the gate). A fake is not the same as a real model, so we keep both. Because
// there is no model latency here, the full round-trip completes before OpenCode
// tears the session down, so this lane can reliably assert the compiler
// responded — the assertion that races teardown in the real lane.
//
// Run directly (`node test/mock-provider.mjs`) to serve until killed, printing
// the base URL; or import `startMockProvider()` to embed it in a test.

import http from "node:http";

// Mirror the real lane: a deliberately broken IEC 61131-3 program so the
// compiler returns diagnostics.
const BROKEN_PROGRAM =
  "FUNCTION_BLOCK fb\nVAR x : BOOL := notabool; END_VAR\nEND_FUNCTION_BLOCK";

// OpenCode namespaces the MCP tool as `ironplc_check` in the model-facing tool
// catalog; that is the name the "model" must emit.
const TOOL_NAME = "ironplc_check";
const TOOL_ARGUMENTS = {
  sources: [{ name: "main.st", content: BROKEN_PROGRAM }],
  options: { dialect: "iec61131-3-ed2" },
};

/// True once OpenCode has sent us the tool result (a `role: "tool"` message),
/// i.e. we are on the follow-up turn and should stop rather than call again.
function toolAlreadyRan(body) {
  const messages = Array.isArray(body?.messages) ? body.messages : [];
  return messages.some((m) => m?.role === "tool");
}

/// Whether this request actually offers the check tool. OpenCode issues side
/// calls (e.g. title generation) that do not expose the MCP tools; emitting a
/// tool call for a tool the request never advertised would be rejected, so we
/// only drive the tool when it is on offer.
function offersCheckTool(body) {
  const tools = Array.isArray(body?.tools) ? body.tools : [];
  return tools.some((t) => t?.function?.name === TOOL_NAME);
}

function toolCallMessage() {
  return {
    role: "assistant",
    content: null,
    tool_calls: [
      {
        id: "call_ironplc_check_1",
        type: "function",
        function: {
          name: TOOL_NAME,
          arguments: JSON.stringify(TOOL_ARGUMENTS),
        },
      },
    ],
  };
}

const USAGE = { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 };

/// Non-streaming completion response.
function jsonResponse(model, { toolCall }) {
  const message = toolCall
    ? toolCallMessage()
    : { role: "assistant", content: "Reported the diagnostics." };
  return {
    id: "chatcmpl-mock",
    object: "chat.completion",
    created: Math.floor(Date.now() / 1000),
    model,
    choices: [
      {
        index: 0,
        message,
        finish_reason: toolCall ? "tool_calls" : "stop",
      },
    ],
    usage: USAGE,
  };
}

/// Server-sent-events chunks for a streaming completion. The AI SDK's
/// openai-compatible provider (which OpenCode uses) consumes OpenAI-style
/// `chat.completion.chunk` deltas: a tool call announces `id`/`name` first, then
/// accumulates `arguments`, then a terminal chunk carries `finish_reason`.
function streamChunks(model, { toolCall }) {
  const base = {
    id: "chatcmpl-mock",
    object: "chat.completion.chunk",
    created: Math.floor(Date.now() / 1000),
    model,
  };
  const chunk = (delta, finish_reason = null) => ({
    ...base,
    choices: [{ index: 0, delta, finish_reason }],
  });

  const chunks = [];
  if (toolCall) {
    chunks.push(
      chunk({
        role: "assistant",
        content: null,
        tool_calls: [
          {
            index: 0,
            id: "call_ironplc_check_1",
            type: "function",
            function: { name: TOOL_NAME, arguments: "" },
          },
        ],
      }),
    );
    chunks.push(
      chunk({
        tool_calls: [
          { index: 0, function: { arguments: JSON.stringify(TOOL_ARGUMENTS) } },
        ],
      }),
    );
    chunks.push(chunk({}, "tool_calls"));
  } else {
    chunks.push(chunk({ role: "assistant", content: "Reported the diagnostics." }));
    chunks.push(chunk({}, "stop"));
  }
  // A final usage-only chunk (empty choices), matching OpenAI's
  // `stream_options.include_usage` behavior.
  chunks.push({ ...base, choices: [], usage: USAGE });
  return chunks;
}

function readBody(req) {
  return new Promise((resolve) => {
    let raw = "";
    req.on("data", (c) => (raw += c));
    req.on("end", () => {
      try {
        resolve(JSON.parse(raw || "{}"));
      } catch {
        resolve({});
      }
    });
  });
}

function handleCompletion(req, res, body) {
  const model = typeof body?.model === "string" ? body.model : "mock";
  const toolCall = offersCheckTool(body) && !toolAlreadyRan(body);

  if (process.env.MOCK_PROVIDER_DEBUG) {
    // Strip control characters (e.g. newlines) from untrusted request fields
    // before logging so a crafted request cannot forge log lines (CodeQL: log
    // injection).
    const safe = (v) => String(v).replace(/[^\x20-\x7E]/g, "?").slice(0, 200);
    const safeTools = Array.isArray(body?.tools)
      ? safe(String(body.tools.length))
      : safe("none");
    const safeStream = safe(body?.stream ? "true" : "false");
    console.error(
      `[mock] ${safe(req.method)} ${safe(req.url)} stream=${safeStream} ` +
        `tools=${safeTools} ` +
        `roles=[${(body?.messages || []).map((m) => safe(m?.role)).join(",")}] ` +
        `-> ${toolCall ? "tool_call" : "stop"}`,
    );
  }

  if (body?.stream) {
    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    });
    for (const c of streamChunks(model, { toolCall })) {
      res.write(`data: ${JSON.stringify(c)}\n\n`);
    }
    res.write("data: [DONE]\n\n");
    res.end();
    return;
  }

  res.writeHead(200, { "Content-Type": "application/json" });
  res.end(JSON.stringify(jsonResponse(model, { toolCall })));
}

/// Start the mock provider on an ephemeral port. Resolves to `{ url, close }`,
/// where `url` is the OpenAI base URL to configure as the provider `baseURL`.
export function startMockProvider() {
  const server = http.createServer(async (req, res) => {
    // The AI SDK appends `/chat/completions` to the base URL; accept it under
    // any prefix (e.g. `/v1`). Everything else gets a harmless empty payload.
    if (req.method === "POST" && req.url.endsWith("/chat/completions")) {
      handleCompletion(req, res, await readBody(req));
      return;
    }
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ object: "list", data: [] }));
  });

  if (process.env.MOCK_PROVIDER_DEBUG) {
    server.on("connection", () => console.error("[mock] tcp connection opened"));
  }

  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({
        url: `http://127.0.0.1:${port}/v1`,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}

// When run directly, serve until killed so the mock can be probed by hand.
if (import.meta.url === `file://${process.argv[1]}`) {
  const { url } = await startMockProvider();
  console.log(`mock provider listening: ${url}`);
}
