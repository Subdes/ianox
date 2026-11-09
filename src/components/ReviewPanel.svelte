<script lang="ts">
  import { t } from "../i18n";
  // ianox — Review tab: git status and per-file diff
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, untrack } from "svelte";
  import Icon from "../components/Icon.svelte";
  import EmptyState from "../components/EmptyState.svelte";

  interface GitFile {
    path: string;
    status: string;
    staged: boolean;
    untracked: boolean;
  }

  interface GitStatus {
    is_repo: boolean;
    branch: string | null;
    files: GitFile[];
  }

  let { project }: { project: string | null } = $props();

  let status = $state<GitStatus | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state("");
  let selected = $state<string | null>(null);
  let diff = $state("");
  let diffLoading = $state(false);
  let initializing = $state(false);

  const visibleFiles = $derived(
    (status?.files ?? []).filter((f) =>
      filter ? f.path.toLowerCase().includes(filter.toLowerCase()) : true
    )
  );

  const selectedFile = $derived((status?.files ?? []).find((f) => f.path === selected) ?? null);
  const diffLines = $derived(diff ? diff.split("\n") : []);

  function lineClass(line: string): string {
    if (line.startsWith("+++") || line.startsWith("---")) return "dl meta";
    if (line.startsWith("@@")) return "dl hunk";
    if (line.startsWith("diff --git")) return "dl meta";
    if (line.startsWith("+")) return "dl add";
    if (line.startsWith("-")) return "dl del";
    return "dl ctx";
  }

  onMount(() => {
    let unlisten: UnlistenFn | undefined;
    listen("app://workspace-changed", () => {
      load();
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  });

  async function load(): Promise<void> {
    if (!project) {
      status = null;
      loading = false;
      return;
    }
    loading = true;
    error = null;
    try {
      const next = await invoke<GitStatus>("git_status", { project });
      status = next;
      if (selected && !next.files.some((f) => f.path === selected)) selected = null;
      if (!selected && next.files.length > 0) await selectFile(untrack(() => next.files[0]));
    } catch (e) {
      error = String(e);
      status = null;
    } finally {
      loading = false;
    }
  }

  async function selectFile(file: GitFile): Promise<void> {
    if (!project) return;
    selected = file.path;
    diffLoading = true;
    diff = "";
    try {
      diff = await invoke<string>("git_diff", {
        project,
        file: file.path,
        untracked: file.untracked,
      });
    } catch (e) {
      error = String(e);
    } finally {
      diffLoading = false;
    }
  }

  async function initRepo(): Promise<void> {
    if (!project) return;
    initializing = true;
    error = null;
    try {
      await invoke("git_init", { project });
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      initializing = false;
    }
  }

  async function discard(file: GitFile): Promise<void> {
    if (!project) return;
    if (!confirm(t("Discard changes to {file}?", { file: file.path }))) return;
    try {
      await invoke("git_discard", { project, file: file.path, untracked: file.untracked });
      if (selected === file.path) {
        selected = null;
        diff = "";
      }
      await load();
    } catch (e) {
      error = String(e);
    }
  }

  let loadedProject: string | null | undefined;
  $effect(() => {
    const p = project;
    if (p === loadedProject) return;
    loadedProject = p;
    diff = "";
    selected = null;
    status = null;
    untrack(() => load());
  });
</script>

<div class="review-panel">
  <aside class="review-side">
    <div class="review-side-head">
      <span class="review-side-title">{t("Changes")}</span>
      {#if status?.is_repo && status.branch}
        <span class="review-branch" title={status.branch}>{status.branch}</span>
      {/if}
      <button class="icon-btn" title={t("Refresh")} onclick={load} disabled={loading}>
        <Icon name="refresh" size={14} />
      </button>
    </div>

    {#if status?.is_repo}
      <div class="review-filter">
        <span class="review-filter-icon"><Icon name="search" size={13} /></span>
        <input class="input" placeholder={t("Filter files")} bind:value={filter} spellcheck="false" />
      </div>

      <div class="review-files">
        {#if visibleFiles.length === 0}
          <div class="review-files-empty">{status.files.length === 0 ? t("No changes") : t("No matching files")}</div>
        {/if}
        {#each visibleFiles as file (file.path)}
          <div class={`review-file${selected === file.path ? " active" : ""}`}>
            <button class="review-file-main" onclick={() => selectFile(file)} title={file.path}>
              <span class="review-file-status" data-status={file.status}>{file.status}</span>
              <span class="review-file-name">{file.path}</span>
            </button>
            <button class="review-file-action" title={t("Discard changes")} onclick={() => discard(file)}>
              <Icon name="trash" size={13} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </aside>

  <section class="review-main">
    {#if !project}
      <EmptyState icon="folder" title={t("No project")} desc="Abre un proyecto para revisar sus cambios." />
    {:else if loading && !status}
      <div class="review-center">{t("Loading changes…")}</div>
    {:else if error}
      <div class="review-center" style="color:var(--danger)">⚠ {error}</div>
    {:else if status && !status.is_repo}
      <EmptyState
        icon="diff"
        title={t("No tracked changes")}
        desc="Rastrea, revisa y deshaz cambios en este proyecto."
      >
        <button class="btn btn-primary btn-sm" onclick={initRepo} disabled={initializing}>
          {initializing ? t("Creating…") : t("Create Git repository")}
        </button>
      </EmptyState>
    {:else if status && status.files.length === 0}
      <EmptyState
        icon="check"
        title={t("No uncommitted changes")}
        desc={status.branch ? t("Everything clean on {branch}.", { branch: status.branch }) : t("The working tree is clean.")}
      />
    {:else if selectedFile}
      <div class="review-diff-head">
        <span class="review-file-status" data-status={selectedFile.status}>{selectedFile.status}</span>
        <span class="review-diff-path" title={selectedFile.path}>{selectedFile.path}</span>
        {#if selectedFile.staged}<span class="badge badge-info" style="font-size:10px">{t("staged")}</span>{/if}
      </div>
      <div class="review-diff">
        {#if diffLoading}
          <div class="review-center">{t("Loading diff…")}</div>
        {:else if diff.length === 0}
          <div class="review-center">{t("No text differences (binary file?).")}</div>
        {:else}
          {#each diffLines as line, i (i)}
            <div class={lineClass(line)}>{line === "" ? " " : line}</div>
          {/each}
        {/if}
      </div>
    {:else}
      <div class="review-center">{t("Select a file to see its diff.")}</div>
    {/if}
  </section>
</div>
