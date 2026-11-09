#!/usr/bin/env node
/**
 * ianox — flujo de traducciones.
 *
 *   locales/*.po  (fuente, editable con Poedit/Weblate)
 *        │  msgmerge with the template generated from the code
 *        ▼
 *   locales/ianox.pot  (keys extracted from t("..."))
 *        │  .po → JSON
 *        ▼
 *   src/locales/<lang>.json      (lo importa el frontend)
 *   src-tauri/locales/<lang>.json (embedded by Rust with include_str!)
 *
 * JSON instead of .mo so it works the same on Linux, Windows and macOS
 * sin libintl ni dependencias nativas.
 *
 *   npm run i18n            regenera plantilla, mezcla y compila
 *   npm run i18n -- --check only verifies no keys are missing
 */
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";

const ROOT = process.cwd();
const LOCALES_DIR = path.join(ROOT, "locales");
const POT = path.join(LOCALES_DIR, "ianox.pot");
const LOCALES = ["es", "en"];

const checkOnly = process.argv.includes("--check");

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

/** Keys used in the code: t("key", ...) */
function extractKeys() {
  const keys = new Set();
  const files = walk(path.join(ROOT, "src")).filter((f) => /\.(svelte|ts)$/.test(f));
  for (const file of files) {
    if (file.endsWith(path.join("src", "i18n.ts"))) continue;
    const text = readFileSync(file, "utf8");
    for (const match of text.matchAll(/\bt\(\s*"([^"]+)"/g)) keys.add(match[1]);
  }

  // Lists whose entries are translated at render time with t(item.label) / t(item.desc).
  for (const rel of ["src/pages/Settings.svelte", "src/components/Sidebar.svelte"]) {
    const text = readFileSync(path.join(ROOT, rel), "utf8");
    const script = (text.match(/<script[^>]*>([\s\S]*?)<\/script>/) ?? [])[1] ?? "";
    for (const m of script.matchAll(/\b(?:desc|label)\s*:\s*"([^"]+)"/g)) keys.add(m[1]);
    for (const list of script.matchAll(/features\s*:\s*\[([^\]]*)\]/g)) {
      for (const m of list[1].matchAll(/"([^"]+)"/g)) keys.add(m[1]);
    }
  }

  // Texts the Rust side translates (tray menu): crate::i18n::t(lang, "key").
  for (const file of walk(path.join(ROOT, "src-tauri", "src")).filter((f) => f.endsWith(".rs") && !f.endsWith("i18n.rs"))) {
    const text = readFileSync(file, "utf8");
    for (const m of text.matchAll(/\bi18n::t\(\s*[^,]+,\s*"([^"]+)"/g)) keys.add(m[1]);
  }

  // Shortcut labels: translated at render time, not with t() in the module.
  const shortcuts = readFileSync(path.join(ROOT, "src", "shortcuts.ts"), "utf8");
  for (const match of shortcuts.matchAll(/label:\s*"([^"]+)"/g)) keys.add(match[1]);

  return [...keys].sort();
}

function potHeader() {
  return `# Plantilla generada por scripts/i18n.mjs — no editar a mano.
msgid ""
msgstr ""
"Project-Id-Version: ianox 1.0.0\\n"
"MIME-Version: 1.0\\n"
"Content-Type: text/plain; charset=UTF-8\\n"
"Content-Transfer-Encoding: 8bit\\n"
"Plural-Forms: nplurals=2; plural=(n != 1);\\n"
`;
}

function writePot(keys) {
  const body = keys.map((key) => `msgid "${key}"\nmsgstr ""\n`).join("\n");
  writeFileSync(POT, `${potHeader()}\n${body}`);
  return keys;
}

function msgmerge(locale) {
  const po = path.join(LOCALES_DIR, `${locale}.po`);
  execFileSync("msgmerge", ["--update", "--backup=none", "--quiet", po, POT], { stdio: "inherit" });
  return po;
}

/** Minimal PO parser: msgid/msgstr with continuations and escapes. */
function parsePo(text) {
  const entries = {};
  let id = null;
  let str = null;
  let target = null;
  let fuzzy = false;

  const flush = () => {
    if (id !== null && id !== "" && !fuzzy) entries[id] = str ?? "";
    id = null;
    str = null;
    target = null;
    fuzzy = false;
  };

  for (const raw of text.split("\n")) {
    const line = raw.trim();

    if (line === "") {
      flush();
      continue;
    }
    if (line.startsWith("#")) {
      if (line.includes("fuzzy")) fuzzy = true;
      continue;
    }
    if (line.startsWith("msgid ")) {
      if (id !== null) flush();
      id = unquote(line.slice(6));
      target = "msgid";
      continue;
    }
    if (line.startsWith("msgstr ")) {
      str = unquote(line.slice(7));
      target = "msgstr";
      continue;
    }
    if (line.startsWith('"')) {
      if (target === "msgid") id += unquote(line);
      else if (target === "msgstr") str += unquote(line);
    }
  }
  flush();
  return entries;
}

function unquote(value) {
  const text = value.trim().replace(/;$/, "");
  try {
    return JSON.parse(text);
  } catch {
    return text.slice(1, -1);
  }
}

function compile(locale) {
  const po = path.join(LOCALES_DIR, `${locale}.po`);
  const entries = parsePo(readFileSync(po, "utf8"));
  const json = JSON.stringify(entries, null, 2) + "\n";

  writeFileSync(path.join(ROOT, "src", "locales", `${locale}.json`), json);
  writeFileSync(path.join(ROOT, "src-tauri", "locales", `${locale}.json`), json);

  return Object.keys(entries).length;
}

mkdirSync(path.join(ROOT, "src", "locales"), { recursive: true });
mkdirSync(path.join(ROOT, "src-tauri", "locales"), { recursive: true });
mkdirSync(LOCALES_DIR, { recursive: true });

const keys = extractKeys();
if (keys.length === 0) {
  console.error("No se encontraron claves t(\"...\") en src/");
  process.exit(1);
}

if (checkOnly) {
  for (const locale of LOCALES) {
    const entries = parsePo(readFileSync(path.join(LOCALES_DIR, `${locale}.po`), "utf8"));
    const missing = keys.filter((key) => !entries[key]);
    if (missing.length) {
      console.error(`${locale}: faltan ${missing.length} traducciones`);
      for (const key of missing) console.error(`  - ${key}`);
      process.exit(1);
    }
  }
  console.log(`i18n OK · ${keys.length} claves · ${LOCALES.join(", ")}`);
  process.exit(0);
}

writePot(keys);
for (const locale of LOCALES) {
  if (!existsSync(path.join(LOCALES_DIR, `${locale}.po`))) {
    console.error(`Falta locales/${locale}.po`);
    process.exit(1);
  }
  msgmerge(locale);
  const count = compile(locale);
  console.log(`${locale}: ${count} entradas → src/locales y src-tauri/locales`);
}
console.log(`Plantilla con ${keys.length} claves en locales/ianox.pot`);
