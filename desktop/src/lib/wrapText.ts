// Wraps text content substring of element with a class
export function wrapText(
  element: HTMLElement,
  className: string,
  a: number,
  b: number,
): number {
  console.log("wrapText", element, className, a, b);
  let currentTextLength = 0;
  let replaceMap: [ChildNode, [string, ChildNode, string]][] = [];
  for (const child of element.childNodes) {
    if (child.nodeType === 3) {
      const start = a - currentTextLength;
      const end = b - currentTextLength;

      console.log(child.textContent, start, end);

      const textLength = child.textContent!.length;

      const startContent = child.textContent!.slice(
        0,
        Math.max(Math.min(start, textLength), 0),
      );
      const middleContent = child.textContent!.slice(
        Math.max(Math.min(start, textLength), 0),
        Math.max(Math.min(end, textLength), 0),
      );
      const endContent = child.textContent!.slice(
        Math.max(Math.min(end, textLength), 0),
        textLength,
      );

      const middleEl = document.createElement("span");
      middleEl.textContent = middleContent;
      middleEl.classList.add(className);

      replaceMap.push([child, [startContent, middleEl, endContent]]);

      currentTextLength += textLength;
    } else {
      currentTextLength += wrapText(
        child as HTMLElement,
        className,
        a - currentTextLength,
        b - currentTextLength,
      );
    }
  }

  for (const [node, [start, middle, end]] of replaceMap) {
    if (middle.textContent!.length === 0) {
      node.replaceWith(
        document.createTextNode(start),
        document.createTextNode(end),
      );
    } else {
      node.replaceWith(
        document.createTextNode(start),
        middle,
        document.createTextNode(end),
      );
    }
  }

  return currentTextLength;
}
