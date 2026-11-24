<script lang="ts">
  // ianox — render de markdown saneado (mismo resultado que react-markdown + gfm)
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { onMount, tick } from "svelte";

  let { source }: { source: string } = $props();

  /** Inline code that names a file or folder, e.g. `src/app.svelte.ts` or `locales/*.po`. */
  const looksLikePath = (text: string) =>
    !/\s/.test(text) && (/[\\/]/.test(text) || /^[\w.-]+\.[a-z][a-z0-9]{0,5}$/i.test(text));

  const html = $derived(
    (DOMPurify.sanitize(
      marked.parse(source ?? "", { async: false, gfm: true, breaks: false }) as string
    ) as string).replace(/(?<!<pre>)<code>([^<]*)<\/code>/g, (whole, text: string) =>
      looksLikePath(text.replace(/&[a-z]+;/g, "")) ? `<code class="path">${text}</code>` : whole
    )
  );

  const COPY_SVG = `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
  `;

  const CHECK_SVG = `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  `;

  // Wrap each <pre><code> block with a container that holds a copy button.
  const htmlWithCopy = $derived.by(() => {
    return html.replace(
      /<pre><code[^>]*>([\s\S]*?)<\/code><\/pre>/g,
      (match) => {
        return `<div class="code-block-wrapper" style="position: relative;">
  <button class="copy-code-btn" type="button" title="Copy" aria-label="Copy">${COPY_SVG}</button>
  ${match}
</div>`;
      }
    );
  });

  let mdRef: HTMLDivElement;

  onMount(async () => {
    await tick();
    attachCopyListeners(mdRef);
  });

  function attachCopyListeners(container: HTMLElement | undefined): void {
    if (!container) return;
    container.removeEventListener("click", handleCopyClick);
    container.addEventListener("click", handleCopyClick);
  }

  function handleCopyClick(event: Event): void {
    const target = event.target as HTMLElement;
    const btn = target.closest(".copy-code-btn");
    if (!btn) return;

    const wrapper = btn.closest(".code-block-wrapper");
    if (!wrapper) return;

    const codeEl = wrapper.querySelector("code");
    if (!codeEl) return;

    const text = codeEl.innerText || codeEl.textContent || "";

    navigator.clipboard.writeText(text).then(() => {
      btn.innerHTML = CHECK_SVG;
      setTimeout(() => {
        btn.innerHTML = COPY_SVG;
      }, 1500);
    });
  }
</script>

<div class="md" bind:this={mdRef}>{@html htmlWithCopy}</div>

<style>
  :global(.code-block-wrapper) {
    position: relative;
    margin: 1em 0;
  }

  :global(.copy-code-btn) {
    position: absolute;
    top: 6px;
    right: 6px;
    z-index: 10;
    padding: 4px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    cursor: pointer;
    opacity: 1;
    transition: background 0.15s, border-color 0.15s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  :global(.copy-code-btn svg) {
    width: 14px;
    height: 14px;
  }

  :global(.copy-code-btn:hover) {
    background: var(--accent-primary);
    border-color: var(--accent-primary);
  }
</style>