import { describe, expect, test } from "vitest";
import { sourceFiles, stripComments } from "./support/source-scan";

// Decisions S1 (no network, no persistence, no telemetry) and S2 (untrusted text is never
// interpreted) of story #53's engine layer, read as text over apps/web/src/ (outside the
// generated src/wasm/): a regression is a policy regression.

const FORBIDDEN: [string, RegExp, string][] = [
  ["fetch", /\bfetch\s*\(/, "fetch("],
  ["XMLHttpRequest", /\bXMLHttpRequest\b/, "new XMLHttpRequest()"],
  ["WebSocket", /\bWebSocket\b/, "new WebSocket(u)"],
  ["sendBeacon", /\bsendBeacon\b/, "navigator.sendBeacon(u)"],
  ["EventSource", /\bEventSource\b/, "new EventSource(u)"],
  ["localStorage", /\blocalStorage\b/, "localStorage.getItem(k)"],
  ["sessionStorage", /\bsessionStorage\b/, "sessionStorage.x"],
  ["indexedDB", /\bindexedDB\b/, "indexedDB.open()"],
  ["document.cookie", /\bdocument\.cookie\b/, "document.cookie = x"],
  ["dangerouslySetInnerHTML", /\bdangerouslySetInnerHTML\b/, "<p dangerouslySetInnerHTML={x} />"],
  ["innerHTML", /\binnerHTML\b/, "el.innerHTML = x"],
  ["outerHTML", /\bouterHTML\b/, "el.outerHTML = x"],
  ["insertAdjacentHTML", /\binsertAdjacentHTML\b/, "el.insertAdjacentHTML('a', x)"],
  ["document.write", /\bdocument\.write(ln)?\s*\(/, "document.write(x)"],
  ["eval", /\beval\s*\(/, "eval(x)"],
  ["new Function", /\bnew\s+Function\b/, "new Function('x')"],
  ["setTimeout with a string", /\bset(Timeout|Interval)\s*\(\s*["'`]/, 'setTimeout("x()", 1)'],
  ["Math.random", /\bMath\.random\b/, "Math.random()"],
  ["console other than error", /\bconsole\.(?!error\b)\w+/, "console.log(x)"],
];

describe("the scanner is not blind", () => {
  test.each(FORBIDDEN)("%s is detected in a sample", (_name, re, sample) => {
    expect(re.test(stripComments(sample))).toBe(true);
  });

  test("comments are ignored, console.error is allowed", () => {
    expect(/\bconsole\.(?!error\b)\w+/.test(stripComments("// console.log(1)\n/* fetch( */ console.error(e)"))).toBe(false);
  });

  test("source files are found", () => {
    expect(sourceFiles().length).toBeGreaterThan(5);
  });
});

describe("apps/web/src policy", () => {
  for (const [name, re] of FORBIDDEN) {
    test(`no ${name}`, () => {
      for (const f of sourceFiles()) {
        expect(re.test(stripComments(f.text)), `${f.rel} uses ${name}`).toBe(false);
      }
    });
  }
});
