// UI localization: one dictionary per language, English as the source and fallback.
// `t()` reads reactive state, so templates re-render when the language changes.
import { en, type MessageKey, type Messages } from "./en";
import { tr } from "./tr";
import { de } from "./de";
import { es } from "./es";
import { fr } from "./fr";
import { ptBR } from "./pt-BR";
import { ru } from "./ru";
import { ar } from "./ar";
import { zhCN } from "./zh-CN";
import { ja } from "./ja";
import { ko } from "./ko";

export type Lang = "en" | "de" | "es" | "fr" | "pt-BR" | "ru" | "tr" | "ar" | "zh-CN" | "ja" | "ko";

type LangInfo = { code: Lang; name: string; dir: "ltr" | "rtl"; messages: Messages };

// Listed by their own name so users can find their language whatever the current one is.
export const LANGUAGES: LangInfo[] = [
  { code: "en", name: "English", dir: "ltr", messages: en },
  { code: "de", name: "Deutsch", dir: "ltr", messages: de },
  { code: "es", name: "Español", dir: "ltr", messages: es },
  { code: "fr", name: "Français", dir: "ltr", messages: fr },
  { code: "pt-BR", name: "Português (Brasil)", dir: "ltr", messages: ptBR },
  { code: "ru", name: "Русский", dir: "ltr", messages: ru },
  { code: "tr", name: "Türkçe", dir: "ltr", messages: tr },
  { code: "ar", name: "العربية", dir: "rtl", messages: ar },
  { code: "zh-CN", name: "中文（简体）", dir: "ltr", messages: zhCN },
  { code: "ja", name: "日本語", dir: "ltr", messages: ja },
  { code: "ko", name: "한국어", dir: "ltr", messages: ko },
];

const byCode = new Map(LANGUAGES.map((l) => [l.code, l]));

/** Best match for the system's preferred languages (`zh-TW` → `zh`, `pt-PT` → `pt-BR`…), else English. */
export function detectLang(preferred: readonly string[] = navigator.languages ?? [navigator.language]): Lang {
  for (const p of preferred) {
    const tag = p.toLowerCase();
    const exact = LANGUAGES.find((l) => l.code.toLowerCase() === tag);
    if (exact) return exact.code;
    const base = tag.split("-")[0];
    const sameBase = LANGUAGES.find((l) => l.code.toLowerCase().split("-")[0] === base);
    if (sameBase) return sameBase.code;
  }
  return "en";
}

/** Current UI language; `setLanguage(null)` follows the system language. */
export const i18n = $state<{ lang: Lang }>({ lang: "en" });

export function setLanguage(choice: string | null | undefined) {
  const lang = choice && byCode.has(choice as Lang) ? (choice as Lang) : detectLang();
  i18n.lang = lang;
  const info = byCode.get(lang)!;
  document.documentElement.lang = lang;
  document.documentElement.dir = info.dir;
}

/** Translate `key`, filling `{name}` placeholders from `vars`. Missing text falls back to English. */
export function t(key: MessageKey, vars?: Record<string, string | number>): string {
  const text = byCode.get(i18n.lang)?.messages[key] || en[key] || key;
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (m, name) => (name in vars ? String(vars[name]) : m));
}
