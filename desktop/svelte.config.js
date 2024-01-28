import { preprocessMeltUI, sequence } from "@melt-ui/pp";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
export default {
  // Consult https://svelte.dev/docs#compile-time-svelte-preprocess
  // for more information about preprocessors
  preprocess: sequence([vitePreprocess(), preprocessMeltUI()]),
};
