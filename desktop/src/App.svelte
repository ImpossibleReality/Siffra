<script lang="ts">
  import Editor from "./lib/Editor.svelte";
  import Titlebar from "./lib/Titlebar.svelte";
  import { onMount } from "svelte";
  import { appWindow } from "@tauri-apps/api/window";

  onMount(() => {
    appWindow.show();
  });
</script>
<Titlebar />

<main>
  <Editor />
</main>

<style>
  main {
    box-sizing: border-box;
    height: 100vh;
    overflow-y: scroll;
  }

  main::-webkit-scrollbar {
    display: none;
  }
</style>
