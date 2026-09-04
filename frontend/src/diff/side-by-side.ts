export interface PairedRow {
  leftIndex: number | null;
  rightIndex: number | null;
  leftText: string | null;
  rightText: string | null;
  isContext: boolean;
}

/** Pairs a run of removals index-wise against the additions that immediately follow it. */
export function pairHunkLines(lines: string[]): PairedRow[] {
  const rows: PairedRow[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.startsWith(" ") || line.length === 0) {
      rows.push({ leftIndex: i, rightIndex: i, leftText: line.slice(1), rightText: line.slice(1), isContext: true });
      i++;
      continue;
    }
    if (line.startsWith("-")) {
      const removedStart = i;
      while (i < lines.length && lines[i].startsWith("-")) i++;
      const removedEnd = i;
      const addedStart = i;
      while (i < lines.length && lines[i].startsWith("+")) i++;
      const addedEnd = i;
      const removedCount = removedEnd - removedStart;
      const addedCount = addedEnd - addedStart;
      const pairs = Math.max(removedCount, addedCount);
      for (let k = 0; k < pairs; k++) {
        const leftIdx = k < removedCount ? removedStart + k : null;
        const rightIdx = k < addedCount ? addedStart + k : null;
        rows.push({
          leftIndex: leftIdx,
          rightIndex: rightIdx,
          leftText: leftIdx !== null ? lines[leftIdx].slice(1) : null,
          rightText: rightIdx !== null ? lines[rightIdx].slice(1) : null,
          isContext: false,
        });
      }
      continue;
    }
    if (line.startsWith("+")) {
      rows.push({ leftIndex: null, rightIndex: i, leftText: null, rightText: line.slice(1), isContext: false });
      i++;
      continue;
    }
    // Anything else (e.g. "\ No newline at end of file") renders as a full-width note.
    rows.push({ leftIndex: i, rightIndex: i, leftText: line, rightText: line, isContext: true });
    i++;
  }
  return rows;
}
