export function reconstructModifiedFromUnifiedDiff(
  original: string,
  unifiedDiff: string,
): string {
  const origLines = original.split("\n");
  const diffLines = unifiedDiff.split("\n");
  const result: string[] = [];
  let origIdx = 0;
  let inHunk = false;

  for (const line of diffLines) {
    if (line.startsWith("--- ") || line.startsWith("+++ ")) {
      continue;
    }
    if (line.startsWith("@@ ")) {
      const match = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
      if (match) {
        const targetStart = Math.max(0, Number.parseInt(match[1], 10) - 1);
        while (origIdx < targetStart && origIdx < origLines.length) {
          result.push(origLines[origIdx]);
          origIdx += 1;
        }
        inHunk = true;
      }
      continue;
    }
    if (!inHunk) {
      continue;
    }
    if (line.startsWith("+")) {
      result.push(line.slice(1));
    } else if (line.startsWith("-")) {
      origIdx += 1;
    } else if (line.startsWith(" ")) {
      result.push(origIdx < origLines.length ? origLines[origIdx] : line.slice(1));
      origIdx += 1;
    } else if (line.startsWith("\\ No newline at end of file")) {
      continue;
    }
  }

  while (origIdx < origLines.length) {
    result.push(origLines[origIdx]);
    origIdx += 1;
  }

  return result.length > 0 ? result.join("\n") : original;
}
