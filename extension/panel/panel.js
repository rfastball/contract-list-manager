// 문서 만들기 사이드 패널(ADR-045). 이 창의 활성 탭에 보이는 나라장터 건 하나로 서식을 채워 고른 폴더에 저장하거나(한글
// .hwpx), 채운 글을 복사한다(기안 .txt). 나르미 컴팩트 0.2.0 의 패널을 옮겼고, 값은 컴팩트의 캐시가 아니라 계약목록의 면 뷰에서
// 읽는다 — 그래서 사람이 고친 값과 저장된 연결(접수 → 공고 → 계약)이 문서에 그대로 든다.
//
// 차례는 저장 → 보기 → 만들기다(4항). 패널을 열거나 탭이 바뀌면 중계기(background.js documentCall)에 check 를 묻는다:
//   same     → read 로 면 뷰의 값을 받아 보이고, 서식을 고르고, 만든다. 만들기는 그 값만 쓰고 계약목록에 쓰지 않는다.
//   differs  → 무엇이 다른지(저장된 값 → 화면 값)와 그 아래 「계약목록에 저장」 단추 하나. 서식 · 채워질 값 · 만들기는 잠근다.
//   blocked  → 까닭과 계약목록의 「지금 보는 화면」 을 가리킨다.
// 무엇을 보일지는 state.js 가 정한다(Chrome 없이 시험한다). 여기는 그리고 누름을 받는다.

import { loadEngine } from "../engine.js";
import * as store from "../store.js";
import * as saver from "../saver.js";
import { NAME_PATTERNS, loadTemplates } from "../templates.js";
import {
  EMPTY_HINT, KIND_LABEL, NEW_CASE, OFFLINE, caseKeys, defaultNamePattern, focusKeys, moveTemplate, nameCheck, namePatternFor, orderTemplates,
  draftPieces, fieldRows, freshRead, keptSelections, meMismatch, readNotes, reportCounts, shownTemplates, splitValues, stage, usable,
} from "./state.js";

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const ic = (id, cls = "") => `<svg class="i ${cls}" aria-hidden="true"><use href="#i-${id}"/></svg>`;
/** 받침에 따라 조사: pp("공고문","으로") → "으로", pp("거래처","을","를") → "를" */
function pp(word, a, b) {
  const ch = String(word).charCodeAt(String(word).length - 1);
  const jong = ch >= 0xac00 && ch <= 0xd7a3 ? (ch - 0xac00) % 28 : 0;
  if (a === "으로") return jong === 0 || jong === 8 ? "로" : "으로";
  return jong ? a : b;
}
function when(ms) {
  if (!ms) return "";
  const d = new Date(ms), now = new Date();
  const hm = `${d.getHours()}:${String(d.getMinutes()).padStart(2, "0")}`;
  return d.toDateString() === now.toDateString() ? `오늘 ${hm}` : `${d.getMonth() + 1}월 ${d.getDate()}일`;
}
function today() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
// 「만드는 중」을 먼저 그리고 엔진을 부른다(엔진이 도는 동안은 화면이 멈춘다). 숨은 탭에서는 rAF 가 돌지 않아 시간으로도 끝낸다.
const frame = () => new Promise((r) => { requestAnimationFrame(() => setTimeout(r, 0)); setTimeout(r, 100); });

const ui = {
  view: "main", // main | me(설정)
  windowId: null,
  st: { stage: "loading", lines: [] }, // state.js stage()
  read: null, // 호스트 document {op:"read"} 의 답 — 만들기는 이것만 쓴다
  readDay: "", // read 를 받은 날(today()) — 만들 때 오늘이 아니면 다시 읽는다(state.js freshRead)
  notes: null, // state.js readNotes(read)
  saving: false,
  saveError: null,
  pending: false, // 저장하는 사이에 탭이 바뀌었다는 알림이 왔다
  seen: null, // 그린 check 답의 {tabId, read} — 저장에 되실어 그 화면만 저장하게 한다(background.js documentCall)
  saved: {}, // 저장한 파일 기록
  templates: [], // 서식 폴더의 파일(templates.js loadTemplates) — 열 때 · 다시 확인 때 · 설정에서 돌아올 때 다시 읽는다
  templatesFolder: "", // 서식 폴더의 경로(호스트가 알린다)
  templatesError: null, // 목록부터 받지 못했다(연결 없음 등)
  templateId: null, // 고른 서식의 파일 이름
  order: [], // 설정 「서식」 의 순서 — 파일 이름들. 새 파일은 맨 아래(state.js orderTemplates)
  hidden: [], // 숨긴 서식의 파일 이름들 — 만들기 화면의 목록에 나오지 않는다
  selections: {}, // 서식마다 고른 항목 {파일 이름: {항목 id: 선택 id}}
  me: {},
  folder: null,
  namePatterns: {}, // 사람이 서식마다 정한 파일 이름 패턴 {파일 이름: 패턴}. 없으면 동봉 기본 → 엔진 기본(state.js namePatternFor)
  nameEdit: null, // 아랫단 파일 이름 줄을 고치는 중 — {id: 파일 이름, draft: 적던 글}
  valuesOpen: null,
  foot: { state: "idle" },
  lastCase: null,
};
let engine;
let engineHash = ""; // engine.wasm 의 SHA-256 — 엔진이 바뀌면 서식 컴파일본(store.js prepared)을 쓰지 않는다
let vocabMe = []; // 어휘의 내 정보 칸(이름 · 다른 이름)
let MISSING = "【확인 필요】"; // 엔진 어휘의 missing 으로 바꾼다

/* ── 자료 ───────────────────────────────────────── */

async function loadSettings() {
  [ui.me, ui.folder, ui.namePatterns, ui.order, ui.hidden, ui.selections] = await Promise.all([
    store.setting("me"), saver.folder(), store.setting("fileNamePatterns"),
    store.setting("templateOrder"), store.setting("hiddenTemplates"), store.setting("selections"),
  ]);
  ui.me ||= {};
  ui.namePatterns ||= {};
  ui.order ||= [];
  ui.hidden ||= [];
  ui.selections ||= {};
  await loadSaved();
}

let loadingSaved = 0;
let savedCheckTimer = null;
async function loadSaved(retryDownloads = true) {
  const token = ++loadingSaved;
  const saved = await saver.savedFiles(await store.all("saved"));
  if (token !== loadingSaved) return;
  ui.saved = saved;
  clearTimeout(savedCheckTimer);
  // Chrome 은 디스크 확인을 10초에 한 번만 한다. 방금 저장한 파일을 지운 경우도 한 번 더 확인한다.
  if (retryDownloads && Object.values(saved).some((rec) => !rec.fileHandle)) {
    savedCheckTimer = setTimeout(() => { loadSaved(false).then(render).catch(() => {}); }, 10_100);
  }
}

let loadingTemplates = 0;
/** 서식 폴더를 다시 읽는다(한글 서식은 바뀐 것만 컴파일한다 — store.js prepared). 늦게 온 옛 결과는 버린다. */
async function loadTemplateFiles() {
  const token = ++loadingTemplates;
  const prepared = {
    engine: engineHash,
    all: () => store.all("prepared"),
    put: (name, record) => store.put("prepared", name, record),
    remove: (name) => store.remove("prepared", name),
  };
  const loaded = await loadTemplates(engine, (params) => ask({ op: "templates", ...params }), prepared);
  if (token !== loadingTemplates) return;
  ui.templatesError = loaded.error || null;
  if (loaded.folder) ui.templatesFolder = loaded.folder;
  ui.templates = (loaded.templates || []).map((t) => ({ ...t, selections: keptSelections(t.slots, ui.selections[t.id]) }));
  if (!template()) {
    const last = await store.setting("lastTemplate");
    ui.templateId = shownList().some((t) => t.id === last && usable(t)) ? last : null;
  }
}

async function loadAll() {
  await loadSettings();
  await loadTemplateFiles();
}

const context = () => ({ me: ui.me });
/** 설정의 순서대로 놓은 서식 · 그 가운데 숨기지 않은 것. */
const ordered = () => orderTemplates(ui.templates, ui.order);
const shownList = () => shownTemplates(ordered(), ui.hidden);
/** 고른 서식 — 숨겼거나 오류가 난 서식은 고른 것으로 치지 않는다. */
const template = () => shownList().find((t) => t.id === ui.templateId && usable(t)) || null;
/** 저장할 파일 이름 → {name} | {error: 한 줄} | {}(건이 없다). */
const fileName = (read, t, pattern = namePatternFor(t?.id, ui.namePatterns, NAME_PATTERNS)) => nameCheck(engine.fileName(read || {}, t?.name || "", pattern, today()), !!read);
/** 만들 수 있는 건 — 저장된 것과 같고, 읽은 값이 있다. 아니면 null(서식 · 값 · 만들기를 잠근다). */
const current = () => (ui.st.stage === "same" && ui.read && ui.notes && !ui.notes.older ? ui.read : null);
/** 서식의 칸마다 채워질 값. */
function valuesFor(read, t) {
  const { values } = engine.resolve(read, context());
  return fieldRows(t.fields, values, (name) => engine.makeText(`{{${name}}}`, read, context(), {}).fields[0]);
}

/** 빈 칸의 까닭 한 줄(내 정보 칸은 단추만). 【확인 필요】로 남는다는 것은 주 단추 위의 한 줄(missingLine)이 말한다. */
function missingHint(row) {
  if (!row.canonical) return { text: "없는 칸 이름입니다" };
  if (vocabMe.some((m) => m.name === row.canonical)) return { text: "", act: "me", label: "내 정보 적기" };
  return { text: "계약목록에 이 값이 비어 있습니다" };
}

/** 이 건으로 만든 문서 가운데, 지금 값과 달라진 것. */
function staleDocs(read) {
  const keys = caseKeys(read);
  const out = [];
  for (const rec of Object.values(ui.saved)) {
    if (!rec.caseKeys?.some((k) => keys.includes(k)) || !rec.values) continue;
    const t = ui.templates.find((x) => x.id === rec.templateId);
    if (!usable(t)) continue;
    const now = Object.fromEntries(valuesFor(read, t).map((r) => [r.field, r.value]));
    const changed = Object.entries(rec.values).filter(([f, v]) => f in now && now[f] !== v).map(([f, v]) => ({ field: f, was: v, now: now[f] }));
    if (changed.length) out.push({ rec, template: t, changed });
  }
  // 서식마다 가장 최근 파일 하나. 지금 고른 서식의 것을 먼저, 그다음 최근에 저장한 것.
  out.sort((a, b) => b.rec.savedAt - a.rec.savedAt);
  const seen = new Set();
  return out.filter((x) => !seen.has(x.template.id) && seen.add(x.template.id))
    .sort((a, b) => (b.template.id === ui.templateId) - (a.template.id === ui.templateId) || b.rec.savedAt - a.rec.savedAt);
}

/* ── 계약목록에 묻기 ─────────────────────────────── */

function ask(params) {
  return chrome.runtime.sendMessage({ type: "pclm-document", windowId: ui.windowId, ...params })
    .then((reply) => reply || { kind: "offline" })
    .catch(() => ({ kind: "offline" })); // 중계기가 답하지 않는다 — 연결 안 됨으로 그린다
}

let checking = 0;
/** 지금 화면이 저장된 것과 같은가 → 같으면 면 뷰를 읽는다. 늦게 온 옛 답은 버린다. */
async function check() {
  if (ui.saving) { ui.pending = true; return; }
  const token = ++checking;
  ui.saveError = null;
  if (!["same", "differs", "blocked"].includes(ui.st.stage)) { ui.st = { stage: "reading", lines: [] }; render(); }
  const reply = await ask({ op: "check" });
  if (token !== checking) return;
  ui.seen = reply.kind === "answer" ? { tabId: reply.tabId, read: reply.read } : null;
  await settle(stage(reply), token);
}

/** 단계를 받아들이고, 같으면 읽는다. */
async function settle(next, token) {
  if (next.stage !== "same") {
    ui.st = next; ui.read = null; ui.notes = null;
    render();
    return;
  }
  // 같은 건을 이미 읽어 보이고 있어도 다시 읽는다 — 그사이 계약목록에서 값을 고쳤을 수 있다.
  const reply = await ask({ op: "read", entity: next.entity, entityType: next.entityType });
  if (token !== checking) return;
  if (reply.kind !== "answer") {
    ui.st = stage(reply); ui.read = null; ui.notes = null;
  } else {
    ui.st = next; ui.read = reply.answer; ui.notes = readNotes(reply.answer); ui.readDay = today();
  }
  render();
}

/** 만들 건 — 읽은 날이 지났으면 다시 읽은 것. 다시 읽어 만들 수 없게 되면 null 이고 화면은 check 가 그 단계로 그렸다. */
const fresh = (read) => freshRead(read, ui.readDay, today(), async () => { await check(); return current(); });

/** 「계약목록에 저장」 — 저장한 뒤의 check 꼴이 돌아온다(호스트가 다시 본 것). 같으면 읽는다. */
async function saveToList() {
  if (ui.saving || ui.st.stage !== "differs") return;
  ui.saving = true; ui.saveError = null; ui.pending = false;
  render();
  const token = ++checking;
  const reply = await ask({ op: "save", ...ui.seen });
  ui.saving = false;
  // 보이던 것과 다른 화면이라 저장하지 않았다 — 지금 화면을 다시 확인한다.
  if (reply.kind === "reading") ui.pending = true;
  const next = stage(reply);
  if (reply.kind === "answer" && reply.answer?.saved) toast("계약목록에 저장했습니다");
  if (next.stage === "error" && token === checking) ui.saveError = next.lines[0];
  if (token === checking) await settle(next.stage === "error" ? ui.st : next, token);
  if (ui.pending) { ui.pending = false; await check(); }
}

let scheduled = null;
function recheck() {
  clearTimeout(scheduled);
  scheduled = setTimeout(() => { scheduled = null; check().catch(() => {}); }, 150);
}

/* ── 그리기 ─────────────────────────────────────── */

function statusHtml() {
  const s = ui.st;
  const say = {
    loading: ["busy", "문서 만들기를 여는 중"],
    reading: ["busy", "화면을 읽는 중입니다."],
    empty: null, // 비운 패널 — 본문 말고는 아무것도 말하지 않는다
    offline: ["bad", OFFLINE],
    error: ["bad", "화면을 확인하지 못했습니다"],
    same: ui.notes?.older ? ["off", "옛 차수 화면입니다"] : ["ok", "계약 목록에 연결됨"],
    differs: ["warn", "계약목록과 다릅니다"],
    blocked: ["bad", "계약목록에 저장할 수 없는 화면입니다"],
  }[s.stage];
  const again = ["empty", "loading"].includes(s.stage) ? ""
    : `<button class="ib" data-act="recheck" title="이 화면 다시 확인" aria-label="이 화면 다시 확인">${ic("refresh", "sm")}</button>`;
  const said = say ? `<span class="say ${say[0]}" title="${esc(say[1])}"><span>${esc(say[1])}</span></span>` : "<span></span>";
  return `<div class="status">${said}<span class="tools">${again}<button class="ib" data-act="me" title="설정" aria-label="설정">${ic("gear", "sm")}</button></span></div>`;
}

/** 건 카드 — 읽기 전이면 화면에서 읽은 번호만, 읽은 뒤에는 건명과 탭의 번호. */
function caseHtml() {
  const read = ui.read;
  if (read?.values) {
    return `<div class="sec"><div class="case follow"><div class="t">${esc(read.values.건명 || "이름 없는 건")}</div><div class="no num">${esc(KIND_LABEL[read.entityType] || "")} ${esc(read.entity || "")}</div></div></div>`;
  }
  if (!ui.st.entity) return "";
  const fresh = ui.st.stage === "differs" && ui.st.isNew;
  return `<div class="sec"><div class="case follow">${fresh ? `<div class="t" id="new-case">${esc(NEW_CASE)}</div>` : ""}<div class="no num">화면의 건 · ${esc(KIND_LABEL[ui.st.entityType] || "")} ${esc(ui.st.entity)}</div></div></div>`;
}

function linesHtml(lines, kind = "chgd") {
  if (!lines?.length) return "";
  return `<div class="sec tight">${lines.map((l) => `<div class="box ${kind}"><div class="h">${ic("alert")}<span>${esc(l)}</span></div></div>`).join("")}</div>`;
}

// 서식 종류 아이콘(16px) — 이름 앞에 둔다. 한글은 파란 네모의 「한」, 글은 위가 하늘색인 메모장.
const KIND_ICON = {
  hwpx: `<span class="ki" title="한글 서식 · 파일로 저장"><svg role="img" aria-label="한글 서식" viewBox="0 0 16 16"><rect x="1" y="1" width="14" height="14" rx="3" fill="#1f7ae0"/><text x="8" y="11.7" text-anchor="middle" font-size="10" font-weight="700" fill="#ffffff">한</text></svg></span>`,
  txt: `<span class="ki" title="글 서식 · 복사해서 붙여 넣기"><svg role="img" aria-label="글 서식" viewBox="0 0 16 16"><rect x="2.5" y="2.5" width="11" height="12" rx="1.5" fill="#ffffff" stroke="#5c626b"/><path d="M3 6.5V4a1 1 0 0 1 1-1h8a1 1 0 0 1 1 1v2.5z" fill="#5aa9e6"/><path d="M5 9.5h6M5 12h4M5.5 1v2.5M8 1v2.5M10.5 1v2.5" stroke="#5c626b" stroke-linecap="round"/></svg></span>`,
};

/** 서식 목록 — 숨긴 것은 빼고, 오류 난 서식은 그 줄에 까닭 한 줄을 달고 고를 수 없다. 다른 서식은 그대로 쓴다. */
function templatesHtml() {
  const buttons = shownList().map((t) => (usable(t)
    ? `<button class="f" data-act="template" data-id="${esc(t.id)}" aria-pressed="${t.id === ui.templateId}">${KIND_ICON[t.kind]}<span class="nm">${esc(t.name)}</span></button>`
    : `<button class="f bad" data-id="${esc(t.id)}" disabled title="${esc(t.error)}">${KIND_ICON[t.kind]}<span class="tx"><span class="nm">${esc(t.name)}</span><span class="err">${esc(t.error)}</span></span></button>`)).join("");
  const failed = ui.templatesError ? `<p class="hint" id="forms-error">${esc(ui.templatesError)}</p>` : "";
  return `<div class="sec tight"><div class="lrow"><span class="lbl" id="forms-label">서식</span></div>${failed}<div class="forms" role="group" aria-labelledby="forms-label">${buttons}</div></div>`;
}

const choosable = (t) => (t?.slots || []).filter((s) => s.options.length);
const needPick = (t) => choosable(t).find((s) => !s.options.some((o) => o.id === t.selections?.[s.id])) || null;

function picksHtml(t) {
  const slots = choosable(t);
  if (!slots.length) return "";
  const body = slots.map((s) => {
    const chosen = t.selections?.[s.id];
    const need = !s.options.some((o) => o.id === chosen);
    const buttons = s.options.map((o) => `<button class="o" data-act="option" data-slot="${esc(s.id)}" data-option="${esc(o.id)}" aria-pressed="${o.id === chosen}">${esc(o.label || "(비움)")}</button>`).join("");
    return `<div class="pick ${need ? "need" : ""}" role="group" aria-label="${esc(s.label)}"><div class="ph"><b>${esc(s.label)}</b>${need ? "<small>하나 고르기</small>" : ""}</div><div class="seg">${buttons}</div></div>`;
  }).join("");
  return `<div class="sec"><div class="lrow"><span class="lbl">포함할 내용</span></div>${body}</div>`;
}

/** 채워질 값 — 빈 칸(【확인 필요】로 남을 칸)을 위에 늘 펼치고, 채워진 칸은 접어 둔다. */
function valuesHtml(read, t, rows) {
  const { missing, filled } = splitValues(rows.filter((r) => r.kind !== "josa"));
  const josa = rows.filter((r) => r.kind === "josa");
  const stale = staleDocs(read);
  const open = ui.valuesOpen === true;
  const gaps = missing.length ? `<div class="gaps" id="gaps"><p>${ic("alert", "sm")}<span>빈 칸 ${missing.length}개</span></p><ul>${missing.map((r) => {
    const hint = missingHint(r);
    return `<li><b>${esc(r.field)}</b>${hint.text ? `<span>${esc(hint.text)}</span>` : ""}${hint.act ? `<button class="fix" data-act="${hint.act}">${esc(hint.label)}</button>` : ""}</li>`;
  }).join("")}</ul></div>` : "";
  const more = stale.length > 2 ? `<p class="hint" style="margin:var(--sp-8) 0 0">옛 값으로 만든 문서가 ${stale.length - 2}개 더 있습니다.</p>` : "";
  const staleBox = stale.slice(0, 2).map((s) => {
    const list = s.changed.slice(0, 3).map((x) => `${esc(x.field)} <s>${esc(x.was ?? "빈 값")}</s> → ${esc(x.now ?? "빈 값")}`).join("<br>");
    const action = s.template.id !== ui.templateId
      ? `<div class="row"><button class="btn warn sm" data-act="template" data-id="${esc(s.template.id)}">「${esc(s.template.name)}」${pp(s.template.name, "으로")} 다시 만들기</button></div>`
      : "";
    return `<div class="box chgd" style="margin-top:var(--sp-12)"><div class="h">${ic("refresh")}<span>옛 값으로 만든 문서가 있습니다</span></div><p>${esc(s.rec.name)}</p><p>${list}</p>${action}</div>`;
  }).join("") + more;
  const foldLabel = missing.length ? `채워진 ${filled.length}칸 보기` : `${ic("check", "xs ok")}${filled.length}칸 보기`;
  const fold = filled.length ? `<button class="fold" data-act="fold" aria-expanded="${open}" aria-controls="filled">${ic("chev-r", "xs chev")}<span>${foldLabel}</span></button>` : "";
  const list = open && filled.length ? `<dl class="v" id="filled">${filled.map((r) => `<dt>${esc(r.field)}</dt><dd>${esc(r.value)}</dd>`).join("")}</dl>` : "";
  const head = `<div class="lrow"><span class="lbl">채워질 값</span><span class="count num">${missing.length + filled.length}칸 중 ${filled.length}칸 채움</span></div>`;
  const josaHtml = t.kind !== "txt" && josa.length ? `<div class="lrow"><span class="lbl">조사 선택</span></div><dl class="v">${josa.map((r) => `<dt>${esc(r.target)} · ${esc(r.pair)}</dt><dd>${esc(r.value || "선택 불가")}${r.error ? `<br><span class="hint">${esc(r.error)}</span>` : ""}</dd>`).join("")}</dl>${josaReviewHtml(josa)}` : "";
  return { html: `<div class="sec tight">${head}${mismatchHtml(read, t)}${gaps}${staleBox}${fold}${list}${josaHtml}</div>`, missing: missing.length, error: josa.find((r) => r.error)?.error };
}

function josaReviewHtml(fields) {
  const rows = (fields || []).filter((f) => f.kind === "josa" && f.status === "needs_review");
  if (!rows.length) return "";
  return `<div class="box ask"><div class="h">${ic("alert")}<span>조사 ${rows.length}개 확인 필요</span></div>${rows.map((r) => `<p>「${esc(r.target)}」: ${r.reason === "missing_value" ? "대상 값이 없어" : "끝소리를 판단할 수 없어"} ${esc(r.value)}로 남습니다.</p>`).join("")}</div>`;
}

const completionText = ({ missing = 0, review = 0 }) => [missing ? `빈 칸 ${missing}개 ${MISSING}` : "", review ? `조사 ${review}개 확인 필요` : ""].filter(Boolean).join(" · ") || "모두 채움";

/** 서식이 내 정보 칸을 쓰는데 내 정보가 공고 값과 다르면 그 항목들 — 보이기만 하고 막지 않는다(ADR-016). */
function mismatchHtml(read, t) {
  const diff = meMismatch(t.fields, vocabMe.map((m) => m.name), ui.me, read.values);
  if (!diff.length) return "";
  const rows = diff.map((d) => `<span class="k">${esc(d.label)}</span><span><span class="w">내 정보</span> <b>${esc(d.mine)}</b></span><span><span class="w">공고</span> <b>${esc(d.notice)}</b></span>`).join("");
  return `<div class="box ask" id="me-mismatch"><div class="h">${ic("alert")}<span>내 정보가 공고와 다릅니다</span></div><div class="mm">${rows}</div><p>문서에는 내 정보가 들어갑니다.</p><div class="row"><button class="chg" data-act="me">내 정보 고치기</button></div></div>`;
}

/** 아랫단 파일 이름 줄의 이름 — 고른 서식의 패턴으로 지은 것, 없는 칸이면 그 한 줄. */
function nameLine() {
  const named = fileName(current(), template());
  return named.name ?? named.error;
}

/**
 * 파일 이름 줄과 폴더 줄. 파일 이름 줄의 연필이 그 자리를 고른 서식의 패턴 입력 칸으로 바꾼다(ui.nameEdit).
 * 폴더는 「설정」 에서 바꾼다. editable 이 거짓이면(만드는 중) 연필을 두지 않는다.
 */
function destHtml(name, editable = true) {
  const dir = `저장 폴더 · ${esc(ui.folder ? ui.folder.name : saver.DEFAULT_LABEL)}`;
  const folder = `${ic("folder")}<span class="dir" title="${dir}">${dir}</span>`;
  const t = template();
  if (editable && ui.nameEdit && t) {
    const value = ui.nameEdit.draft ?? ui.namePatterns[t.id] ?? "";
    const fallback = defaultNamePattern(t.id, NAME_PATTERNS);
    const named = fileName(current(), t, value.trim() || fallback);
    return `<div class="dest editing">${ic("file")}<div class="edit"><label class="sr" for="name-pattern">파일 이름</label><input id="name-pattern" value="${esc(value)}" placeholder="${esc(fallback)}" aria-invalid="${!!named.error}" aria-describedby="name-note" spellcheck="false" autocomplete="off">${nameNoteHtml(named)}</div>${folder}</div>`;
  }
  const pen = editable ? `<button class="ib" data-act="name-edit" id="name-edit" title="파일 이름 고치기" aria-label="파일 이름 고치기">${ic("pen", "sm")}</button>` : "<span></span>";
  return `<div class="dest">${ic("file")}<span class="fn" id="file-name" title="${esc(name)}">${esc(name)}</span>${pen}${folder}</div>`;
}

/**
 * 파일 이름 줄을 닫는다. 아랫단만 갈아 끼운다 — 칸을 떠나는(change) 순간은 대개 다른 단추를 누르는 중이라, 통째로 다시 그려
 * 그 단추를 갈아 끼우면 누름이 사라진다.
 */
function closeNameEdit() {
  ui.nameEdit = null;
  const dest = document.querySelector(".dest.editing");
  if (!dest) return;
  dest.outerHTML = destHtml(nameLine());
  keyed($("app"));
}

/** 적은 패턴이 맞으면 고른 서식의 것으로 저장하고 줄로 돌아간다(빈 값은 사람 패턴을 지워 기본으로). 틀리면 아무것도 하지 않는다. */
function commitName(input) {
  const t = template();
  if (!ui.nameEdit || !t) return null;
  const value = input.value.trim();
  if (fileName(current(), t, value || defaultNamePattern(t.id, NAME_PATTERNS)).error) return null;
  const patterns = { ...ui.namePatterns };
  if (value) patterns[t.id] = value;
  else delete patterns[t.id];
  ui.namePatterns = patterns;
  closeNameEdit();
  return store.setSetting("fileNamePatterns", patterns);
}

/** 글 서식: 만든 글. 【확인 필요】는 눈에 띄게, 값이 들어간 자리는 옅게 — 화면에만 보이고 복사되는 글(report.text)은 그대로다. */
function draftHtml(d) {
  if (d.error) return `<div class="sec"><div class="box bad" id="draft-error"><div class="h">${ic("alert")}<span>글을 만들지 못했습니다</span></div><p>${esc(d.error)}</p></div></div>`;
  const body = draftPieces(d.report.segments, MISSING)
    .map(({ text, mark }) => (mark ? `<mark${mark === "fill" ? ' class="fill"' : ""}>${esc(text)}</mark>` : esc(text)))
    .join("");
  return `<div class="sec tight"><div class="lrow"><span class="lbl" id="draft-label">복사할 글</span></div><pre class="draft" id="draft" tabindex="0" aria-labelledby="draft-label">${body}</pre>${josaReviewHtml(d.report.fields)}</div>`;
}

/** 주 단추 위의 한 줄 — 빈 칸을 【확인 필요】로 남기는 것은 정상 동작이라(ADR-016) 주의색이 아니다. */
const missingLine = (missing) => (missing ? `<p class="pre">${ic("alert", "sm")}<span>빈 칸 ${missing}개는 ${esc(MISSING)}로 남습니다</span></p>` : "");

function copyFootHtml(t, d) {
  const f = ui.foot;
  const need = needPick(t);
  if (need) return `<button class="btn" id="copy" disabled>「${esc(need.label)}」${pp(need.label, "을", "를")} 고르세요</button>`;
  if (!d || d.error) return `<button class="btn" id="copy" disabled>복사할 글이 없습니다</button>`;
  const counts = reportCounts(d.report.fields);
  if (f.state === "copied" && f.text === d.report.text) {
    return `<div class="box ok" id="copied"><div class="h">${ic("check")}<span>복사했습니다 · ${esc(completionText(counts))}</span></div></div><div class="row"><button class="btn ghost" id="copy" data-act="copy">${ic("copy", "sm")}다시 복사</button></div>`;
  }
  const error = f.state === "error" ? `<div class="box bad" id="error" style="margin-bottom:var(--sp-12)"><div class="h">${ic("alert")}<span>복사하지 못했습니다</span></div><p>${esc(f.message)}</p></div>` : "";
  return error + missingLine(counts.missing) + `<button class="btn" id="copy" data-act="copy">${ic("copy")}글 복사</button>`;
}

function footHtml(read, t, missing, draft, fieldError) {
  const f = ui.foot;
  if (!t) return `<button class="btn" id="save" disabled>서식을 고르세요</button>`;
  if (t.kind === "txt") return copyFootHtml(t, draft);
  if (fieldError) return `<p class="pre">${esc(fieldError)}</p><button class="btn" id="save" disabled>조사 대상 필드를 확인하세요</button>`;
  const name = nameLine();
  if (f.state === "making") return destHtml(f.name || name, false) + `<button class="btn busy" id="save" disabled>만드는 중…</button>`;
  if (f.state === "confirm") {
    const ex = f.existing;
    const saidAt = ex.record ? when(ex.record.savedAt) : when(ex.lastModified);
    if (ex.kind === "ours") {
      return `<div class="box ask" id="confirm" data-kind="ours"><div class="h">${ic("alert")}<span>같은 이름 파일이 있습니다</span></div><p>${esc(f.name)} · ${esc(saidAt)} 문서 만들기가 저장한 그대로입니다. 덮어쓰면 새 값으로 바뀝니다.</p><div class="row"><button class="btn warn sm" data-act="overwrite">덮어쓰기</button><button class="btn ghost" data-act="cancel">취소</button></div></div>`;
    }
    const why = ex.kind === "edited" ? `문서 만들기가 ${esc(saidAt)}에 저장한 뒤 누군가 고쳤습니다. 덮어쓰면 고친 내용이 사라집니다.` : "문서 만들기가 만들지 않은 파일입니다. 덮어쓰면 이 파일의 내용이 사라집니다.";
    return `<div class="box ask" id="confirm" data-kind="${ex.kind}"><div class="h">${ic("alert")}<span>${ex.kind === "edited" ? "손으로 고친 파일이 있습니다" : "같은 이름 파일이 있습니다"}</span></div><p>${esc(f.name)}</p><p>${why}</p><div class="row"><button class="btn sm" data-act="save-as">새 이름으로 저장</button><button class="btn warn sm" data-act="overwrite">덮어쓰기</button></div><div class="row"><button class="btn ghost" data-act="cancel">취소</button></div></div>`;
  }
  if (f.state === "saved") {
    return `<div class="box ok" id="saved"><div class="h">${ic("check")}<span>저장했습니다 · ${esc(completionText(f))}</span></div><p id="saved-name">${esc(f.name)}</p>`
      + (f.downloadId != null
        ? `<p>저장 폴더 · ${esc(saver.DEFAULT_LABEL)}</p><div class="row"><button class="btn ghost" data-act="show-download">${ic("folder", "sm")}폴더 열기</button></div></div>`
        : `<p>저장 폴더 · ${esc(ui.folder?.name || "")}</p></div>`);
  }
  const error = f.state === "error" ? `<div class="box bad" id="error" style="margin-bottom:var(--sp-12)"><div class="h">${ic("alert")}<span>저장하지 못했습니다</span></div><p>${esc(f.message)}</p></div>` : "";
  const need = needPick(t);
  if (need) return error + destHtml(name) + `<button class="btn" id="save" disabled>「${esc(need.label)}」${pp(need.label, "을", "를")} 고르세요</button>`;
  return error + destHtml(name) + missingLine(missing) + `<button class="btn" id="save" data-act="save">한글 파일로 저장</button>`;
}

const shown = (v) => (v === "" || v == null ? `<span class="none">빈 값</span>` : esc(v));

/** 무엇이 다른지 — 저장된 값 → 화면 값. 값은 견주지 않은 글 그대로다(ADR-016). 호스트가 앞의 몇 개만 싣는다. */
function changesHtml(s) {
  if (!s.changes?.length) return "";
  const rest = s.changedCount - s.changes.length;
  const rows = s.changes.map((c) => `<li><span class="k">${esc(c.label)}</span><span class="ch"><span class="was">${shown(c.before)}</span>${ic("arrow", "sm")}<span class="now">${shown(c.after)}</span></span></li>`).join("");
  return `<div class="lrow"><span class="lbl" id="changes-label">저장된 값 ${ic("arrow", "xs")} 화면 값</span></div><ul class="diff" id="changes" aria-labelledby="changes-label">${rows}</ul>${rest > 0 ? `<p class="more">외 ${rest}칸이 더 다릅니다.</p>` : ""}`;
}

/** 저장된 것과 달라 잠긴 화면 — 무엇이 다른지와 그 바로 아래 「계약목록에 저장」 단추 하나. */
function differsHtml() {
  const error = ui.saveError ? `<div class="box bad" id="list-error" style="margin-bottom:var(--sp-12)"><div class="h">${ic("alert")}<span>저장하지 못했습니다</span></div><p>${esc(ui.saveError)}</p></div>` : "";
  const list = ui.st.isNew ? "" : changesHtml(ui.st);
  return `<div class="sec">${list}${error}${ui.st.lines[0] ? `<p class="hint" id="differs">${esc(ui.st.lines[0])}</p>` : ""}`
    + (ui.saving ? `<button class="btn busy" id="save-list" disabled>저장하는 중…</button>` : `<button class="btn" id="save-list" data-act="save-list">계약목록에 저장</button>`)
    + `</div>`;
}

function mainHtml() {
  const s = ui.st;
  if (s.stage === "loading" || s.stage === "reading") return statusHtml();
  if (s.stage === "empty") return statusHtml() + `<div class="scroll"><div class="sec"><div class="empty" id="empty"><b>${esc(s.lines[0])}</b><p>${esc(EMPTY_HINT)}</p></div></div></div>`;
  if (s.stage === "offline") return statusHtml() + `<div class="scroll">${linesHtml(s.lines.filter((l) => l !== OFFLINE), "ask")}</div>`;
  if (s.stage === "error") return statusHtml() + `<div class="scroll">${linesHtml(s.lines, "bad")}</div>`;
  if (s.stage === "differs") return statusHtml() + `<div class="scroll" id="scroll">${caseHtml()}${differsHtml()}</div>`;
  if (s.stage === "blocked") {
    const list = changesHtml(s);
    return statusHtml() + `<div class="scroll" id="scroll">${caseHtml()}<div class="sec tight"><div class="box bad" id="blocked"><div class="h">${ic("alert")}<span>${esc(s.lines[0])}</span></div><p>${esc(s.lines[1])}</p></div></div>${list ? `<div class="sec">${list}</div>` : ""}</div>`;
  }
  // same — 읽은 값으로 만든다.
  if (!ui.read) return statusHtml();
  if (ui.notes.older) return statusHtml() + `<div class="scroll">${linesHtml(ui.notes.lines)}</div>`;
  const read = current();
  const t = template();
  // 건이나 서식이 바뀌면 저장 자리를 처음으로.
  const sig = `${caseKeys(read).join(",")}|${t?.id || ""}`;
  if (ui.lastCase !== sig) {
    if (ui.foot.state !== "making") ui.foot = { state: "idle" };
    ui.valuesOpen = null;
    ui.lastCase = sig;
  }
  // 서식을 바꿔 고르면 고치던 파일 이름은 버린다 — 이름 줄은 새 서식의 패턴으로 다시 지어진다.
  if (ui.nameEdit && ui.nameEdit.id !== t?.id) ui.nameEdit = null;
  let values = { html: "", missing: 0 };
  if (t) values = valuesHtml(read, t, valuesFor(read, t));
  // 글 서식은 글이 짧아 그릴 때마다 만든다(수 ms). 고를 항목이 남았으면 만들지 않는다.
  let draft = null;
  if (t && t.kind === "txt" && !needPick(t)) {
    try {
      draft = { report: engine.makeText(t.text, read, context(), t.selections || {}) };
    } catch (error) {
      draft = { error: error.message };
    }
  }
  return statusHtml() + `<div class="scroll" id="scroll">${caseHtml()}${linesHtml(ui.notes.lines)}${templatesHtml()}${t ? picksHtml(t) : ""}${draft ? draftHtml(draft) : ""}${values.html}</div><div class="foot">${footHtml(read, t, values.missing, draft, values.error)}</div>`;
}

/** 파일 이름 입력 칸 아래 한 줄 — 없는 칸이면 그 한 줄만, 아니면 지금 건과 서식으로 지은 이름(건이 없으면 비운다). */
const nameNoteHtml = (named) => `<p class="name-note${named.error ? " bad" : ""}" id="name-note" title="${esc(named.error || named.name || "")}">${named.error ? ic("alert", "sm") : ""}<span>${esc(named.error || named.name || "")}</span></p>`;

// 순서 손잡이(6×10 점 여섯).
const GRIP = `<svg aria-hidden="true" width="6" height="10" viewBox="0 0 6 10" fill="currentColor"><circle cx="1" cy="1" r="1"/><circle cx="5" cy="1" r="1"/><circle cx="1" cy="5" r="1"/><circle cx="5" cy="5" r="1"/><circle cx="1" cy="9" r="1"/><circle cx="5" cy="9" r="1"/></svg>`;

/**
 * 설정의 「서식」 — 서식 폴더의 파일을 설정의 순서대로. 줄마다 순서 손잡이(끌기 · ↑ ↓) · 종류 · 이름 · 눈(숨기기 /
 * 다시 보이기). 고치기는 한글 · 메모장에서 한다 — 「폴더 열기」 가 그 폴더를 탐색기로 연다.
 */
function formsBandHtml() {
  const rows = ordered().map((t) => {
    const off = ui.hidden.includes(t.id);
    const say = off ? "다시 보이기" : "숨기기";
    return `<li class="trow${off ? " off" : ""}" data-id="${esc(t.id)}"><button type="button" class="grip" data-act="grip" data-id="${esc(t.id)}" draggable="true" title="끌어서 순서 바꾸기" aria-label="${esc(t.name)} 순서 바꾸기 (위 · 아래 화살표)">${GRIP}</button>${KIND_ICON[t.kind]}<span class="nm" title="${esc(t.id)}">${esc(t.name)}</span><button type="button" class="ib" data-act="toggle-hidden" data-id="${esc(t.id)}" title="${say}" aria-label="${esc(t.name)} ${say}">${ic(off ? "eye-off" : "eye", "sm")}</button></li>`;
  }).join("");
  const where = ui.templatesFolder;
  return `<section class="band" aria-labelledby="forms-title"><div class="bhead"><h2 id="forms-title">서식</h2><button type="button" class="chg" data-act="open-templates" id="open-templates">${ic("folder", "sm")}폴더 열기</button></div>`
    + `${where ? `<p class="where" id="templates-folder" title="${esc(where)}">${esc(where)}</p>` : ""}${ui.templatesError ? `<p class="where bad">${esc(ui.templatesError)}</p>` : ""}`
    + `<ul class="tl" id="template-order" aria-labelledby="forms-title">${rows}</ul></section>`;
}

/** 설정 — 내 정보 · 저장 폴더 · 서식. 저장 단추가 없다: 칸마다 change 때 저장한다(아래 change 듣기). */
function meHtml() {
  const me = ui.meDraft || ui.me || {};
  return `<div class="status"><span class="say off"><span>설정</span></span><span class="tools"><button class="ib" data-act="back" title="닫기" aria-label="설정 닫기">${ic("x", "sm")}</button></span></div>
  <div class="scroll set" id="scroll"><form id="me-form">
    <section class="band" aria-labelledby="me-title"><h2 id="me-title">내 정보</h2><div class="pairs">
      <label for="me-dept">부서</label><input id="me-dept" name="dept" value="${esc(me.dept)}" autocomplete="organization">
      <label for="me-name">이름</label><input id="me-name" name="name" value="${esc(me.name)}" autocomplete="name">
      <label for="me-phone">연락처</label><input id="me-phone" name="phone" value="${esc(me.phone)}" autocomplete="tel" inputmode="tel">
      <label for="me-fax">팩스</label><input id="me-fax" name="fax" value="${esc(me.fax)}" inputmode="tel">
    </div></section>
    <section class="band" aria-labelledby="save-title"><h2 id="save-title">저장</h2><div class="pairs">
      <span class="k" id="folder-label">폴더</span>
      <div class="folder" role="group" aria-labelledby="folder-label"><span class="now" id="folder-now" title="${esc(ui.folder ? ui.folder.name : saver.DEFAULT_LABEL)}">${esc(ui.folder ? ui.folder.name : saver.DEFAULT_LABEL)}</span>${ui.folder ? `<button type="button" class="chg" data-act="default-folder">기본 폴더로</button>` : ""}<button type="button" class="chg" data-act="folder" id="pick-folder" aria-label="저장 폴더 고르기">고르기</button></div>
    </div></section>
  </form>${formsBandHtml()}</div>`;
}

// 누를 것 · 고칠 것. 그릴 때마다 열쇠(data-focus-key)를 달아, 통째로 다시 그려도 누른 것으로 포커스를 되돌린다.
const PRESSABLE = "button, input, pre.draft, [data-act]";
function keyed(root) {
  const els = [...root.querySelectorAll(PRESSABLE)];
  const keys = focusKeys(els.map((el) => ({
    act: el.dataset.act, target: el.dataset.id, slot: el.dataset.slot, option: el.dataset.option, id: el.id, name: el.name, tag: el.tagName.toLowerCase(),
  })));
  els.forEach((el, i) => { el.dataset.focusKey = keys[i]; });
}

function render() {
  const app = $("app");
  const active = document.activeElement;
  const focused = active?.dataset?.focusKey;
  const caret = active?.selectionStart != null ? [active.selectionStart, active.selectionEnd] : null;
  const scroll = $("scroll")?.scrollTop;
  // 적던 중에 다시 그려도(폴더 고르기 · 탭이 바뀐 알림) 적던 글과 커서가 남게 — 아직 change 가 나지 않아 저장하지 않은 「설정」
  // 의 글, 저장하지 않는 틀린 파일 이름 패턴도 화면에는 남는다.
  const meForm = ui.view === "me" && $("me-form");
  ui.meDraft = meForm ? Object.fromEntries(new FormData(meForm)) : null;
  if (ui.nameEdit && $("name-pattern")) ui.nameEdit.draft = $("name-pattern").value;
  app.innerHTML = ui.view === "me" ? meHtml() : mainHtml();
  keyed(app);
  if (scroll && $("scroll")) $("scroll").scrollTop = scroll;
  const back = focused && app.querySelector(`[data-focus-key="${CSS.escape(focused)}"]`);
  back?.focus({ preventScroll: true });
  if (caret && back?.setSelectionRange) back.setSelectionRange(...caret);
  // 지금 단계를 읽어 준다 — 상태줄은 그릴 때마다 새로 서므로, 그대로 남는 알림 자리(#live)에 옮겨 적는다.
  const said = app.querySelector(".status .say")?.textContent || "";
  if ($("live") && $("live").textContent !== said) $("live").textContent = said;
}

let toastTimer;
function toast(message) {
  const el = $("toast");
  el.textContent = message;
  el.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove("show"), 2400);
}

/* ── 파일로 저장(한글 서식) ─────────────────────── */

async function save({ mode = "new" } = {}) {
  let read = current();
  const t = template();
  if (!read || !t || needPick(t)) return;
  try {
    // 고른 폴더가 없으면 다운로드\계약문서 로 내려받는다(묻지 않는다). 고른 폴더의 권한 물음은 누른 곧바로(사람 동작 안에서) 한다.
    const dir = ui.folder;
    if (dir && !(await saver.permit(dir))) throw Object.assign(new Error("권한 없음"), { name: "NotAllowedError" });
    read = await fresh(read);
    if (!read) return;
    const named = fileName(read, t);
    if (named.error) throw new Error(named.error);
    let name = named.name;
    if (dir && mode === "save-as") name = await saver.freeName(dir, name);
    ui.foot = { state: "making", name };
    render();
    await frame();
    // 덮어쓰기 확인에서 만든 문서를 다시 쓰지 않는다 — 확인을 띄운 사이 그 건을 다시 읽어 값이 바뀌었을 수 있다.
    const { report, doc } = engine.make(t.bytes, read, context(), t.selections || {});
    if (report.mismatch) throw new Error(`만든 문서를 다시 읽어 보니 ${report.mismatch}칸이 달라 저장하지 않았습니다. 서식을 확인해 주세요.`);
    if (dir && mode === "new") {
      const existing = await saver.existing(dir, name);
      if (existing) {
        ui.foot = { state: "confirm", name, existing };
        render();
        return;
      }
    }
    const values = Object.fromEntries(valuesFor(read, t).map((r) => [r.field, r.value]));
    const counts = reportCounts(report.fields);
    const meta = { caseKeys: caseKeys(read), templateId: t.id, templateName: t.name, values, ...counts };
    let record, downloadId;
    if (dir) record = await saver.write(dir, name, doc, meta);
    else {
      const done = await saver.download(name, doc, meta);
      if (!done) { ui.foot = { state: "idle" }; render(); return; } // 사람이 저장 창을 닫았다 — 실패가 아니다.
      ({ record, id: downloadId } = done);
    }
    await loadSaved();
    // 내려받기는 같은 이름이 있으면 번호가 붙는다 — 실제 이름을 보인다.
    ui.foot = { state: "saved", name: record.name, ...counts, downloadId };
    toast("저장했습니다");
  } catch (error) {
    ui.foot = { state: "error", message: error.name && error.name !== "Error" ? saver.explain(error) : error.message };
  }
  render();
}

/* ── 복사(글 서식) ──────────────────────────────── */

/** 클립보드에 넣는다. 패널이 초점을 잃어 Clipboard API 가 거절하면 옛 방식(execCommand)으로 한 번 더. */
async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch { /* 아래로 */ }
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.cssText = "position:fixed;top:0;left:0;opacity:0;pointer-events:none";
  document.body.append(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  if (!ok) throw new Error("클립보드에 넣지 못했습니다. 위 「복사할 글」을 끌어 골라 Ctrl+C 로 복사해 주세요.");
}

async function copy() {
  let read = current();
  const t = template();
  if (!read || !t || t.kind !== "txt" || needPick(t)) return;
  try {
    read = await fresh(read);
    if (!read) return;
    const report = engine.makeText(t.text, read, context(), t.selections || {});
    if (report.mismatch) throw new Error(`만든 글에 바꾸지 못한 칸이 ${report.mismatch}개 있어 복사하지 않았습니다. 서식을 확인해 주세요.`);
    await copyText(report.text);
    ui.foot = { state: "copied", text: report.text };
    toast("복사했습니다");
  } catch (error) {
    ui.foot = { state: "error", message: error.message };
  }
}

async function pickFolder() {
  try {
    const dir = await saver.chooseFolder();
    if (dir) ui.folder = dir;
  } catch (error) {
    ui.foot = { state: "error", message: error.message };
  }
  render();
}

/* ── 누름 ───────────────────────────────────────── */

const actions = {
  async template(el) {
    ui.templateId = el.dataset.id;
    await store.setSetting("lastTemplate", ui.templateId);
  },
  async option(el) {
    const t = template();
    const selections = { ...(t.selections || {}), [el.dataset.slot]: el.dataset.option };
    t.selections = selections;
    ui.selections = { ...ui.selections, [t.id]: selections };
    await store.setSetting("selections", ui.selections);
    if (ui.foot.state !== "making") ui.foot = { state: "idle" };
  },
  async fold() { ui.valuesOpen = !(document.querySelector(".fold")?.getAttribute("aria-expanded") === "true"); },
  async save() { await save(); },
  async copy() { await copy(); },
  async overwrite() { await save({ mode: "overwrite" }); },
  async "save-as"() { await save({ mode: "save-as" }); },
  async cancel() { ui.foot = { state: "idle" }; },
  async folder() { await pickFolder(); },
  async "default-folder"() {
    await store.setSetting("folder", null);
    ui.folder = null;
  },
  async "show-download"() { if (ui.foot.downloadId != null) chrome.downloads.show(ui.foot.downloadId); },
  async "save-list"() { await saveToList(); },
  // 다시 확인은 서식 폴더도 다시 읽는다 — 한글에서 고쳐 저장한 서식이 곧 반영된다.
  async recheck() { await Promise.all([loadSaved(), loadTemplateFiles(), check()]); },
  async me() { ui.view = "me"; },
  async back() { ui.view = "main"; await loadTemplateFiles(); },
  async "open-templates"() {
    const reply = await ask({ op: "templates", action: "open" });
    if (reply.kind !== "answer") toast(reply.message || OFFLINE);
  },
  async "toggle-hidden"(el) {
    const id = el.dataset.id;
    ui.hidden = ui.hidden.includes(id) ? ui.hidden.filter((x) => x !== id) : [...ui.hidden, id];
    await store.setSetting("hiddenTemplates", ui.hidden);
  },
  async "name-edit"() {
    ui.nameEdit = { id: template()?.id, draft: null };
    setTimeout(() => $("name-pattern")?.focus()); // 다시 그린 뒤
  },
};

document.addEventListener("click", async (event) => {
  const el = event.target.closest("[data-act]");
  if (!el || el.disabled) return;
  const act = actions[el.dataset.act];
  if (!act) return;
  event.preventDefault();
  await act(el);
  render();
});

// 파일 이름 패턴을 적는 동안 그 아래 한 줄만 고친다(통째로 다시 그리면 적던 자리를 잃는다). 빈 칸이면 그 서식의 기본으로 짓는다.
document.addEventListener("input", (event) => {
  if (event.target.id !== "name-pattern") return;
  const t = template();
  const named = fileName(current(), t, event.target.value.trim() || defaultNamePattern(t?.id, NAME_PATTERNS));
  event.target.setAttribute("aria-invalid", String(!!named.error));
  $("name-note").outerHTML = nameNoteHtml(named);
});

/** 설정 「서식」 의 순서를 바꿔 저장한다 — 지금 차례에서 id 를 to 자리로(state.js moveTemplate). */
function moveTo(id, to) {
  ui.order = moveTemplate(ordered().map((t) => t.id), id, to);
  render();
  store.setSetting("templateOrder", ui.order).catch(() => {});
}

// 순서 손잡이: ↑ · ↓ 로 한 칸씩. 다시 그려도 손잡이의 열쇠(grip:파일 이름)가 같아 포커스가 따라간다.
document.addEventListener("keydown", (event) => {
  const el = event.target;
  if (el.dataset?.act !== "grip" || !["ArrowUp", "ArrowDown"].includes(event.key)) return;
  event.preventDefault();
  const at = ordered().findIndex((t) => t.id === el.dataset.id);
  moveTo(el.dataset.id, at + (event.key === "ArrowUp" ? -1 : 1));
});

// 순서 손잡이를 끌어 다른 줄에 놓으면 그 줄의 자리로 옮긴다.
let dragging = null;
document.addEventListener("dragstart", (event) => {
  const grip = event.target.closest?.("[data-act=grip]");
  if (!grip) return;
  dragging = grip.dataset.id;
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData("text/plain", dragging);
  grip.closest(".trow")?.classList.add("drag");
});
document.addEventListener("dragover", (event) => {
  const row = dragging && event.target.closest?.(".trow");
  if (!row) return;
  event.preventDefault();
  document.querySelectorAll(".trow.over").forEach((r) => r !== row && r.classList.remove("over"));
  row.classList.add("over");
});
document.addEventListener("drop", (event) => {
  const row = dragging && event.target.closest?.(".trow");
  if (!row) return;
  event.preventDefault();
  const id = dragging;
  dragging = null;
  moveTo(id, ordered().findIndex((t) => t.id === row.dataset.id));
});
document.addEventListener("dragend", () => {
  dragging = null;
  document.querySelectorAll(".trow.drag, .trow.over").forEach((r) => r.classList.remove("drag", "over"));
});

// 파일 이름: Enter 는 맞는 패턴이면 저장하고 줄로, Esc 는 버리고 줄로 — 그 줄의 연필로 초점을 돌린다.
document.addEventListener("keydown", (event) => {
  if (event.target.id !== "name-pattern" || event.isComposing) return;
  if (event.key === "Enter") {
    event.preventDefault();
    if (commitName(event.target)) $("name-edit")?.focus();
  } else if (event.key === "Escape") {
    event.preventDefault();
    closeNameEdit();
    $("name-edit")?.focus();
  }
});

// 칸을 떠날 때(change) 저장한다. 다시 그리지 않는다: 떠나는 순간은 대개 다른 단추를 누르는 중이라, 그 단추를 갈아 끼우면
// 누름이 사라진다.
//   파일 이름   맞는 패턴만 그 서식의 것으로 저장하고 줄로 돌아간다(아랫단만 갈아 끼운다). 틀리면 칸 아래 한 줄이 까닭을 보인다.
//   「설정」    내 정보 칸마다 곧장 저장한다 — 저장 단추가 없다.
document.addEventListener("change", async (event) => {
  const el = event.target;
  if (el.id === "name-pattern") { commitName(el); return; }
  if (ui.view !== "me" || el.form?.id !== "me-form") return;
  ui.me = { ...ui.me, [el.name]: el.value.trim() };
  await store.setSetting("me", ui.me);
});

/* ── 시작 ───────────────────────────────────────── */

async function start() {
  const wasm = new Uint8Array(await (await fetch(chrome.runtime.getURL("engine.wasm"))).arrayBuffer());
  [engine, engineHash] = await Promise.all([loadEngine(wasm), saver.sha256(wasm)]);
  const v = engine.vocabulary();
  vocabMe = v.me || [];
  MISSING = v.missing || MISSING;
  ui.windowId = (await chrome.windows.getCurrent()).id; // 호스트에 묻는 것(서식 폴더도)은 창 번호를 싣는다
  // downloads.search 는 디스크 확인이 끝나기 전에 답할 수 있다. 첫 조회부터 뒤늦은 삭제 확인을 듣는다.
  chrome.downloads.onChanged.addListener((delta) => {
    if (delta.exists) loadSaved().then(render).catch(() => {});
  });
  await loadSettings();
  // 중계기가 이 창의 탭이 바뀌었거나 그 화면을 다시 읽었다고 알린다(windowId 가 null 이면 연결이 서거나 끊겼다).
  chrome.runtime.onMessage.addListener((message, sender) => {
    if (sender.id !== chrome.runtime.id || message?.type !== "pclm-document-changed") return;
    if (message.windowId === null || message.windowId === ui.windowId) recheck();
  });
  // 다른 창의 패널이 바꾼 것(서식 설정 · 내 정보 · 저장한 파일)과 서식 폴더를 다시 읽고, 그사이 계약목록에서 고친 값을 다시 묻는다.
  document.addEventListener("visibilitychange", async () => {
    if (document.visibilityState !== "visible") return;
    await loadAll();
    render();
    recheck();
  });
  window.addEventListener("focus", () => { loadSaved().then(render).catch(() => {}); });
  render();
  // 서식 폴더 읽기(한글 서식 컴파일)와 화면 확인을 함께 — 서식이 늦게 와도 상태줄은 먼저 선다.
  await Promise.all([loadTemplateFiles().then(render), check()]);
}

start().catch((error) => {
  $("app").innerHTML = `<div class="status"><span class="say bad"><span>문서 만들기를 열지 못했습니다</span></span></div><div class="sec"><div class="box bad"><p>${esc(error.message)}</p></div></div>`;
  throw error;
});
