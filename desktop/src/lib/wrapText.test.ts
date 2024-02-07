/// <reference lib="dom" />

import { wrapText } from "./wrapText";

describe("wrapText", () => {
  let element: HTMLElement;
  let className: string;
  let a: number;
  let b: number;

  beforeEach(() => {
    element = document.createElement("div");
    className = "test-class";
    a = 0;
    b = 0;
  });

  it("wraps text content substring of element with a class", () => {
    element.textContent = "Hello, world!";
    a = 0;
    b = 5;
    wrapText(element, className, a, b);
    expect(element.innerHTML).toBe(
      '<span class="test-class">Hello</span>, world!',
    );
  });

  it("does not wrap text if a and b are the same", () => {
    element.textContent = "Hello, world!";
    a = 5;
    b = 5;
    wrapText(element, className, a, b);
    expect(element.innerHTML).toBe("Hello, world!");
  });

  it("wraps text content substring of child element with a class", () => {
    const childElement = document.createElement("span");
    childElement.textContent = "Hello, world!";
    element.appendChild(childElement);
    a = 0;
    b = 5;
    wrapText(element, className, a, b);
    expect(childElement.innerHTML).toBe(
      '<span class="test-class">Hello</span>, world!',
    );
  });

  it("does not wrap text if a and b are out of range", () => {
    element.textContent = "Hello, world!";
    a = 50;
    b = 55;
    wrapText(element, className, a, b);
    expect(element.innerHTML).toBe("Hello, world!");
  });

  it("wraps text nested inside child elements", () => {
    element.innerHTML = "<span>Hello <span>there</span><br /> world!</span>";
    a = 3;
    b = 15;
    wrapText(element, className, a, b);
    expect(element.innerHTML).toBe(
      '<span>Hel<span class="test-class">lo </span><span><span class="test-class">there</span></span><br><span class="test-class"> wor</span>ld!</span>',
    );
  });
});
