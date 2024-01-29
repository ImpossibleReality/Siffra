<script lang="ts">
  import { afterUpdate, onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api";

  let inputEl: HTMLElement | null;
  let displayedEl: HTMLElement | null;
  let linesEl: HTMLElement | null;
  let borderEl: HTMLElement | null;
  let outputEl: HTMLElement | null;
  let editorWrapperEl: HTMLElement | null;
  let rulerWrapperEl: HTMLElement | null;

  let showBorder = false;

  let lineData: {
    height: number;
    output: string;
    startPadding: number;
    endPadding: number;
  }[] = [];

  const operation_regex =
    /(=|\+|plus|-|minus|times|of|\/|over|divided by|divide by|by|\*|\^)/dg;
  const number_regex = /(\d+(?:\.\d+)?(?:E\d+)?)/dg;
  const comment_regex = /(\/\/.*|\/\*.*\*\/)/dg;

  function escapeHTML(unsafe: String): String {
    return unsafe
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#039;")
      .replace(/\n/g, "<br />");
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

    // Remove any matches that overlap. Give preference to the first class in the map
    let i = 0;
    while (i < match_pairs.length - 1) {
      if (match_pairs[i][1][1] > match_pairs[i + 1][1][0]) {
        match_pairs.splice(i + 1, 1);
      } else {
        i++;
      }
    }

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

  function update_input() {
    update_result();
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

    displayedEl!.innerText = inputEl!.innerText;

    // Restore cursor position
    if (selection) {
      const range = document.createRange();
      range.setStart(selection.anchorNode!, selection.anchorOffset);
      range.setEnd(selection.focusNode!, selection.focusOffset);
      selection.removeAllRanges();
      selection.addRange(range);
    }

    // split by newlines and create a div for each line
    let lines = inputEl!.innerText.trim().split("\n");
    let html = "";
    for (let line of lines) {
      if (line.trim() === "") {
        html += `<div class="line">&nbsp;</div>`;
      } else {
        html += `<div class="line"><span class="line-inner">${line}</span></div>`;
      }
    }
    linesEl!.innerHTML = html;

    update_line_size();

    highlight_matches(
      displayedEl!,
      new Map([
        ["comment", comment_regex],
        ["number", number_regex],
        ["operation", operation_regex],
      ]),
    );
  }

  function update_line_size() {
    // Increase the font size in increments of 1 px as editor gets bigger
    const fontSize = Math.max(
      Math.min(1.1, 0.9 + (window.innerWidth - 300) / 800),
      0.9,
    ).toFixed(1);

    editorWrapperEl!.style.setProperty("--font-size", `${fontSize}rem`);

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
            startPadding: 0,
            endPadding: 0,
          };
        }
        lineData[i].height = lineEl.getBoundingClientRect().height;
        if (lineData[i].height > parseInt(lineHeight.split("px")[0])) {
          showBorder = true;
          lineEl.classList.add("wrapped");
        } else {
          lineEl.classList.remove("wrapped");
        }
        lineData[i].startPadding = lineEl.children[0]
          ? lineEl.children[0].getBoundingClientRect().width
          : 0;
        i++;
      }
    }, 0);
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
    await tick();
    for (const el of outputEl!.querySelectorAll(".output")) {
      lineData[parseInt(el.dataset.line!)].endPadding =
        el.getBoundingClientRect().width;
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
        update_line_size();
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

<div class="editor-wrapper" bind:this={editorWrapperEl}>
  <div class="ruler-wrapper" bind:this={rulerWrapperEl}>
    {#each lineData as line, i}
      <div style="height: {line.height}px" class="ruler-line">
        <span
          class="ruler"
          style="margin-left: {line.startPadding +
            10}px; margin-right: {line.endPadding + 5}px;"
          class:active={!!line.output && window.innerWidth > 700}
        ></span>
      </div>
    {/each}
  </div>
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
        on:input={update_input}
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
          data-line={i}>{line.output}</span
        >
      </div>
    {/each}
  </div>
</div>

<style>
  .editor-wrapper {
    position: relative;
    width: 100%;
    overflow-x: hidden;
    font-size: 1rem;
    font-family: "Roboto Mono", monospace;
    display: flex;
    flex-direction: row;
    min-height: calc(100vh - var(--nav-height));
  }
  .ruler-wrapper {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 1rem;
  }
  .ruler-line {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: flex-end;
  }

  .ruler {
    height: 2px;
    width: 100%;
    transition: background-color 0.1s ease-in-out;
  }

  .ruler.active {
    background-color: rgba(255, 255, 255, 0.03);
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
    font-size: var(--font-size);
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
  .displayed-el :global(.comment) {
    color: #727b8c;
  }
  .lines-el {
    color: transparent;
  }
  .lines-el :global(.line) {
    overflow-x: hidden;
    overflow-wrap: anywhere;
  }
  .output-container {
    padding: 1rem;
    overflow-y: auto;
    -ms-overflow-style: none;
    scrollbar-width: none;
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
    position: relative;
    padding: 0 0.5rem;
    display: inline-block;
    font-size: var(--font-size);
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
    font-weight: bolder;
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
