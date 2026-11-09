<script lang="ts">
  // ianox — marca: Terminal prompt (chevron blanco) + Bloque de ejecución local (#1E5FFF)
  let { size = 30, radius = 8 }: { size?: number; radius?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 512 512"
  aria-hidden="true"
  style={`border-radius:${radius}px;display:block`}
>
  <rect x="0" y="0" width="512" height="512" fill="#121318" />
  <g fill="none" stroke-linecap="round" stroke-linejoin="round">
    <path d="M 140 160 L 250 256 L 140 352" stroke="#ffffff" stroke-width="48" />
    <rect x="290" y="226" width="96" height="60" rx="16" fill="var(--accent-primary, #1e5fff)" stroke="none" />
  </g>
</svg>