import {
  FloppyDisk,
  PencilSimple,
  Robot,
  SpinnerGap,
  Trash,
  WarningCircle,
  X,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import type { MessageKey } from "../i18n";
import { api } from "../lib/api";
import type { ProviderPreset, ProviderProfile, ProviderUpsert } from "../types";
import { Button } from "./ui/button";
import { ConfirmDialog } from "./ui/confirm";

type NotifyType = "success" | "error" | "info";

type AiSettingsProps = {
  t: (key: MessageKey) => string;
  notify?: (type: NotifyType, message: string, detail?: string) => void;
};

const DEFAULT_PROTOCOL = "openai-compatible";
const PROTOCOLS = [DEFAULT_PROTOCOL, "openai-chat", "anthropic", "gemini", "ollama"];
const ENVIRONMENT_VARIABLE = /^[A-Za-z_][A-Za-z0-9_]*$/;

function errorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return fallback;
}

function isLocalProvider(name: string, protocol: string, baseUrl: string) {
  if (protocol.trim().toLowerCase() === "ollama") return true;
  if (name.trim().toLowerCase() === "lm studio") return true;
  try {
    const hostname = new URL(baseUrl.trim()).hostname.toLowerCase();
    return hostname === "localhost" || hostname === "127.0.0.1" || hostname === "[::1]" || hostname === "::1";
  } catch {
    return false;
  }
}

export function AiSettings({ t, notify }: AiSettingsProps) {
  const [profiles, setProfiles] = useState<ProviderProfile[]>([]);
  const [presets, setPresets] = useState<ProviderPreset[]>([]);
  const [name, setName] = useState("");
  const [protocol, setProtocol] = useState(DEFAULT_PROTOCOL);
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [credentialRef, setCredentialRef] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ProviderProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  function reportError(error: unknown) {
    const detail = errorMessage(error, t("unknownAiError"));
    const title = t("providerOperationFailed");
    setError(title);
    setErrorDetail(detail);
    notify?.("error", title, detail);
  }

  async function load(): Promise<boolean> {
    setLoading(true);
    setError(null);
    setErrorDetail(null);
    setLoadError(null);
    try {
      const [nextProfiles, nextPresets] = await Promise.all([
        api.listProviderProfiles(),
        api.providerPresets(),
      ]);
      setProfiles(nextProfiles);
      setPresets(nextPresets);
      return true;
    } catch (error) {
      setLoadError(errorMessage(error, t("unknownAiError")));
      reportError(error);
      return false;
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  function resetForm() {
    setEditingId(null);
    setName("");
    setProtocol(DEFAULT_PROTOCOL);
    setBaseUrl("");
    setModel("");
    setCredentialRef("");
    setSubmitted(false);
  }

  function applyPreset(presetName: string) {
    const preset = presets.find((item) => item.name === presetName);
    if (!preset) return;

    setError(null);
    setErrorDetail(null);
    // A preset describes a new connection. Never let applying it retain the
    // id of a profile that happened to be open in the editor, otherwise Save
    // would silently overwrite that existing profile.
    setEditingId(null);
    setName(preset.name);
    setProtocol(preset.protocol);
    setBaseUrl(preset.baseUrl ?? "");
    setModel(preset.defaultModel);
    setCredentialRef(preset.credentialRef);
    setSubmitted(false);
  }

  function editProfile(profile: ProviderProfile) {
    setError(null);
    setErrorDetail(null);
    setEditingId(profile.id);
    setName(profile.name);
    setProtocol(profile.protocol);
    setBaseUrl(profile.baseUrl ?? "");
    setModel(profile.model);
    setCredentialRef(profile.credentialRef);
    setSubmitted(false);
  }

  function validate() {
    const localProvider = isLocalProvider(name, protocol, baseUrl);
    const credential = credentialRef.trim();
    if (!name.trim() || !protocol.trim() || !model.trim() || (!localProvider && !credential)) {
      setError(t("providerRequiredError"));
      setErrorDetail(null);
      return false;
    }
    if (credential && !ENVIRONMENT_VARIABLE.test(credential)) {
      setError(t("credentialInvalidError"));
      setErrorDetail(null);
      return false;
    }
    return true;
  }

  async function save() {
    setSubmitted(true);
    setError(null);
    setErrorDetail(null);
    if (!validate()) return;

    const savedName = name.trim();
    const wasEditing = Boolean(editingId);
    const upsert: ProviderUpsert = {
      id: editingId,
      name: savedName,
      protocol: protocol.trim(),
      baseUrl: baseUrl.trim() || null,
      model: model.trim(),
      credentialRef: credentialRef.trim(),
    };

    setSaving(true);
    try {
      await api.upsertProviderProfile(upsert);
      const loaded = await load();
      if (!loaded) return;
      resetForm();
      notify?.("success", `${savedName} · ${t(wasEditing ? "providerUpdated" : "providerSaved")}`);
    } catch (error) {
      reportError(error);
    } finally {
      setSaving(false);
    }
  }

  async function confirmDelete() {
    const target = deleteTarget;
    if (!target) return;

    setDeleteTarget(null);
    setDeletingId(target.id);
    setError(null);
    try {
      await api.deleteProviderProfile(target.id);
      const loaded = await load();
      if (!loaded) return;
      if (editingId === target.id) resetForm();
      notify?.("success", `${target.name} · ${t("providerRemoved")}`);
    } catch (error) {
      reportError(error);
    } finally {
      setDeletingId(null);
    }
  }

  const protocolOptions = Array.from(new Set([...PROTOCOLS, protocol].filter(Boolean)));
  const localProvider = isLocalProvider(name, protocol, baseUrl);
  const nameInvalid = submitted && !name.trim();
  const protocolInvalid = submitted && !protocol.trim();
  const modelInvalid = submitted && !model.trim();
  const credentialInvalid = submitted && ((!localProvider && !credentialRef.trim()) || (Boolean(credentialRef.trim()) && !ENVIRONMENT_VARIABLE.test(credentialRef.trim())));
  const formDisabled = loading || saving || Boolean(deletingId);

  return (
    <section className="settings-card settings-provider" aria-busy={loading || saving}>
      <div className="settings-heading">
        <span className="settings-card-number" aria-hidden="true">03</span>
        <Robot className="settings-heading-icon" size={20} weight="duotone" aria-hidden="true" />
        <div className="settings-heading-copy">
          <h3 className="settings-title">{t("aiProviders")}</h3>
          <p className="settings-description">{t("configureProviders")}</p>
        </div>
      </div>

      {error && !loadError && (
        <>
          <p className="settings-error" role="alert">
            <WarningCircle size={16} weight="fill" aria-hidden="true" />
            <span>{error}</span>
          </p>
          {errorDetail && <p className="settings-error-detail">{errorDetail}</p>}
        </>
      )}

      {loading ? (
        <div className="settings-loading" role="status" aria-live="polite">
          <SpinnerGap className="settings-spinner" size={18} weight="bold" aria-hidden="true" />
          <span>{t("working")}</span>
        </div>
      ) : loadError ? (
        <div className="settings-error-state" role="alert">
          <p className="settings-error">
            <WarningCircle size={16} weight="fill" aria-hidden="true" />
            <span>{t("providerUnavailable")}</span>
          </p>
          <p className="settings-error-detail">{loadError}</p>
          <Button type="button" variant="primary" onClick={() => void load()}>{t("retry")}</Button>
        </div>
      ) : (
        <>
          <div className="settings-presets" aria-labelledby="provider-presets-heading">
            <div className="settings-section-heading">
              <h4 id="provider-presets-heading" className="settings-section-title">{t("providerPresets")}</h4>
              <span className="settings-section-hint">{t("providerPresetsHint")}</span>
            </div>
            {presets.length === 0 ? (
              <p className="settings-empty settings-empty-inline">{t("noPresets")}</p>
            ) : (
              <div className="settings-preset-list">
                {presets.map((preset) => (
                  <Button
                    key={preset.name}
                    type="button"
                    className="settings-preset"
                    disabled={formDisabled}
                    onClick={() => applyPreset(preset.name)}
                  >
                    {preset.name}
                  </Button>
                ))}
              </div>
            )}
          </div>

          <form
            className="settings-form"
            noValidate
            onSubmit={(event) => {
              event.preventDefault();
              void save();
            }}
          >
            <div className="settings-form-heading">
              <h4 className="settings-section-title">{editingId ? t("editProvider") : t("addProvider")}</h4>
              <span className="settings-section-hint">{t("requiredFields")}</span>
            </div>

            <div className="settings-fields">
              <div className="field-group">
                <label className="field-label" htmlFor="provider-name">
                  {t("providerName")} <span className="field-required" aria-hidden="true">*</span>
                </label>
                <input
                  id="provider-name"
                  className="field-control"
                  value={name}
                  autoComplete="organization"
                  required
                  disabled={formDisabled}
                  aria-invalid={nameInvalid}
                  onChange={(event) => setName(event.target.value)}
                />
              </div>

              <div className="field-group">
                <label className="field-label" htmlFor="provider-protocol">
                  {t("protocol")} <span className="field-required" aria-hidden="true">*</span>
                </label>
                <select
                  id="provider-protocol"
                  className="field-control"
                  value={protocol}
                  required
                  disabled={formDisabled}
                  aria-invalid={protocolInvalid}
                  onChange={(event) => setProtocol(event.target.value)}
                >
                  {protocolOptions.map((option) => (
                    <option key={option} value={option}>{option}</option>
                  ))}
                </select>
              </div>

              <div className="field-group">
                <label className="field-label" htmlFor="provider-base-url">{t("baseUrl")}</label>
                <input
                  id="provider-base-url"
                  className="field-control"
                  value={baseUrl}
                  type="url"
                  inputMode="url"
                  placeholder="https://api.example.com/v1"
                  disabled={formDisabled}
                  onChange={(event) => setBaseUrl(event.target.value)}
                />
                <span className="field-hint">{t("baseUrlHint")}</span>
              </div>

              <div className="field-group">
                <label className="field-label" htmlFor="provider-model">
                  {t("model")} <span className="field-required" aria-hidden="true">*</span>
                </label>
                <input
                  id="provider-model"
                  className="field-control"
                  value={model}
                  required
                  disabled={formDisabled}
                  aria-invalid={modelInvalid}
                  onChange={(event) => setModel(event.target.value)}
                />
              </div>

              <div className="field-group field-group-wide">
                <label className="field-label" htmlFor="provider-credential-ref">
                  {t("credentialEnv")} {localProvider ? <span className="field-optional">({t("optional")})</span> : <span className="field-required" aria-hidden="true">*</span>}
                </label>
                <input
                  id="provider-credential-ref"
                  className="field-control"
                  value={credentialRef}
                  type="password"
                  required={!localProvider}
                  disabled={formDisabled}
                  aria-invalid={credentialInvalid}
                  autoCapitalize="characters"
                  autoComplete="off"
                  spellCheck={false}
                  placeholder={localProvider ? t("optional") : "DEEPSEEK_API_KEY"}
                  onChange={(event) => setCredentialRef(event.target.value)}
                />
                <span className="field-hint">{localProvider ? t("localProviderNoKey") : t("credentialHint")}</span>
              </div>
            </div>

            <div className="settings-actions">
              <Button type="submit" variant="primary" disabled={formDisabled}>
                {saving ? (
                  <SpinnerGap className="settings-spinner" size={16} weight="bold" aria-hidden="true" />
                ) : (
                  <FloppyDisk size={16} weight="duotone" aria-hidden="true" />
                )}
                {saving ? t("working") : t("save")}
              </Button>
              <Button type="button" disabled={formDisabled} onClick={resetForm}>
                <X size={16} weight="bold" aria-hidden="true" />
                {t("cancel")}
              </Button>
            </div>
          </form>

          <div className="settings-profiles" aria-labelledby="saved-providers-heading">
            <div className="settings-section-heading">
              <h4 id="saved-providers-heading" className="settings-section-title">{t("savedProviders")}</h4>
              <span className="settings-section-hint">{t("savedProvidersHint")}</span>
            </div>
            {profiles.length === 0 ? (
              <p className="settings-empty">{t("noProviders")}</p>
            ) : (
              <ul className="settings-profile-list">
                {profiles.map((profile) => {
                  const deleting = deletingId === profile.id;
                  return (
                    <li key={profile.id} className="settings-profile-row">
                      <div className="settings-profile-copy">
                        <span className="settings-profile-name">{profile.name}</span>
                        <span className="settings-profile-meta">{profile.protocol} · {profile.model} · {profile.credentialRef.trim() || (isLocalProvider(profile.name, profile.protocol, profile.baseUrl ?? "") ? t("noApiKey") : t("noCredentialReference"))}</span>
                      </div>
                      <div className="settings-profile-actions">
                        <Button type="button" disabled={formDisabled} onClick={() => editProfile(profile)}>
                          <PencilSimple size={15} weight="duotone" aria-hidden="true" />
                          {t("edit")}
                        </Button>
                        <Button
                          type="button"
                          variant="danger"
                          disabled={formDisabled}
                          onClick={() => setDeleteTarget(profile)}
                        >
                          {deleting ? (
                            <SpinnerGap className="settings-spinner" size={15} weight="bold" aria-hidden="true" />
                          ) : (
                            <Trash size={15} weight="duotone" aria-hidden="true" />
                          )}
                          {t("remove")}
                        </Button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        </>
      )}

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        title={t("removeProvider")}
        body={deleteTarget ? `${t("removeProvider")} · ${deleteTarget.name}` : ""}
        confirmLabel={t("remove")}
        cancelLabel={t("cancel")}
        onOpenChange={(open) => {
          if (!open && !deletingId) setDeleteTarget(null);
        }}
        onConfirm={() => void confirmDelete()}
      />
    </section>
  );
}
