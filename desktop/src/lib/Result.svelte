<script lang="ts">
  import { createPopover } from "@melt-ui/svelte";
  import { onMount } from "svelte";
  import Toggle from "./Toggle.svelte";
  import Tooltip from "./Tooltip.svelte";

  export let output: {
    isErr: boolean;
    value?: string;
    error_message?: string;
    error_description?: string;
    error_location?: string;
    error_span?: [number, number];
  };
  export let convert: () => void;

  let buttonEl: HTMLButtonElement;
  let copied = false;
  let precision = 10;
  let scrollAmount = 0;

  $: {
    if (precision > 30 && precision != null) {
      precision = 30;
    } else if (precision < 1 && precision != null) {
      precision = 1;
    }
  }

  $: isEmpty = !output.value && !output.isErr;

  const {
    elements: { trigger, content, arrow },
    states: { open },
  } = createPopover({
    forceVisible: true,
    closeOnOutsideClick: true,
    onOutsideClick: (e) => {
      $open = false;
      e.preventDefault();
    },
    closeOnEscape: false,
  });

  function copyAnswer() {
    navigator.clipboard.writeText(output.value!);
  }

  function escapeEventListener(e: KeyboardEvent) {
    if (e.key === "Escape") {
      $open = false;
    }
  }

  onMount(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // ctrl/cmd + c to copy
      if ((e.ctrlKey || e.metaKey) && e.key === "c" && !output.isErr) {
        e.preventDefault();
        copyAnswer();
        copied = true;
        setTimeout(() => {
          copied = false;
        }, 200);
      }
    }

    if (buttonEl) {
      buttonEl.addEventListener("mouseenter", () => {
        window.addEventListener("keydown", handleKeyDown);
      });
      buttonEl.addEventListener("mouseleave", () => {
        window.removeEventListener("keydown", handleKeyDown);
      });
      window.addEventListener("keydown", escapeEventListener);
    }
  });
</script>

<button
  class="output"
  class:open={$open}
  class:error={output.isErr}
  class:invisible={isEmpty}
  class:copied
  use:melt={$trigger}
  aria-label="Output options"
  bind:this={buttonEl}
  {...$$restProps}>{output.isErr ? output.error_message : output.value}</button
>

{#if $open}
  <div use:melt={$content} class="content">
    <div use:melt={$arrow} class="arrow" />
    {#if !output.isErr}
      <div class="btn-actions">
        <button
          class="btn"
          on:click={() => {
            copyAnswer();
            $open = false;
          }}
        >
          Copy
        </button>
        <button
          class="btn"
          on:click={() => {
            convert();
            $open = false;
          }}
        >
          Convert
        </button>
      </div>
      <div class="precision-selection">
        <label>
          Precision:
          <input
            type="number"
            class="precision-input"
            bind:value={precision}
            min="1"
            max="30"
            on:click={(e) => e.target.select()}
            on:wheel={(e) => {
              e.preventDefault();
              if (e.deltaMode === 0) {
                scrollAmount += e.deltaY;
                if (scrollAmount >= 30) {
                  precision++;
                  scrollAmount = 0;
                } else if (scrollAmount <= -30) {
                  precision--;
                  scrollAmount = 0;
                }
              } else {
                if (e.deltaY < 0) {
                  precision++;
                } else {
                  precision--;
                }
              }
            }}
            on:keypress={(e) => {
              if (e.key === "Enter") {
                //@ts-ignore
                e.target.blur();
                $open = false;
              }
            }}
          />
        </label>
        <div class="precision-text">
          Display up to {precision || "1"} decimal places
        </div>
      </div>
      <div class="formatting-toggles">
        <Tooltip tooltip="Use commas" placement="bottom">
          <Toggle>
            <div class="comma-icon-container"></div>
          </Toggle>
        </Tooltip>
        <Tooltip tooltip="Use scientific notation" placement="bottom">
          <Toggle>
            <div class="atom-icon-container">
              <svg
                width="24px"
                stroke-width="1.5"
                height="24px"
                viewBox="0 0 24 24"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
                color="currentColor"
                ><path
                  d="M4.40434 13.6099C3.51517 13.1448 3 12.5924 3 12C3 10.3431 7.02944 9 12 9C16.9706 9 21 10.3431 21 12C21 12.7144 20.2508 13.3705 19 13.8858"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                ></path><path
                  d="M12 11.01L12.01 10.9989"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                ></path><path
                  d="M16.8827 6C16.878 4.97702 16.6199 4.25309 16.0856 3.98084C14.6093 3.22864 11.5832 6.20912 9.32664 10.6379C7.07005 15.0667 6.43747 19.2668 7.91374 20.019C8.44117 20.2877 9.16642 20.08 9.98372 19.5"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                ></path><path
                  d="M9.60092 4.25164C8.94056 3.86579 8.35719 3.75489 7.91369 3.98086C6.43742 4.73306 7.06999 8.93309 9.32658 13.3619C11.5832 17.7907 14.6092 20.7712 16.0855 20.019C17.3977 19.3504 17.0438 15.9577 15.3641 12.1016"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                ></path></svg
              >
            </div>
          </Toggle>
        </Tooltip>
        <Tooltip tooltip="Use fractions" placement="bottom">
          <Toggle>
            <div class="fraction-icon-container"></div>
          </Toggle>
        </Tooltip>
      </div>
    {:else}
      {output.error_description}
      {#if output.error_location}
        ({output.error_location})
      {/if}
    {/if}
  </div>
{/if}

<style>
  .output {
    position: relative;
    padding: 0.2rem 0.5rem;
    display: inline-block;
    font-size: var(--font-size);
    border-radius: 0.3rem;
    color: #98c379;
    font-weight: bold;
    background-color: transparent;
    border: none;
    user-select: none;
    -webkit-user-select: none;
    cursor: default;
  }

  .output.error {
    color: #ea6868;
  }

  .output:hover {
    background-color: #98c379;
    color: #282c34;
  }

  .output.error:hover {
    background-color: #b93737;
    color: #f3d6d6;
  }

  .output:active {
    background-color: #719657;
    color: #282c34;
  }

  .output.error:active {
    background-color: #981e1e;
    color: #f3d6d6;
  }

  .output.copied {
    background-color: #719657;
    color: #282c34;
  }

  .output.error.open {
    background-color: #981e1e;
    color: #f3d6d6;
  }

  .output.open {
    background-color: #83a969;
    color: #282c34;
  }

  .output.invisible {
    display: none;
  }

  .content {
    padding: 0.7rem;
    background-color: rgba(39, 43, 51, 0.8);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid #383d46;
    border-radius: 0.4rem;
  }

  .content .arrow {
    border-top: 1px solid #383d46;
    border-left: 1px solid #383d46;
    border-top-left-radius: 0.1rem;
  }

  .btn-actions {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.7rem;
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 0.3rem;
    background-color: #383d46;
    color: #a9aeb9;
    font-weight: bold;
    font-size: 1rem;
    flex-grow: 1;
  }

  .btn:hover {
    background-color: #3f444d;
    color: #a9aeb9;
  }

  .formatting-toggles {
    display: flex;
    gap: 0.5rem;
    justify-content: center;
  }

  .comma-icon-container {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.7rem;
    font-weight: bolder;
    width: 24px;
    height: 24px;
  }

  .comma-icon-container::before {
    content: ",";
    transform: translateY(-0.6rem);
  }

  .fraction-icon-container {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.7rem;
    font-weight: bolder;
    width: 24px;
    height: 24px;
  }

  .fraction-icon-container::before {
    content: "½";
    transform: scale(0.7);
  }

  .atom-icon-container {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.7rem;
    font-weight: bolder;
    width: 24px;
    height: 24px;
  }

  .atom-icon-container * {
    transform-origin: center;
    transform: scale(0.85);
  }

  .precision-selection {
    display: flex;
    flex-direction: column;
  }

  .precision-selection label {
    font-size: 0.9rem;
    display: flex;
    gap: 0.5rem;
    align-items: center;
    justify-content: space-between;
  }

  .precision-input {
    padding: 0.3rem 0.5rem;
    border: 1px solid #383d46;
    border-radius: 0.3rem;
    background-color: #282c34;
    color: #a9aeb9;
    font-size: 1rem;
    width: 100%;
  }

  .precision-text {
    display: flex;
    font-size: 0.5rem;
    justify-content: center;
    cursor: default;
  }
</style>
