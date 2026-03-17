export function sanitizeMarkdownContent(content: string): string {
  return content.replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}
