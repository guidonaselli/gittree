export interface Token {
  text: string;
  class: string | null;
}

const KEYWORDS = new Set([
  "if", "else", "for", "while", "return", "function", "const", "let", "var",
  "pub", "fn", "impl", "struct", "enum", "match", "use", "mod", "async", "await",
  "class", "def", "import", "export", "from", "as", "in", "of", "new", "this",
  "self", "true", "false", "null", "None", "static", "type", "interface",
  "trait", "extends", "implements", "try", "catch", "finally", "throw",
  "yield", "break", "continue", "switch", "case", "default",
]);

const TOKEN_RE =
  /(\/\/[^\n]*)|(#[^\n]*)|("(?:[^"\\]|\\.)*")|('(?:[^'\\]|\\.)*')|(`(?:[^`\\]|\\.)*`)|(\b\d+(?:\.\d+)?\b)|([A-Za-z_][A-Za-z0-9_]*)|(\s+)|(.)/g;

/** Heuristic regex tokenizer: comments, strings, numbers, a common keyword set. */
export function highlightLine(line: string): Token[] {
  const tokens: Token[] = [];
  TOKEN_RE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = TOKEN_RE.exec(line)) !== null) {
    const [, comment, hashComment, dquote, squote, backtick, number, word, space, other] = match;
    if (comment !== undefined || hashComment !== undefined) {
      tokens.push({ text: match[0], class: "tok-comment" });
    } else if (dquote !== undefined || squote !== undefined || backtick !== undefined) {
      tokens.push({ text: match[0], class: "tok-string" });
    } else if (number !== undefined) {
      tokens.push({ text: match[0], class: "tok-number" });
    } else if (word !== undefined) {
      tokens.push({ text: match[0], class: KEYWORDS.has(word) ? "tok-keyword" : null });
    } else if (space !== undefined || other !== undefined) {
      tokens.push({ text: match[0], class: null });
    }
  }
  return tokens;
}
