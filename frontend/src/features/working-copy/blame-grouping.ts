import type { BlameLine } from "../../api/commands";

export interface BlameBlock {
  sha: string;
  author_name: string;
  author_time: number;
  summary: string;
  is_boundary: boolean;
  lines: BlameLine[];
}

/** Groups consecutive blame lines sharing the same commit into one attribution block. */
export function groupBlameLines(lines: BlameLine[]): BlameBlock[] {
  const blocks: BlameBlock[] = [];
  for (const line of lines) {
    const last = blocks[blocks.length - 1];
    if (last && last.sha === line.sha) {
      last.lines.push(line);
    } else {
      blocks.push({
        sha: line.sha,
        author_name: line.author_name,
        author_time: line.author_time,
        summary: line.summary,
        is_boundary: line.is_boundary,
        lines: [line],
      });
    }
  }
  return blocks;
}
