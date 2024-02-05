<script lang="ts">
  import { createTooltip } from "@melt-ui/svelte";

  export let tooltip: string;

  export let placement:
    | "top"
    | "top-start"
    | "top-end"
    | "right"
    | "right-start"
    | "right-end"
    | "bottom"
    | "bottom-start"
    | "bottom-end"
    | "left"
    | "left-start"
    | "left-end"
    | undefined = "top";

  const {
    elements: { trigger, content, arrow },
    states: { open },
  } = createTooltip({
    disableHoverableContent: true,
    positioning: {
      placement,
    },
    openDelay: 1000,
    closeDelay: 0,
    closeOnPointerDown: false,
    forceVisible: true,
  });
</script>

<div use:melt={$trigger}>
  <slot />
</div>

{#if $open}
  <div use:melt={$content} class="tooltip">
    <div use:melt={$arrow} class="arrow" />
    <p>{tooltip}</p>
  </div>
{/if}

<style>
  .tooltip {
    z-index: 10;
    background-color: #3f444d;
    padding: 0.2rem 0.3rem;
    border-radius: 0.3rem;
    border: 1px solid #3f444d;
  }

  .tooltip p {
    margin: 0;
    font-size: 0.8rem;
  }
</style>
