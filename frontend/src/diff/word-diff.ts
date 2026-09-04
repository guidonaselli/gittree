export interface DiffSpan {
  text: string;
  changed: boolean;
}

const TOKEN_RE = /(\s+|\w+|[^\s\w])/g;

function tokenize(line: string): string[] {
  return line.match(TOKEN_RE) ?? [];
}

/** Word-level LCS diff between a removed and an added line. */
export function wordDiff(oldLine: string, newLine: string): { oldSpans: DiffSpan[]; newSpans: DiffSpan[] } {
  const a = tokenize(oldLine);
  const b = tokenize(newLine);
  const n = a.length;
  const m = b.length;

  // Standard LCS table.
  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }

  const oldSpans: DiffSpan[] = [];
  const newSpans: DiffSpan[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      oldSpans.push({ text: a[i], changed: false });
      newSpans.push({ text: b[j], changed: false });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      oldSpans.push({ text: a[i], changed: true });
      i++;
    } else {
      newSpans.push({ text: b[j], changed: true });
      j++;
    }
  }
  while (i < n) {
    oldSpans.push({ text: a[i], changed: true });
    i++;
  }
  while (j < m) {
    newSpans.push({ text: b[j], changed: true });
    j++;
  }

  return { oldSpans: mergeAdjacent(oldSpans), newSpans: mergeAdjacent(newSpans) };
}

function mergeAdjacent(spans: DiffSpan[]): DiffSpan[] {
  const out: DiffSpan[] = [];
  for (const span of spans) {
    const last = out[out.length - 1];
    if (last && last.changed === span.changed) {
      last.text += span.text;
    } else {
      out.push({ ...span });
    }
  }
  return out;
}
