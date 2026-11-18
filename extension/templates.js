// 서식 — 서식 폴더의 파일(ADR-049). 확장은 폴더에 닿지 못하므로 호스트(templates 메서드)가 목록과 바이트를 넘긴다. 한글 ·
// 메모장에서 고쳐 저장하면 그것이 곧 반영이다 — 패널은 열 때 · 「이 화면 다시 확인」 때 · 설정에서 돌아올 때 다시 읽는다.
// 서식의 정체는 파일 이름이다(보이는 이름은 확장자를 뗀 것). 서식의 기준은 언제나 폴더의 파일이다.
//
// 두 종류가 있다(kind — 확장자로 가린다):
//   "hwpx" 한글 서식. 폴더에는 컴파일 전 모습(본문에 {{#항목}}·{{#선택}} 표시가 글로 보인다)으로 두고, 엔진 prepare 가 구조로
//          바꾼다. 원본 파일은 건드리지 않는다. 만든 문서는 폴더에 저장한다. 컴파일본은 원본 바이트의 SHA-256 과 엔진(wasm 의
//          SHA-256)을 붙여 확장의 IndexedDB(store.js prepared)에 담아 두고, 둘이 같으면 다시 컴파일하지 않는다 — 지워도 되는
//          파생물이라 바이트는 매번 폴더에서 받고 해시만 견준다.
//   "txt"  글 서식(기안문 등). {{칸}} 과 줄을 혼자 차지하는 {{#항목}}·{{#선택}} 표시(문서나르미 txt 서식과 같은 문법).
//          만든 글은 패널이 클립보드로 복사한다 — 결재 시스템 본문에 붙여 넣는다. 파일로 저장하지 않는다.
// 읽거나 받아들이지 못한 서식은 error 한 줄을 달고 남는다 — 그 줄만 고를 수 없고 다른 서식은 그대로 쓴다.

import { sha256 } from "./saver.js";

/**
 * 동봉 서식의 기본 파일 이름 패턴 {파일 이름: 패턴}(사람이 그 서식에 정하면 그것이 먼저, 없으면 엔진 기본). 통합 뷰의 열만
 * 쓴다(DocumentPanel.test.ts 가 대조한다) — 없는 칸을 쓰면 아무것도 실패하지 않은 채 그 서식의 저장이 막힌다.
 */
export const NAME_PATTERNS = {
  "물품 구매입찰 공고.hwpx": "공고서-({{입찰공고번호}})-{{공고명}}",
};

export const kindOf = (fileName) => (/\.txt$/i.test(fileName) ? "txt" : "hwpx");
const stem = (fileName) => fileName.replace(/\.(hwpx|txt)$/i, "");

/**
 * 글 서식의 바이트 → 글. UTF-8(BOM 은 뗀다)이 아니면 메모장의 옛 기본값인 EUC-KR(CP949)로 읽는다.
 * 줄 끝은 \n 으로 맞춘다(붙여 넣을 때 Chrome 이 Windows 줄 끝으로 바꾼다).
 */
export function decodeText(bytes) {
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    text = new TextDecoder("euc-kr").decode(bytes);
  }
  return text.replace(/\r\n?/g, "\n");
}

/** 호스트가 실어 보낸 Base64 → 바이트. */
export function fromBase64(text) {
  const raw = atob(text);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return bytes;
}

/**
 * 서식 파일 하나 → 패널이 쓰는 서식 {id(파일 이름), name, kind, bytes | text, fields, slots} 또는 {id, name, kind, error}.
 * bytes 가 null 이면 읽지 못한 것이다(readError 가 까닭).
 */
export function accept(engine, fileName, bytes, readError = null) {
  const base = { id: fileName, name: stem(fileName), kind: kindOf(fileName) };
  if (!bytes) return { ...base, error: readError || "서식을 읽지 못했습니다" };
  let report, content;
  try {
    if (base.kind === "txt") {
      const text = decodeText(bytes);
      report = engine.inspectText(text);
      content = { text };
    } else {
      const prepared = engine.prepare(bytes);
      report = prepared.report;
      content = { bytes: prepared.bytes };
    }
  } catch (error) {
    return { ...base, error: error.message };
  }
  if (report.problems.length) return { ...base, error: `서식을 고쳐 주세요: ${report.problems.join(" / ")}` };
  if (!report.fields.length && !report.slots.length) return { ...base, error: "서식에 칸이 없습니다" };
  return { ...base, ...content, fields: report.fields, slots: report.slots };
}

/** 한글 서식 하나 — 원본 바이트의 SHA-256 과 엔진이 캐시와 같으면 prepare 를 건너뛴다. 캐시가 실패하면 그냥 컴파일한다. */
async function acceptCached(engine, cache, cached, fileName, bytes) {
  if (!cache || !bytes || kindOf(fileName) !== "hwpx") return accept(engine, fileName, bytes);
  const source = await sha256(bytes).catch(() => null);
  const hit = cached[fileName];
  if (source && hit?.source === source && hit.engine === cache.engine) {
    return { id: fileName, name: stem(fileName), kind: "hwpx", bytes: hit.bytes, fields: hit.fields, slots: hit.slots };
  }
  const t = accept(engine, fileName, bytes);
  if (source && !t.error) {
    try {
      await cache.put(fileName, { source, engine: cache.engine, bytes: t.bytes, fields: t.fields, slots: t.slots });
    } catch { /* 다음에 다시 컴파일할 뿐이다 */ }
  }
  return t;
}

/**
 * 서식 폴더를 읽는다. ask(params) 는 호스트 templates 메서드에 묻는 것(패널 → 중계기)이고 {kind, answer|message} 를 돌려준다.
 * cache 는 컴파일본 저장소 {engine, all(), put(이름, 기록), remove(이름)}(패널은 store.js 의 prepared) — 없으면 매번 컴파일한다.
 * 돌려주는 것: {folder, templates} 또는 {error: 한 줄}(목록부터 받지 못했다 — 연결 없음 등).
 */
export async function loadTemplates(engine, ask, cache = null) {
  const listed = await ask({ action: "list" });
  if (listed.kind !== "answer") return { error: listed.message || "서식 폴더를 읽지 못했습니다" };
  const { folder, files } = listed.answer;
  const read = await Promise.all(files.map((f) => ask({ action: "read", name: f.name })));
  let cached = {};
  try {
    if (cache) cached = (await cache.all()) || {};
  } catch { /* 캐시 없이 컴파일한다 */ }
  const templates = [];
  for (const [i, f] of files.entries()) {
    const reply = read[i];
    templates.push(reply.kind === "answer"
      ? await acceptCached(engine, cache, cached, f.name, fromBase64(reply.answer.data))
      : accept(engine, f.name, null, reply.message));
  }
  const names = new Set(files.map((f) => f.name));
  for (const name of Object.keys(cached)) {
    if (names.has(name)) continue;
    try {
      await cache.remove(name);
    } catch { /* 다음 읽기에서 다시 지운다 */ }
  }
  return { folder, templates };
}
