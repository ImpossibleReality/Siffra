<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api";
  import Result from "./Result.svelte";
  import { wrapText } from "./wrapText";

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
    length: number;
    output: {
      isErr: boolean;
      value?: string;
      error_message?: string;
      error_description?: string;
      error_span?: [number, number];
      error_location?: string;
    };
    startPadding: number;
    endPadding: number;
  }[] = [];

  const operation_regex =
    /(=|\+|plus|-|minus|times|of|\/|over|divided by|divide by|by|\*|\^)/dg;
  const number_regex = /((?:\d|,|_)+(?:\.(?:\d|,|_)+)?(?:E(?:\d|,|_)+)?)/dg;
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

  async function update_input() {
    displayedEl!.innerText = inputEl!.innerText;
    let result_promise = update_result();
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

    // Shows "Enter your calculation..." when all elements are cleared
    if (inputEl!.innerText.trim() === "") {
      inputEl!.innerText = "";
    }

    // Restore cursor position
    if (selection) {
      const range = document.createRange();
      range.setStart(selection.anchorNode!, selection.anchorOffset);
      range.setEnd(selection.focusNode!, selection.focusOffset);
      selection.removeAllRanges();
      selection.addRange(range);
    }

    // split by newlines and create a div for each line
    let lines = inputEl!.innerText.split("\n");
    let html = "";
    for (let line of lines) {
      if (line.trim() === "") {
        html += `<div class="line">&nbsp;</div>`;
      } else {
        html += `<div class="line"><span class="line-inner">${line}</span></div>`;
      }
    }
    linesEl!.innerHTML = html;

    await update_line_size();

    highlight_matches(
      displayedEl!,
      new Map([
        ["comment", comment_regex],
        ["number", number_regex],
        ["operation", operation_regex],
      ]),
    );

    // Highlight errors

    await result_promise;

    let currentChars = 0;
    for (const line of lineData) {
      if (line.output.isErr) {
        const span = line.output.error_span!;
        wrapText(
          displayedEl!,
          "error",
          currentChars + span[0],
          currentChars + span[1],
        );
      }
      currentChars += line.length;
    }
  }

  async function update_line_size() {
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
            length: 0,
            output: {
              isErr: false,
              value: "",
            },
            startPadding: 0,
            endPadding: 0,
          };
        }
        lineData[i].length = lineEl.textContent!.length;
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
    // Replace nbsp with space
    const input = inputEl!.innerText.replace(/\u00a0/g, " ");
    const data: {
      line: number;
      output: {
        Value: {
          string: string;
        };
        Error: {
          message: string;
          description: string;
          span: [number, number];
          location?: string;
        };
      };
    }[] = await invoke("get_result", {
      input,
    });
    lineData = lineData.map((line) => {
      line.output = {
        isErr: false,
        value: "",
      };
      return line;
    });
    for (const output of data) {
      if (typeof lineData[output.line] === "undefined") {
        lineData[output.line] = {
          height: 0,
          length: 0,
          output: {
            isErr: false,
            value: "",
          },
          startPadding: 0,
          endPadding: 0,
        };
      }

      if (output.output.Value) {
        lineData[output.line].output = {
          isErr: false,
          value: output.output.Value.string || "",
        };
      } else if (output.output.Error) {
        console.log("error", output.output.Error);
        lineData[output.line].output = {
          isErr: true,
          error_message: output.output.Error.message,
          error_description: output.output.Error.description,
          error_span: output.output.Error.span,
          error_location: output.output.Error.location,
        };
      }
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
          class:active={(!!line.output.value || line.output.isErr) &&
            window.innerWidth > 700}
        ></span>
      </div>
    {/each}
  </div>
  <div class="input-wrapper">
    <div class="anchor-el">
      <div
        class="input-el"
        contenteditable="plaintext-only"
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
        <Result data-line={i} output={line.output} />
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
  .displayed-el :global(.error) {
    position: relative;
    text-decoration: none;
    border-bottom: 2px rgba(255, 40, 40, 0.8) solid;
  }
  .displayed-el :global(.error)::before {
    content: " ";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(255, 40, 40, 0.07);
    pointer-events: none;
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
