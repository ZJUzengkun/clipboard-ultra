// 内容形态检测：纯正则、渲染时现算、不落库。
// kind 只是展示概念，数据库 content_type 仍只有 text/image。

export type ContentKind = "color" | "url" | "text";

// 整串匹配颜色值：#hex / rgb() / rgba() / hsl() / hsla()
const COLOR_RE =
  /^(#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})|rgba?\(\s*[\d.,\s%]+\)|hsla?\(\s*[\d.,\s%deg]+\))$/i;

// 整串匹配单行 URL
const URL_RE = /^https?:\/\/\S+$/i;

export function detectKind(text: string): ContentKind {
  const t = text.trim();
  if (t.length <= 64 && COLOR_RE.test(t)) return "color";
  if (t.length <= 2048 && !t.includes("\n") && URL_RE.test(t)) return "url";
  return "text";
}

export function extractDomain(url: string): string {
  try {
    return new URL(url.trim()).hostname;
  } catch {
    return url.trim();
  }
}
