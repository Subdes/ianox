<script lang="ts">
  // ianox — render de markdown saneado (mismo resultado que react-markdown + gfm)
  import { marked } from "marked";
  import DOMPurify from "dompurify";

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
</script>

<div class="md">{@html html}</div>
