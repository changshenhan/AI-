import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PROVIDER_PRESETS, type ProviderPreset } from "../lib/providers";

export interface LlmSettingsPayload {
  providerId: string;
  apiKey: string;
  baseUrlOverride: string;
  model: string;
}

interface LlmTestResult {
  ok: boolean;
  message: string;
  protocol: string;
  resolvedBaseUrl: string | null;
}

interface Props {
  initial?: LlmSettingsPayload | null;
  onSaved: (s: LlmSettingsPayload) => void;
}

const labelCls =
  "mb-1.5 block text-[11px] font-medium uppercase tracking-wide text-[var(--aura-muted)]";

export function SetupView({ initial, onSaved }: Props) {
  const [providerId, setProviderId] = useState(initial?.providerId ?? "deepseek");
  const preset = useMemo(
    () => PROVIDER_PRESETS.find((p) => p.id === providerId) as ProviderPreset,
    [providerId],
  );
  const [apiKey, setApiKey] = useState(initial?.apiKey ?? "");
  const [baseUrlOverride, setBaseUrlOverride] = useState(
    initial?.baseUrlOverride ?? "",
  );
  const [model, setModel] = useState(initial?.model ?? "");
  const [busy, setBusy] = useState(false);
  const [testMsg, setTestMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const effectiveModel = model.trim() || preset.defaultModel;
  const needsCustomBase = preset.defaultBaseUrl === null;

  async function runTest() {
    setErr(null);
    setTestMsg(null);
    setBusy(true);
    try {
      const r = await invoke<LlmTestResult>("llm_test", {
        settings: {
          providerId,
          apiKey,
          baseUrlOverride: baseUrlOverride.trim() || null,
          model: effectiveModel,
        },
      });
      setTestMsg(
        `${r.ok ? "✓ 连接成功" : "连接返回异常"} · ${r.protocol}\n` +
          (r.resolvedBaseUrl ? `Base: ${r.resolvedBaseUrl}\n` : "") +
          `模型回复片段: ${r.message.slice(0, 500)}`,
      );
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function save() {
    setErr(null);
    setBusy(true);
    try {
      await invoke("llm_save_settings", {
        settings: {
          providerId,
          apiKey,
          baseUrlOverride: baseUrlOverride.trim() || null,
          model: effectiveModel,
        },
      });
      onSaved({
        providerId,
        apiKey,
        baseUrlOverride: baseUrlOverride.trim(),
        model: effectiveModel,
      });
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="aura-panel rounded-2xl p-5">
      <header className="mb-5 border-b border-[var(--aura-border)] pb-4">
        <h1 className="text-lg font-semibold tracking-tight text-[var(--aura-text)]">
          连接大模型 API
        </h1>
        <p className="mt-2 text-xs leading-relaxed text-[var(--aura-muted)]">
          请求由应用内 Rust 直连；密钥写入系统钥匙串（旧版明文配置会自动迁移）。
        </p>
      </header>

      <label className="mb-4 block">
        <span className={labelCls}>厂商预设（{PROVIDER_PRESETS.length} 种）</span>
        <select
          className="aura-input w-full"
          value={providerId}
          onChange={(e) => {
            const id = e.target.value;
            setProviderId(id);
            const p = PROVIDER_PRESETS.find((x) => x.id === id);
            if (p) {
              setModel(p.defaultModel);
              setBaseUrlOverride("");
            }
          }}
        >
          {PROVIDER_PRESETS.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>

      <div className="mb-4 flex items-center justify-between gap-2">
        <span className="text-[11px] text-[var(--aura-muted)]">文档</span>
        <button
          type="button"
          className="text-sm text-[var(--aura-accent)] underline-offset-2 hover:underline"
          onClick={() => openUrl(preset.docUrl)}
        >
          打开官方文档
        </button>
      </div>

      {preset.hint && (
        <p className="mb-4 text-xs leading-relaxed text-[var(--aura-muted)]">
          {preset.hint}
        </p>
      )}

      {needsCustomBase ? (
        <label className="mb-4 block">
          <span className={labelCls}>Base URL（必填）</span>
          <input
            className="aura-input w-full font-mono text-[13px]"
            value={baseUrlOverride}
            onChange={(e) => setBaseUrlOverride(e.target.value)}
            placeholder="https://example.com/v1"
            autoComplete="off"
          />
        </label>
      ) : (
        <div className="aura-panel mb-4 rounded-xl p-3">
          <span className={labelCls}>默认 Base URL</span>
          <code className="mb-3 block break-all font-mono text-[11px] text-[var(--aura-muted)]">
            {preset.defaultBaseUrl}
          </code>
          <label className="block">
            <span className={labelCls}>覆盖（可选）</span>
            <input
              className="aura-input w-full font-mono text-[13px]"
              value={baseUrlOverride}
              onChange={(e) => setBaseUrlOverride(e.target.value)}
              placeholder="留空则使用默认值"
              autoComplete="off"
            />
          </label>
        </div>
      )}

      <label className="mb-4 block">
        <span className={labelCls}>
          API Key {preset.id === "ollama" && "（本地可留空）"}
        </span>
        <input
          className="aura-input w-full"
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="sk-..."
          autoComplete="off"
        />
      </label>

      <label className="mb-4 block">
        <span className={labelCls}>模型 ID</span>
        <input
          className="aura-input w-full font-mono text-[13px]"
          value={model}
          onChange={(e) => setModel(e.target.value)}
          placeholder={preset.defaultModel}
          autoComplete="off"
        />
      </label>

      {err && (
        <div className="mb-3 rounded-lg border border-[rgba(201,123,123,0.35)] bg-[rgba(201,123,123,0.1)] px-3 py-2 text-sm text-[var(--aura-danger)]">
          {err}
        </div>
      )}
      {testMsg && (
        <pre className="mb-3 max-h-40 overflow-auto rounded-lg border border-[var(--aura-border)] bg-black/25 p-3 font-mono text-[11px] leading-relaxed text-[var(--aura-muted)]">
          {testMsg}
        </pre>
      )}

      <p className="mb-4 text-[11px] leading-relaxed text-[var(--aura-muted)]">
        「测试连接」不写钥匙串；排期、流式对话与完成反馈依赖「保存并进入」。
      </p>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          className="aura-btn aura-btn-ghost flex-1 min-[380px]:flex-none"
          disabled={busy}
          onClick={runTest}
        >
          测试连接
        </button>
        <button
          type="button"
          className="aura-btn aura-btn-primary flex-1 min-[380px]:flex-none"
          disabled={busy}
          onClick={save}
        >
          保存并进入
        </button>
      </div>
    </div>
  );
}
