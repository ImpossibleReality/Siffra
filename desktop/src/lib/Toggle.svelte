<script>
  import { createToggle } from "@melt-ui/svelte";

  export let toggled = false;
  export let disabled = false;

  const {
    elements: { root },
    states: { pressed: is_pressed },
  } = createToggle({
    disabled,
  });

  is_pressed.subscribe((value) => {
    toggled = value;
  });

  $: {
    $is_pressed = toggled;
  }
</script>

<button use:melt={$root} class="toggle-btn">
  <slot />
</button>

<style>
  .toggle-btn {
    background-color: transparent;
    border: none;
    color: #98a1a4;
    padding: 0.2rem;
    border-radius: 0.25rem;
  }

  .toggle-btn:hover {
    background-color: rgba(255, 255, 255, 0.06);
  }

  .toggle-btn[data-state="on"] {
    color: #fff;
    background-color: rgba(255, 255, 255, 0.1);
  }

  .toggle-btn:active {
    background-color: rgba(255, 255, 255, 0.2);
  }
</style>
