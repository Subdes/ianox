/**
 * Turns the UI messages into the OpenAI-style payload the backend expects.
 *
 * A finished assistant turn keeps its tool calls and results, so the model
 * remembers in the next message what it read, ran and wrote (the UI-only
 * fields — reasoning, timings, agent — never leave this file).
 */
import type { ImageAttachment, Message } from "./types";

type ApiMessage = Record<string, unknown>;

/** A pasted image in the OpenAI content-block shape the providers accept. */
function imageBlock(image: ImageAttachment): ApiMessage {
  return {
    type: "image_url",
    image_url: { url: `data:${image.mime};base64,${image.data}` },
  };
}

/** Old tool output is capped; the model can read the file again if it needs more. */
const MAX_OUTPUT = 6000;

function cap(output: string): string {
  return output.length > MAX_OUTPUT
    ? `${output.slice(0, MAX_OUTPUT)}\n… (truncated ${output.length - MAX_OUTPUT} chars)`
    : output;
}

/** How the model is told that earlier turns were replaced by a summary. */
export function compactionNotice(summary: string): string {
  return `The conversation so far was compacted to save context. This is a summary of everything before this point:\n\n${summary}\n\nContinue from here; the user's next message follows.`;
}

export function toApiMessages(messages: Message[], clean: (text: string) => string = (t) => t): ApiMessage[] {
  const out: ApiMessage[] = [];

  // Everything before the last compaction is replaced by its summary.
  const cut = messages.map((m) => !!m.compaction).lastIndexOf(true);
  if (cut >= 0) {
    out.push({ role: "user", content: compactionNotice(messages[cut].content) });
    messages = messages.slice(cut + 1);
  }

  for (const message of messages) {
    if (message.role !== "assistant") {
      const text = clean(message.content);
      const images = message.images ?? [];
      if (images.length === 0) {
        out.push({ role: message.role, content: text });
      } else {
        // Multimodal turn: text first, then each image.
        const blocks: ApiMessage[] = [];
        if (text.trim()) blocks.push({ type: "text", text });
        for (const image of images) blocks.push(imageBlock(image));
        out.push({ role: message.role, content: blocks });
      }
      continue;
    }

    const parts = message.parts ?? [];
    if (!parts.some((p) => p.type === "tool-call")) {
      const content = clean(message.content);
      // "⚠ …" lines are errors shown to the user, not something the model said.
      if (content.trim() && !content.startsWith("⚠")) out.push({ role: "assistant", content });
      continue;
    }

    const tools = new Map((message.tools ?? []).map((tool) => [tool.id, tool]));
    let text = "";
    let calls: { id: string; name: string; arguments: string }[] = [];

    const flush = () => {
      if (calls.length === 0) return;
      out.push({
        role: "assistant",
        content: clean(text).trim() ? clean(text) : null,
        tool_calls: calls.map((call) => ({
          id: call.id,
          type: "function",
          function: { name: call.name, arguments: call.arguments },
        })),
      });
      for (const call of calls) {
        const tool = tools.get(call.id);
        out.push({
          role: "tool",
          tool_call_id: call.id,
          content: tool?.output !== undefined ? cap(tool.output) : "Interrupted before it finished.",
        });
      }
      text = "";
      calls = [];
    };

    for (const part of parts) {
      if (part.type === "text") {
        // Text after tool calls starts a new assistant step.
        if (calls.length > 0) flush();
        text += part.text;
      } else if (part.type === "tool-call") {
        calls.push({ id: part.id, name: part.name, arguments: part.arguments });
      }
    }
    flush();
    if (clean(text).trim()) out.push({ role: "assistant", content: clean(text) });
  }

  return out;
}

export interface AttachedFile {
  path: string;
  name: string;
  content: string;
  truncated?: boolean;
}

/** What the model receives: the user's text followed by each attached file in a tagged block. */
export function buildUserContent(text: string, files: AttachedFile[]): string {
  const blocks = files.map((f) => `\n\n<file path="${f.path}">\n${f.content}\n</file>`).join("");
  return `${text.trim()}${blocks}`.trim();
}

/**
 * Splits a stored user message back into what the user typed and the files that went with it,
 * so the bubble shows the text and file chips instead of the whole file contents.
 * Messages saved before the tagged format keep showing as they were.
 */
export function splitAttachments(content: string): { text: string; files: { path: string; name: string }[] } {
  const files: { path: string; name: string }[] = [];
  const text = content
    .replace(/\n*<file path="([^"]*)">\n[\s\S]*?\n<\/file>/g, (_whole, path: string) => {
      files.push({ path, name: path.split("/").filter(Boolean).pop() ?? path });
      return "";
    })
    .trim();
  return { text, files };
}

/** How much of a tool result goes into the transcript that is summarised. */
const TRANSCRIPT_TOOL_OUTPUT = 600;

/**
 * Plain-text transcript of a conversation, for the model to summarise. Tool results are
 * shortened and file attachments reduced to their names; if it is still too long, the
 * oldest part is dropped (it is already covered by the previous summary, if any).
 */
export function buildTranscript(messages: Message[], maxChars = 300_000): string {
  const lines: string[] = [];
  for (const message of messages) {
    if (message.compaction) {
      lines.push(`[Summary of the earlier conversation]\n${message.content}`);
    } else if (message.role === "user") {
      const { text, files } = splitAttachments(message.content);
      const notes: string[] = [];
      if (files.length) notes.push(`attached: ${files.map((f) => f.path).join(", ")}`);
      if (message.images?.length) notes.push(`${message.images.length} image(s)`);
      const attached = notes.length ? `\n[${notes.join("; ")}]` : "";
      lines.push(`User: ${text}${attached}`);
    } else if (message.role === "assistant") {
      const tools = new Map((message.tools ?? []).map((tool) => [tool.id, tool]));
      const parts = message.parts?.length
        ? message.parts
        : [{ type: "text" as const, text: message.content }];
      const chunk: string[] = [];
      for (const part of parts) {
        if (part.type === "text" && part.text.trim()) chunk.push(part.text.trim());
        else if (part.type === "tool-call") {
          const tool = tools.get(part.id);
          const output = (tool?.output ?? "(no result)").slice(0, TRANSCRIPT_TOOL_OUTPUT);
          chunk.push(`[tool ${part.name} ${part.arguments.slice(0, 200)} -> ${output}]`);
        }
      }
      if (chunk.length) lines.push(`Assistant: ${chunk.join("\n")}`);
    }
  }
  const text = lines.join("\n\n");
  return text.length > maxChars ? `[…earlier part omitted…]\n${text.slice(text.length - maxChars)}` : text;
}
