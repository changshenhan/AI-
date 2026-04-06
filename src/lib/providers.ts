/** 厂商预设：与 Rust `llm.rs` 中 provider_id 一致。 */

export type ProviderProtocol = "openai_compatible" | "anthropic" | "gemini";

export interface ProviderPreset {
  id: string;
  name: string;
  protocol: ProviderProtocol;
  /** 默认 Base URL，不含末尾斜杠；null 表示须用户填写 */
  defaultBaseUrl: string | null;
  defaultModel: string;
  docUrl: string;
  hint?: string;
}

export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: "openai",
    name: "OpenAI",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-4o-mini",
    docUrl: "https://platform.openai.com/docs/api-reference",
  },
  {
    id: "deepseek",
    name: "DeepSeek（深度求索）",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek-chat",
    docUrl: "https://api-docs.deepseek.com/",
    hint: "与 OpenAI 兼容的 /v1/chat/completions",
  },
  {
    id: "anthropic",
    name: "Anthropic Claude",
    protocol: "anthropic",
    defaultBaseUrl: "https://api.anthropic.com",
    defaultModel: "claude-3-5-sonnet-20241022",
    docUrl: "https://docs.anthropic.com/en/api/getting-started",
  },
  {
    id: "google_gemini",
    name: "Google Gemini",
    protocol: "gemini",
    defaultBaseUrl: "https://generativelanguage.googleapis.com",
    defaultModel: "gemini-2.0-flash",
    docUrl: "https://ai.google.dev/gemini-api/docs",
    hint: "使用 Google AI Studio 的 API Key；模型名不带 models/ 前缀",
  },
  {
    id: "groq",
    name: "Groq",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://api.groq.com/openai/v1",
    defaultModel: "llama-3.3-70b-versatile",
    docUrl: "https://console.groq.com/docs/overview",
  },
  {
    id: "moonshot",
    name: "Moonshot 月之暗面（Kimi）",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://api.moonshot.cn/v1",
    defaultModel: "moonshot-v1-8k",
    docUrl: "https://platform.moonshot.cn/docs/api/chat",
  },
  {
    id: "zhipu",
    name: "智谱 GLM",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
    defaultModel: "glm-4-flash",
    docUrl: "https://open.bigmodel.cn/dev/api",
  },
  {
    id: "qwen",
    name: "阿里通义 Qwen（DashScope 兼容模式）",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    defaultModel: "qwen-turbo",
    docUrl: "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope",
  },
  {
    id: "mistral",
    name: "Mistral AI",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://api.mistral.ai/v1",
    defaultModel: "mistral-small-latest",
    docUrl: "https://docs.mistral.ai/api/",
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://openrouter.ai/api/v1",
    defaultModel: "openai/gpt-4o-mini",
    docUrl: "https://openrouter.ai/docs",
  },
  {
    id: "together",
    name: "Together AI",
    protocol: "openai_compatible",
    defaultBaseUrl: "https://api.together.xyz/v1",
    defaultModel: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
    docUrl: "https://docs.together.ai/reference",
  },
  {
    id: "ollama",
    name: "Ollama（本地）",
    protocol: "openai_compatible",
    defaultBaseUrl: "http://127.0.0.1:11434/v1",
    defaultModel: "llama3.2",
    docUrl: "https://github.com/ollama/ollama/blob/main/docs/openai.md",
    hint: "本地运行 Ollama 时可不填 API Key",
  },
  {
    id: "custom_openai",
    name: "自定义 OpenAI 兼容端点",
    protocol: "openai_compatible",
    defaultBaseUrl: null,
    defaultModel: "gpt-4o-mini",
    docUrl: "https://platform.openai.com/docs/api-reference/chat",
    hint: "填写与 OpenAI 一致的 Base URL（含 /v1）",
  },
  {
    id: "anthropic_custom",
    name: "自定义 Anthropic 端点",
    protocol: "anthropic",
    defaultBaseUrl: null,
    defaultModel: "claude-3-5-sonnet-20241022",
    docUrl: "https://docs.anthropic.com/",
  },
  {
    id: "gemini_custom",
    name: "自定义 Gemini 端点",
    protocol: "gemini",
    defaultBaseUrl: null,
    defaultModel: "gemini-2.0-flash",
    docUrl: "https://ai.google.dev/gemini-api/docs",
  },
];

export function getProvider(id: string): ProviderPreset | undefined {
  return PROVIDER_PRESETS.find((p) => p.id === id);
}
