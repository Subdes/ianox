<script lang="ts">
  let { keys }: { keys: string[] } = $props();
</script>

<span class="kbd-group">
  {#each keys as key (key)}<kbd>{key}</kbd>{/each}
</span>
