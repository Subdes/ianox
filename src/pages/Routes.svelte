<script lang="ts">
  import { t } from "../i18n";
  // ianox — routes page
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import type { RouteInfo, RouteConfigInput, ProviderInfo } from "../types";
  import Modal from "../components/Modal.svelte";
  import FormField from "../components/FormField.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Icon from "../components/Icon.svelte";

  function defaultForm(): RouteConfigInput {
    return {
      id: "",
      name: "",
      enabled: true,
      model_pattern: "*",
      provider_ids: [],
      fallback: false,
      weight: 1,
    };
  }

  let { compact = false }: { compact?: boolean } = $props();

  let routes = $state<RouteInfo[]>([]);
  let providers = $state<ProviderInfo[]>([]);
  let loading = $state(true);
  let showAddModal = $state(false);
  let editingRoute = $state<RouteInfo | null>(null);
  let form = $state<RouteConfigInput>(defaultForm());

  onMount(() => {
    let unlistenRoutes: UnlistenFn | null = null;
    let unlistenProviders: UnlistenFn | null = null;

    Promise.all([invoke<RouteInfo[]>("get_routes"), invoke<ProviderInfo[]>("list_providers")])
      .then(([routesData, providersData]) => {
        routes = routesData;
        providers = providersData.filter((p) => p.enabled);
      })
      .catch((e) => console.error("Error cargando datos:", e))
      .finally(() => (loading = false));

    listen<RouteInfo[]>("routes://updated", (e) => (routes = e.payload))
      .then((fn) => (unlistenRoutes = fn))
      .catch(() => {});
    listen<ProviderInfo[]>("providers://updated", (e) => {
      providers = e.payload.filter((p) => p.enabled);
    })
      .then((fn) => (unlistenProviders = fn))
      .catch(() => {});

    return () => {
      unlistenRoutes?.();
      unlistenProviders?.();
    };
  });

  async function handleSubmit(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    try {
      if (editingRoute) await invoke("remove_route", { id: editingRoute.id });
      await invoke("add_route", { route: form });
      showAddModal = false;
      editingRoute = null;
      resetForm();
    } catch (e) {
      console.error("Error guardando ruta:", e);
      alert(`${t("Error")}: ${e}`);
    }
  }

  function handleEdit(route: RouteInfo): void {
    editingRoute = route;
    form = { ...route, provider_ids: [...route.provider_ids] };
    showAddModal = true;
  }

  async function handleDelete(id: string): Promise<void> {
    if (!confirm(t("Delete route {id}?", { id }))) return;
    try {
      await invoke("remove_route", { id });
    } catch (e) {
      console.error("Error eliminando ruta:", e);
      alert(`${t("Error")}: ${e}`);
    }
  }

  async function handleToggle(id: string, enabled: boolean): Promise<void> {
    try {
      await invoke("toggle_route", { id, enabled });
    } catch (e) {
      console.error("Error cambiando estado:", e);
    }
  }

  function openAdd(): void {
    resetForm();
    editingRoute = null;
    showAddModal = true;
  }

  function resetForm(): void {
    form = defaultForm();
  }

  const providerName = (id: string) => providers.find((p) => p.id === id)?.name || id;
</script>

{#if loading}
  <div class="page-loading">{t("Loading routes…")}</div>
{:else}
  <div class="ui-page">
    {#if !compact}
      <PageHeader
        title={t("Routing rules")}
        subtitle={t("Define which provider serves each model. Routing patterns, priorities and fallback.")}
      >
        <button class="btn btn-primary" onclick={openAdd}>
          <Icon name="plus" size={15} />
          {t("New route")}
        </button>
      </PageHeader>
    {/if}

    <div class="panel-card">
      <div class="panel-card-bar">
        <span class="panel-card-title">{t("Routes")}</span>
        <span style="font-size:11px;color:var(--text-muted)">{routes.length}</span>
        <div style="flex:1"></div>
        {#if compact}
          <button class="btn btn-primary btn-sm" onclick={openAdd}>
            <Icon name="plus" size={13} />
            {t("New")}
          </button>
        {/if}
      </div>
      <div class="panel-card-body">
        {#if routes.length === 0}
          <EmptyState
            icon="gateway"
            title={t("No routes configured")}
            desc={t("Without routes, routing will use any provider that has the model.")}
          >
            <button class="btn btn-primary btn-sm" onclick={openAdd}>{t("Create route")}</button>
          </EmptyState>
        {:else}
          <div class="list">
            {#each routes as route (route.id)}
              <div class="list-item" style={`opacity:${route.enabled ? 1 : 0.55}`}>
                <div class="list-row">
                  <div class="row-icon"><Icon name="gateway" size={15} /></div>
                  <div class="row-main">
                    <div class="row-title">
                      <span class="truncate">{route.name}</span>
                      <code
                        style="font-size:10.5px;background:var(--bg-tertiary);padding:1px 6px;border-radius:4px;font-family:monospace;flex-shrink:0"
                      >
                        {route.model_pattern}
                      </code>
                      {#if route.fallback}
                        <span class="badge badge-warning" style="font-size:10px;flex-shrink:0">{t("fallback")}</span>
                      {/if}
                    </div>
                    <div class="row-sub">
                      {route.provider_ids.map(providerName).join(" → ") || t("No providers")}
                    </div>
                  </div>
                  <div class="row-meta">
                    <span>{t("weight")}<b>{route.weight}</b></span>
                  </div>
                  <div class="row-actions">
                    <button
                      class={`switch${route.enabled ? " on" : ""}`}
                      title={route.enabled ? "Desactivar ruta" : "Activar ruta"}
                      onclick={() => handleToggle(route.id, !route.enabled)}
                    >
                      <span></span>
                    </button>
                    <button class="icon-btn" title={t("Edit")} onclick={() => handleEdit(route)}>
                      <Icon name="edit" size={15} />
                    </button>
                    <button class="icon-btn danger" title={t("Delete")} onclick={() => handleDelete(route.id)}>
                      <Icon name="trash" size={15} />
                    </button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    {#if showAddModal}
      <Modal
        title={editingRoute ? t("Edit route") : t("New route")}
        maxWidth={640}
        onclose={() => {
          showAddModal = false;
          editingRoute = null;
          resetForm();
        }}
      >
        <form onsubmit={handleSubmit} style="display:flex;flex-direction:column;gap:16px">
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:16px">
            <FormField label="ID" required>
              <input class="input" bind:value={form.id} disabled={!!editingRoute} placeholder={t("route-1")} />
            </FormField>
            <FormField label={t("Name")} required>
              <input class="input" bind:value={form.name} placeholder={t("Main route")} />
            </FormField>
          </div>

          <FormField label={t("Model pattern (glob)")} required>
            <input class="input" bind:value={form.model_pattern} placeholder="gpt-4o*" />
            <p style="font-size:11px;color:var(--text-muted);margin-top:4px">
              {t("Use")} <code>*</code> {t("to match everything, or prefixes like")} <code>gpt-4*</code>,
              <code>claude*</code>, <code>llama*</code>
            </p>
          </FormField>

          <FormField label="Proveedores (orden de prioridad)">
            <div style="display:flex;flex-direction:column;gap:8px;max-height:200px;overflow:auto">
              {#each providers as p (p.id)}
                <label
                  style={`display:flex;align-items:center;gap:8px;cursor:pointer;padding:8px 12px;background:var(--bg-tertiary);border-radius:var(--radius-sm);border:1px solid ${form.provider_ids.includes(p.id) ? "var(--accent-primary)" : "transparent"}`}
                >
                  <input
                    type="checkbox"
                    checked={form.provider_ids.includes(p.id)}
                    onchange={(e) => {
                      form.provider_ids = e.currentTarget.checked
                        ? [...form.provider_ids, p.id]
                        : form.provider_ids.filter((id) => id !== p.id);
                    }}
                  />
                  <span style="font-size:13px;font-weight:500">{p.name}</span>
                  <span style="font-size:11px;color:var(--text-muted);margin-left:auto">{p.kind}</span>
                </label>
              {/each}
            </div>
            <p style="font-size:11px;color:var(--text-muted);margin-top:4px">
              {t("They are tried in this order. Check at least one.")}
            </p>
          </FormField>

          <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:16px">
            <FormField label="Fallback">
              <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
                <input type="checkbox" bind:checked={form.fallback} />
                <span style="font-size:13px">{t("If it doesn't match, try all providers")}</span>
              </label>
            </FormField>
            <FormField label="Peso (1-10)">
              <input
                class="input"
                type="number"
                min={1}
                max={10}
                value={form.weight}
                oninput={(e) => (form.weight = parseInt(e.currentTarget.value))}
              />
            </FormField>
            <FormField label="Habilitada">
              <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
                <input type="checkbox" bind:checked={form.enabled} />
                <span style="font-size:13px">{t("Yes")}</span>
              </label>
            </FormField>
          </div>

          <div style="display:flex;justify-content:flex-end;gap:8px;margin-top:8px">
            <button
              type="button"
              class="btn btn-secondary"
              onclick={() => {
                showAddModal = false;
                editingRoute = null;
                resetForm();
              }}>{t("Cancel")}</button
            >
            <button type="submit" class="btn btn-primary">{editingRoute ? t("Update") : t("Create")}</button>
          </div>
        </form>
      </Modal>
    {/if}
  </div>
{/if}
