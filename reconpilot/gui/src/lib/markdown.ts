export type MarkdownBlock =
  | { type: "heading"; depth: number; text: string }
  | { type: "bullet"; text: string }
  | { type: "paragraph"; text: string };

export function parseMarkdownBlocks(markdown: string): MarkdownBlock[] {
  const blocks: MarkdownBlock[] = [];
  for (const rawLine of markdown.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }

    const headingMatch = /^(#{1,6})\s+(.*)$/.exec(line);
    if (headingMatch) {
      blocks.push({
        type: "heading",
        depth: headingMatch[1].length,
        text: headingMatch[2]
      });
      continue;
    }

    const bulletMatch = /^[-*]\s+(.*)$/.exec(line);
    if (bulletMatch) {
      blocks.push({
        type: "bullet",
        text: bulletMatch[1]
      });
      continue;
    }

    blocks.push({
      type: "paragraph",
      text: line
    });
  }
  return blocks;
}
