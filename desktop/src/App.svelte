<script lang="ts">
  import Editor from './lib/Editor.svelte'
  import Titlebar from "./lib/Titlebar.svelte";
</script>

<Titlebar />

<main>
    <Editor />
</main>

<style>
main {
    padding-top: var(--nav-height);
    box-sizing: border-box;
    height: 100vh;
    overflow-y: scroll;
}

    main::-webkit-scrollbar {
        display: none;
    }
</style>