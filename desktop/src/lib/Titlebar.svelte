<script>
  import Toggle from "./Toggle.svelte";
  import { appWindow } from "@tauri-apps/api/window";
  import { platform } from "@tauri-apps/api/os";

  let toggled = false;
  let isMacos = false;

  $: {
    appWindow.setAlwaysOnTop(toggled);
  }
</script>

{#await platform() then platform}
  <div
    data-tauri-drag-region
    class="titlebar"
    class:macos={platform === "darwin"}
  >
    <span class="titlebar-text">Siffra</span>
    <div class="titlebar-menu">
      <Toggle bind:toggled>
        <svg
          width="14px"
          height="14px"
          viewBox="0 0 24 24"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
          stroke-width="1.5"
          ><path
            d="M9.5 14.5L3 21"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          ></path><path
            d="M5.00007 9.48528L14.1925 18.6777L15.8895 16.9806L15.4974 13.1944L21.0065 8.5211L15.1568 2.67141L10.4834 8.18034L6.69713 7.78823L5.00007 9.48528Z"
            fill="currentColor"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          ></path></svg
        >
      </Toggle>
    </div>
  </div>
  <div class="titlebar-padding"></div>
{/await}

<style>
  .titlebar-padding {
    height: var(--nav-height);
  }

  .titlebar {
    height: var(--nav-height);
    width: 100vw;
    position: fixed;
    user-select: none;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: rgb(33, 35, 41);
    z-index: 100;
  }

  .titlebar-text {
    visibility: hidden;
    user-select: none;
    pointer-events: none;
    font-weight: bold;
    font-size: 0.8rem;
  }

  .titlebar.macos .titlebar-text {
    visibility: visible;
  }

  .titlebar-menu {
    position: fixed;
    top: 0;
    right: 0;
    height: var(--nav-height);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0.3rem 0.3rem;
  }
</style>
