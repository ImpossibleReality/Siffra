<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api";

  let inputEl: HTMLElement | null;
  let displayedEl: HTMLElement | null;
  let linesEl: HTMLElement | null;
  let borderEl: HTMLElement | null;
  let outputEl: HTMLElement | null;

  let showBorder = false;

  let lineData: {
    height: number;
    output: string;
  }[] = [];

  const operation_regex =
    /(=|\+|plus|-|minus|times|of|\/|over|divided by|divide by|by|\*|\^)/dg;
  const number_regex = /(\d+(?:\.\d+)?(?:E\d+)?)/dg;

  function escapeHTML(unsafe: String): String {
    return unsafe
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;")
      .replace("\n", "<br>");
  }

  function highlight_matches(
    node: HTMLElement,
    class_map: Map<string, RegExp>,
  ) {
    const match_pairs: [string, [number, number]][] = [];
    for (const [class_name, regex] of class_map) {
      const matches = node.innerText.matchAll(regex);
      for (const match of matches) {
        match_pairs.push([
          class_name,
          [match.index!, match.index! + match[0].length],
        ]);
      }
    }

    // Sort matches by start index
    match_pairs.sort((a, b) => a[1][0] - b[1][0]);

    // Create a new string with the matches highlighted
    let new_string = "";
    let last_index = 0;
    for (const match of match_pairs) {
      new_string += escapeHTML(node.innerText.slice(last_index, match[1][0]));
      new_string += `<span class="${match[0]}">${escapeHTML(
        node.innerText.slice(match[1][0], match[1][1]),
      )}</span>`;
      last_index = match[1][1];
    }

    // Add the rest of the string
    new_string += escapeHTML(node.innerText.slice(last_index));

    // Set the innerHTML of the node
    node.innerHTML = new_string;
  }

  async function update_syntax_highlighting() {
    highlight_matches(
      displayedEl!,
      new Map([
        ["number", number_regex],
        ["operation", operation_regex],
      ]),
    );
  }

  async function update_line_size() {
    // Get current cursor position
    const selection = window.getSelection();
    // Remove elements with style properties
    inputEl!.querySelectorAll("[style]").forEach((el) => {
      el.removeAttribute("style");
    });
    // Remove any elements that are not divs/breaks
    inputEl!.querySelectorAll(":not(div, br)").forEach((el) => {
      el.parentNode!.replaceChild(
        document.createTextNode((el as HTMLElement).innerText),
        el,
      );
    });

    // Restore cursor position
    if (selection) {
      const range = document.createRange();
      range.setStart(selection.anchorNode!, selection.anchorOffset);
      range.setEnd(selection.focusNode!, selection.focusOffset);
      selection.removeAllRanges();
      selection.addRange(range);
    }

    displayedEl!.innerText = inputEl!.innerText;

    // split by newlines and create a div for each line
    let lines = inputEl!.innerText.trim().split("\n");
    let html = "";
    for (let line of lines) {
      if (line.trim() === "") {
        html += `<div class="line">&nbsp;</div>`;
      } else {
        html += `<div class="line">${line}</div>`;
      }
    }
    linesEl!.innerHTML = html;
    const { lineHeight } = getComputedStyle(linesEl!);
    setTimeout(() => {
      let lineEls = linesEl!.querySelectorAll(".line");
      let i = 0;
      lineData = lineData.map((line) => {
        line.height = 0;
        return line;
      });
      showBorder = false;
      for (let lineEl of lineEls) {
        if (typeof lineData[i] === "undefined") {
          lineData[i] = {
            height: 0,
            output: "",
          };
        }
        lineData[i].height = lineEl.getBoundingClientRect().height;
        if (lineData[i].height > parseInt(lineHeight.split("px")[0])) {
          showBorder = true;
          lineEl.classList.add("wrapped");
        } else {
          lineEl.classList.remove("wrapped");
        }
        i++;
      }
    }, 0);

    // Update syntax highlighting
    await update_syntax_highlighting();
  }

  async function update_result() {
    const input = inputEl!.innerText.trim();
    const data: {
      line: number;
      output: string;
    }[] = await invoke("get_result", {
      input,
    });
    lineData = lineData.map((line) => {
      line.output = "";
      return line;
    });
    for (const output of data) {
      lineData[output.line].output = output.output;
    }
  }

  onMount(() => {
    update_line_size();
    window.addEventListener("resize", update_line_size);
    borderEl!.addEventListener("mousedown", (e) => {
      e.preventDefault();
      const onMouseMove = (e: MouseEvent) => {
        let newPercent = 1 - e.clientX / window.innerWidth;
        outputEl!.style.width = `${newPercent * 100}%`;
      };
      const onMouseUp = () => {
        window.removeEventListener("mousemove", onMouseMove);
        window.removeEventListener("mouseup", onMouseUp);
      };
      window.addEventListener("mousemove", onMouseMove);
      window.addEventListener("mouseup", onMouseUp);
    });
    return () => {
      window.removeEventListener("resize", update_line_size);
    };
  });
</script>

<div class="editor-wrapper">
  <div class="input-wrapper">
    <div class="anchor-el">
      <div
        class="input-el"
        contenteditable="true"
        autocomplete="off"
        autocorrect="off"
        autocapitalize="off"
        spellcheck="false"
        bind:this={inputEl}
        on:input={update_line_size}
        on:input={update_result}
      ></div>
      <div bind:this={displayedEl} class="displayed-el"></div>
      <div bind:this={linesEl} class="lines-el"></div>
    </div>
  </div>
  <div class="border-anchor">
    <div class="border-container" bind:this={borderEl} class:shown={showBorder}>
      <div class="border"></div>
    </div>
  </div>
  <div class="output-container" bind:this={outputEl}>
    {#each lineData as line, i}
      <div style="height: {line.height}px" class="output-line">
        <span
          class="output"
          on:click={(e) => navigator.clipboard.writeText(e.target.innerText)}
          >{line.output}</span
        >
      </div>
    {/each}
  </div>
</div>

<style>
  .editor-wrapper {
    width: 100%;
    overflow-x: hidden;
    font-size: 1rem;
    font-family: "Roboto Mono", monospace;
    display: flex;
    flex-direction: row;
    min-height: calc(100vh - var(--nav-height));
  }
  .input-wrapper {
    padding: 1rem;
    flex-grow: 1;
    flex-shrink: 1;
    overflow-x: hidden;
  }
  .anchor-el {
    position: relative;
    overflow-x: hidden;
  }
  .input-el,
  .displayed-el,
  .lines-el {
    outline: none;
    font-size: 1rem;
    line-height: 1.8;
    font-family: "Roboto Mono", monospace;
  }
  .input-el {
    width: 100%;
    min-height: calc(100vh - var(--nav-height) - 2rem);
    color: transparent;
  }
  .input-el:empty::after {
    content: "Enter your calculations...";
    color: #4b5263;
  }
  .lines-el,
  .displayed-el {
    user-select: none;
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    overflow-x: hidden;
    pointer-events: none;
    overflow-wrap: anywhere;
  }

  .displayed-el :global(.number) {
    color: #dec4ab;
  }
  .displayed-el :global(.operation) {
    color: #90bee3;
  }
  .lines-el {
    visibility: hidden;
  }
  .lines-el :global(.line) {
    overflow-x: hidden;
    overflow-wrap: anywhere;
  }
  .output-container {
    padding: 1rem;
    overflow-y: scroll;
    min-width: 6rem;
    width: 40%;
    max-width: calc(100% - 8rem);
    text-align: right;
    flex-shrink: 0;
    user-select: none;
  }
  .output-line {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: flex-end;
  }
  .output {
    padding: 0 0.5rem;
    display: inline-block;
    border-radius: 0.3rem;
    color: #98c379;
    user-select: none;
    -webkit-user-select: none;
    cursor: default;
    transition:
      background-color 0.2s ease-in-out,
      color 0.2s ease-in-out,
      font-weight 0.2s ease-in-out;
  }

  .output:hover {
    background-color: #98c379;
    color: #282c34;
    font-weight: bold;
  }

  .output:active {
    background-color: #719657;
    color: #282c34;
    font-weight: bold;
  }

  .border-anchor {
    position: relative;
    flex-shrink: 0;
    width: 1px;
  }
  .border-container {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    opacity: 0;
    transition: opacity 0.2s ease-in-out;
    cursor: col-resize;
    padding: 0 0.5rem;
  }
  .border-container:hover {
    opacity: 1;
  }
  .border-container.shown {
    opacity: 1;
  }

  .border {
    width: 1px;
    height: 100%;
    background: #4b5263;
  }
</style>
