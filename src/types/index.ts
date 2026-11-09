// ianox — shared types (mirror the Rust config)

export interface Config {
  version: number;
  server: ServerConfig;
  providers: Record<string, ProviderConfig>;
  routes: RouteConfig[];
  mcp_servers?: Record<string, McpServerConfig>;
  projects?: ProjectInfo[];
  active_project?: string | null;
  ui: UiConfig;
}

export interface McpServerConfig {
  id: string;
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
}

export interface McpServerInfo {
  id: string;
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
}

export interface McpServerInput {
  id: string;
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
}

export interface McpToolInfo {
  name: string;
  description: string | null;
}

export interface McpTestResult {
  ok: boolean;
  server_name: string | null;
  server_version: string | null;
  protocol_version: string | null;
  tools: McpToolInfo[];
  error: string | null;
}

export interface ServerConfig {
  enabled: boolean;
  host: string;
  port: number;
  cors: boolean;
  auth_token: string | null;
}

export type ProviderKind =
  | "OpenAI"
  | "Anthropic"
  | "OpenRouter"
  | "Ollama"
  | "Google"
  | "Azure"
  | "Custom";

export interface ProviderConfig {
  id: string;
  name: string;
  kind: ProviderKind;
  base_url: string;
  api_key: string | null;
  models: string[];
  enabled: boolean;
  priority: number;
  rate_limit: RateLimit | null;
}

export interface RateLimit {
  requests_per_minute: number;
  tokens_per_minute: number;
}

export interface RouteConfig {
  id: string;
  name: string;
  enabled: boolean;
  model_pattern: string;
  provider_ids: string[];
  fallback: boolean;
  weight: number;
}

export interface UiConfig {
  theme: string;
  language: string;
  auto_start_server: boolean;
  minimize_to_tray: boolean;
  close_to_tray: boolean;
  /** What closing the window or Ctrl+Q does: ask, keep in the tray, or quit. */
  quit_action?: "ask" | "tray" | "quit";
}

export interface ServerStatus {
  running: boolean;
  addr: string | null;
}

export interface ProviderInfo {
  id: string;
  name: string;
  kind: string;
  base_url: string;
  models: string[];
  enabled: boolean;
  priority: number;
  /** A key is saved; the key itself never leaves the backend. */
  has_key?: boolean;
}

export interface RouteInfo {
  id: string;
  name: string;
  enabled: boolean;
  model_pattern: string;
  provider_ids: string[];
  fallback: boolean;
  weight: number;
}

export interface ModelInfo {
  id: string;
  /** Display name, e.g. "DeepSeek V4 Pro". */
  name: string;
  provider_name: string;
  /** Context window in tokens, when the catalog knows it. */
  context: number | null;
  /** Listed at no cost in the catalog. */
  free: boolean;
  provider: string;
  provider_kind: string;
  size: number | null;
}

// API Types
export interface ChatRequest {
  model: string;
  messages: Message[];
  stream?: boolean;
  temperature?: number;
  max_tokens?: number;
}

export type MessagePart =
  | { type: "text"; text: string }
  | { type: "reasoning"; text: string }
  | { type: "tool-call"; id: string; name: string; arguments: string }
  | { type: "tool-result"; id: string; output: string; is_error?: boolean };

export interface DiffLine {
  kind: "add" | "del" | "ctx";
  text: string;
  /** Line number in the previous file (added rows have none). */
  old: number | null;
  /** Line number in the new file (deleted rows have none). */
  new: number | null;
}

export interface FileDiff {
  path: string;
  added: number;
  removed: number;
  truncated: boolean;
  lines: DiffLine[];
}

export interface ToolRun {
  id: string;
  name: string;
  arguments: string;
  output?: string;
  is_error?: boolean;
  /** What the call changed in a file, ready to paint (add/del/ctx rows). */
  diff?: FileDiff;
  startedAt: number;
  endedAt?: number;
}

/** An image pasted into the composer, kept as base64 (no `data:` prefix). */
export interface ImageAttachment {
  name: string;
  mime: string;
  data: string;
}

export interface Message {
  role: string;
  content: string;
  /** Images that went with this message (pasted screenshots, drag and drop…). */
  images?: ImageAttachment[];
  /** Texto de razonamiento de modelos "thinking" (DeepSeek, o3…). */
  reasoning?: string;
  /** Agent and model the message was sent with. */
  agent?: string;
  model?: string;
  /** Contenido estructurado en orden (texto, razonamiento, herramientas). */
  parts?: MessagePart[];
  /** How long the reasoning stream took (UI state). */
  reasoningMs?: number;
  reasoningStartedAt?: number;
  /** Tool calls of this turn (UI state, merged from the tool events). */
  tools?: ToolRun[];
  /** This message is the summary that replaced everything before it. */
  compaction?: boolean;
}

export interface ChatResponse {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Choice[];
  usage?: Usage;
}

export interface Choice {
  index: number;
  message: Message;
  finish_reason: string;
}

export interface Usage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

export interface ProviderConfigInput {
  id: string;
  name: string;
  kind: ProviderKindInput;
  base_url: string;
  api_key: string | null;
  models: string[];
  enabled: boolean;
  priority: number;
  rate_limit: RateLimitInput | null;
}

export type ProviderKindInput =
  | "OpenAI"
  | "Anthropic"
  | "OpenRouter"
  | "Ollama"
  | "Google"
  | "Azure"
  | "Custom";

export interface RateLimitInput {
  requests_per_minute: number;
  tokens_per_minute: number;
}

export interface RouteConfigInput {
  id: string;
  name: string;
  enabled: boolean;
  model_pattern: string;
  provider_ids: string[];
  fallback: boolean;
  weight: number;
}
export interface ProjectInfo {
  id: string;
  name: string;
  path: string;
  /** Small image (data URL). Empty string: the user removed it, do not detect again. */
  icon?: string | null;
}

export interface FsEntry {
  name: string;
  path: string;
  is_dir: boolean;
}

export interface FileContent {
  content: string;
  truncated: boolean;
}

export type AgentMode = "build" | "plan";

export interface AgentConfig {
  id: string;
  name: string;
  mode: AgentMode;
  model: string | null;
  system_prompt: string;
}

export interface AgentInput {
  id: string;
  name: string;
  mode: AgentMode;
  model: string | null;
  system_prompt: string;
}

export interface ProviderTestResult {
  ok: boolean;
  latency_ms: number;
  models: string[];
  error: string | null;
}

export interface UsageDayPoint {
  label: string;
  tokens: number;
  requests: number;
}

export interface UsageProvider {
  id: string;
  tokens: number;
  requests: number;
}

export interface UsageModel {
  model: string;
  tokens: number;
  requests: number;
}

export interface UsageSummary {
  days: UsageDayPoint[];
  by_provider: UsageProvider[];
  by_model: UsageModel[];
  total_requests: number;
  total_tokens: number;
  total_cost_usd: number | null;
}

export interface SessionSummary {
  id: string;
  title: string;
  project_id: string | null;
  agent_id: string;
  model: string | null;
  updated_at: number;
  message_count: number;
}

export interface StoredSession {
  id: string;
  title: string;
  project_id: string | null;
  agent_id: string;
  model: string | null;
  created_at: number;
  updated_at: number;
  messages: Message[];
}
