// 문서 만들기 패널이 지금 무엇을 보일지 정하는 셈(ADR-045 1·4항과 덧붙여 정한 것). Chrome 없이 시험할 수 있게 DOM ·
// chrome.* 에 닿지 않는 순수 함수만 둔다 — panel.js 가 이 결과대로 그린다.
//
//   stage(reply)          중계기(background.js documentCall)의 답 → {stage, lines, …}
//   readNotes(read)       호스트 document {op:"read"} 의 답 → 값 위에 한 줄씩 보일 알림, 옛 차수인가
//   caseKeys(read)        저장한 파일을 이 건에 잇는 열쇠(본번호) — 차수가 올라도 같은 건이다
//   splitValues(rows)     채워질 값 → 빈 칸(위에 늘 펼친다)과 채워진 칸(접어 둔다)
//   namePatternFor(…)     서식의 파일 이름 패턴 — 사람이 정한 것 → 동봉 기본 → 엔진 기본
//   orderTemplates(…)     서식 폴더의 파일 → 설정의 순서대로(새 파일은 맨 아래) · shownTemplates 숨긴 것 빼기 · moveTemplate 옮기기
//   usable(t)             고를 수 있는 서식인가 — 오류 난 서식은 그 줄에 까닭만 보인다
//   nameCheck(result, …)  엔진이 지은 파일 이름 → 이름 또는 「「업체」 칸이 없습니다」 한 줄
//   focusKeys(list)       다시 그려도 같은 누를 것에 같은 열쇠 — 그리기 뒤 포커스를 되돌린다
//   meMismatch(…)         서식이 내 정보 칸을 쓸 때, 내 정보가 그 건의 공고 값과 다른 항목
//   draftPieces(…)        만든 글의 조각 → 미리보기에 그릴 [{text, mark}] — 서식의 글 · 들어간 값 · 【확인 필요】

export const EMPTY = "나라장터 수집 화면을 열면 여기에 건이 보입니다";
export const EMPTY_HINT = "계약요청 접수 · 입찰공고 · 계약서 화면에서 이 탭의 건으로 서식을 채웁니다.";
// 연결이 없을 때 패널이 대는 까닭.
export const OFFLINE = "계약 목록에 연결되지 않았습니다";
export const OFFLINE_HINT = "계약 목록을 실행하면 자동으로 이어집니다.";
export const READING = "화면을 읽는 중입니다.";
// 확장에는 계약목록 창을 앞으로 부르는 길이 없다(호스트 명령은 창 → 확장 한 방향뿐이다). 그래서 어디를 보라고만 말한다.
export const OPEN_WINDOW = "계약 목록 창 오른쪽 「지금 보는 화면」에서 이 화면을 확인하세요. 창이 닫혀 있으면 계약 목록을 여세요.";

export const KIND_LABEL = { request: "접수", notice: "공고", contract: "계약" };

// 새 건이면 무엇이 다른지 대신 건 카드에 이 한 줄을 보인다(차수가 새로 올라온 것도 새 것이다).
export const NEW_CASE = "계약목록에 아직 없는 건(차수)입니다";

/** 「계약목록에 저장」 단추 위의 한 줄 — 몇 칸이 다른지. 새 건이면 건 카드(NEW_CASE)가 말하므로 없다. */
export function differsLine(answer) {
  return `계약목록에 저장된 것과 ${answer.changedCount}칸이 다릅니다.`;
}

/**
 * 중계기의 답 → 패널의 단계.
 *   empty    나라장터 수집 화면이 아니다(다른 사이트 · 수집하지 않는 나라장터 화면)
 *   offline  계약 목록에 닿지 못한다 · 준비가 필요하다
 *   reading  탭을 아직 읽는 중
 *   error    화면을 읽지 못했다 · 호스트가 거절했다
 *   same     저장된 것과 같다 → 읽어서 보이고 만든다
 *   differs  다르다 → 「계약목록에 저장」 만
 *   blocked  저장하지 못하는 화면 → 까닭과 「지금 보는 화면」
 */
export function stage(reply) {
  switch (reply?.kind) {
    case "elsewhere":
      return { stage: "empty", lines: [EMPTY] };
    case "offline":
      return { stage: "offline", lines: [OFFLINE, OFFLINE_HINT] };
    case "reading":
      return { stage: "reading", lines: [READING] };
    case "error":
      return { stage: "error", lines: [reply.message || "화면을 읽지 못했습니다."] };
    case "failed":
      // 연결 쪽 실패(호스트 없음 · 준비 필요)는 rpc() 가 사람 말로 바꿔 둔다.
      return reply.connection
        ? { stage: "offline", lines: [reply.message || OFFLINE] }
        : { stage: "error", lines: [reply.message || "계약 목록의 응답을 확인하지 못했습니다."] };
    case "answer":
      break;
    default:
      return { stage: "offline", lines: [OFFLINE, OFFLINE_HINT] };
  }
  const answer = reply.answer || {};
  const who = { entity: answer.entity ?? null, entityType: answer.entityType ?? null, saved: answer.saved === true };
  if (answer.state === "same") return { stage: "same", lines: [], ...who };
  // 무엇이 다른지 — 호스트가 앞의 몇 개만 싣는다. 수는 changedCount 다.
  const diff = { isNew: answer.isNew === true, changedCount: answer.changedCount || 0, changes: Array.isArray(answer.changes) ? answer.changes : [] };
  if (answer.state === "differs") return { stage: "differs", lines: answer.isNew ? [] : [differsLine(answer)], ...who, ...diff };
  if (answer.state === "blocked") return { stage: "blocked", lines: [answer.reason || "이 화면은 저장할 수 없습니다.", OPEN_WINDOW], ...who, ...diff };
  return { stage: "error", lines: ["계약 목록의 응답을 확인하지 못했습니다."] };
}

/**
 * 읽은 값 위에 한 줄씩 보일 알림. older 면 만들지 않는다 — 통합 뷰는 최신 차수의 값을 내므로 옛 차수 화면의 값이 아니다.
 * 답이 없으면(null) 계약목록에 없는 건이다 — 같다고 한 뒤에 지워졌다.
 */
export function readNotes(read) {
  if (!read) return { older: true, lines: ["이 건이 계약목록에 없습니다. 이 화면을 다시 확인하세요."] };
  if (read.older) {
    return { older: true, lines: ["나라장터에서 최신 차수를 여세요."] }; // 옛 차수라는 것은 상태줄이 말한다
  }
  const lines = [];
  if (read.rows > 1) {
    lines.push(`이 건에 계약 ${read.rows}건이 이어져 있어 계약마다 다른 칸은 비워 둡니다. 계약 화면을 열면 그 계약으로 만듭니다.`);
  }
  return { older: false, lines };
}

/** 이 건의 열쇠 — 「kind:본번호」(호스트가 싣는다). 저장한 파일 기록(saved.caseKeys)이 이 건의 것인지 가린다. */
export function caseKeys(read) {
  return read?.caseKeys || [];
}

/**
 * 저장해 둔 항목 고르기 가운데 지금 서식에 있는 항목 · 선택만. 사람이 서식에서 항목을 고치거나 지우면 옛 고르기가 남는데,
 * 엔진은 서식에 없는 항목 · 선택을 거절해 그 서식을 영영 만들 수 없게 된다 — 버리면 다시 고르라고 묻는다.
 */
export function keptSelections(slots, selections) {
  return Object.fromEntries(Object.entries(selections || {})
    .filter(([slot, option]) => (slots || []).some((s) => s.id === slot && s.options.some((o) => o.id === option))));
}

/** 채워질 값을 빈 칸과 채워진 칸으로 — 서식의 차례를 지킨다. 빈 칸은 문서에 【확인 필요】로 남는 칸이다. */
export function splitValues(rows) {
  const missing = [], filled = [];
  for (const row of rows || []) (row.value === null || row.value === undefined ? missing : filled).push(row);
  return { missing, filled };
}

/** 조사 값은 엔진이 고른다. 일반 칸은 어휘·사용자 열 이름으로 찾고, 시스템 토큰은 데이터 열로 찾지 않는다. */
export function fieldRows(fields, values, renderJosa) {
  const key = (s) => s.replace(/[\s_·-]/g, "");
  const seen = new Set();
  return (fields || []).filter((f) => !seen.has(f.name) && seen.add(f.name)).map((f) => {
    if (f.kind === "josa") {
      try {
        return renderJosa(f.name);
      } catch (error) {
        return { ...f, field: f.name, value: null, error: error.message };
      }
    }
    const name = f.canonical || Object.keys(values).find((k) => key(k) && key(k) === key(f.name));
    return { field: f.name, canonical: name || null, value: name ? values[name] ?? null : null };
  });
}

/** 조사 병기는 빈 값 표시와 다르다 — 완료 문구에서도 확인할 조사를 따로 센다. */
export function reportCounts(fields) {
  return {
    missing: (fields || []).filter((f) => f.kind !== "josa" && f.missing).length,
    review: (fields || []).filter((f) => f.kind === "josa" && f.status === "needs_review").length,
  };
}

// 파일 이름 패턴이 비었을 때 엔진(naming::DEFAULT_PATTERN)이 쓰는 것 — 동봉 기본 패턴이 없는 서식의 기본.
export const DEFAULT_NAME_PATTERN = "{{건명}}_{{서식이름}}";

/** 서식의 기본 파일 이름 패턴 — 동봉 서식이 정한 것(templates.js NAME_PATTERNS {파일 이름: 패턴}), 없으면 엔진 기본. */
export function defaultNamePattern(id, defaults) {
  return defaults?.[id] || DEFAULT_NAME_PATTERN;
}

/** 서식의 파일 이름 패턴 — 사람이 그 서식에 정한 것(patterns: {파일 이름: 패턴}) → 동봉 기본 → 엔진 기본. */
export function namePatternFor(id, patterns, defaults) {
  return patterns?.[id] || defaultNamePattern(id, defaults);
}

/**
 * 서식을 설정의 순서(order: 파일 이름들)대로 — 순서에 든 것이 그 차례로 먼저, 폴더에 새로 생긴 것은 맨 아래(받은 차례).
 * 사라진 파일의 기록은 조용히 무시한다.
 */
export function orderTemplates(list, order) {
  const at = new Map((order || []).map((name, i) => [name, i]));
  return (list || []).map((t, i) => [t, i])
    .sort(([a, i], [b, j]) => (at.get(a.id) ?? Infinity) - (at.get(b.id) ?? Infinity) || i - j)
    .map(([t]) => t);
}

/** 만들기 화면의 서식 — 숨긴 것(hidden: 파일 이름들)을 뺀다. */
export function shownTemplates(list, hidden) {
  return (list || []).filter((t) => !(hidden || []).includes(t.id));
}

/** 고를 수 있는 서식인가 — 읽기 · 받아들이기 오류가 난 서식은 그 줄에 까닭만 보이고 고를 수 없다. */
export const usable = (t) => !!t && !t.error;

/** 순서를 옮긴 뒤의 차례 — ids(지금 차례)에서 id 를 to 자리(0 부터, 넘으면 끝으로 붙인다)로. 없는 id 면 그대로. */
export function moveTemplate(ids, id, to) {
  if (!ids.includes(id)) return [...ids];
  const next = ids.filter((x) => x !== id);
  next.splice(Math.max(0, Math.min(to, next.length)), 0, id);
  return next;
}

/**
 * 만들 건 — 읽은 날(readDay)이 오늘(day)이 아니면 reread() 로 다시 읽은 것, 아니면 그대로. 오늘_년월일 · 계약보증금납부기한은
 * 호스트가 읽는 날로 서므로(ADR-051), 자정을 넘겨 열어 둔 패널이 어제 날짜를 찍지 않게 만들기 직전에 본다. 다시 읽어 만들 수
 * 없으면 reread 가 null 을 낸다.
 */
export async function freshRead(read, readDay, day, reread) {
  return read && readDay !== day ? reread() : read;
}

/**
 * 엔진 fileName 의 답 → {name} | {error: 한 줄} | {}. 건이 없으면(hasCase 거짓) 칸 목록을 모르니 문법 오류만 보고 이름도 내지 않는다.
 */
export function nameCheck(result, hasCase) {
  if (result?.missing?.length) return hasCase ? { error: `${result.missing.map((n) => `「${n}」`).join(" · ")} 칸이 없습니다` } : {};
  if (result?.error) return { error: result.error };
  return hasCase && result?.name ? { name: result.name } : {};
}

// 내 정보와 견줄 통합 뷰의 열 — 부서 · 팩스는 견줄 상대가 없다.
const ME_PAIRS = [
  { key: "name", label: "이름", column: "공고담당자", same: (mine, notice) => notice.includes(mine) }, // 자동 수집의 이름 조건과 같다
  { key: "phone", label: "연락처", column: "공고담당 전화번호", same: (mine, notice) => mine.replace(/\D/g, "") === notice.replace(/\D/g, "") },
];

/**
 * 내 정보가 그 건의 공고 값과 다른 항목 → [{label, mine, notice}]. 서식의 칸(fields: [{canonical}])이 내 정보 칸(meNames)을 하나도
 * 쓰지 않으면 빈 목록. 어느 쪽이든 비었으면 그 항목은 견주지 않는다. 다르다는 것만 보이고 막지 않는다(ADR-016).
 */
export function meMismatch(fields, meNames, me, values) {
  if (!(fields || []).some((f) => (meNames || []).includes(f.canonical))) return [];
  return ME_PAIRS.flatMap(({ key, label, column, same }) => {
    const mine = String(me?.[key] ?? "").trim();
    const notice = String(values?.[column] ?? "").trim();
    return mine && notice && !same(mine, notice) ? [{ label, mine, notice }] : [];
  });
}

/**
 * 글 서식으로 만든 글의 조각(엔진 makeText 의 segments: [{kind, text}]) → [{text, mark}]. mark 는 서식의 글이면 null, 값이 없어
 * 【확인 필요】가 들어간 자리면 "missing", 값이 들어간 자리면 "fill". 보이기만 한다 — 복사하는 글은 report.text 그대로다.
 */
export function draftPieces(segments, missing) {
  return (segments || []).map(({ kind, text, needs_review }) => ({ text, mark: kind === "literal" ? null : text === missing || needs_review || kind === "review" ? "missing" : "fill" }));
}

/**
 * 누를 것마다의 열쇠 — 하는 일(data-act)과 그 대상(서식 · 고를 항목 · 선택지), 없으면 요소 id · 입력 이름. 같은 열쇠가 또 나오면
 * 차례로 #2 · #3 을 붙인다. 화면을 통째로 다시 그려도 같은 것은 같은 열쇠를 받아, 누른 단추로 포커스가 돌아간다.
 */
export function focusKeys(list) {
  const seen = new Map();
  return (list || []).map((x) => {
    const base = [x.act || (x.id ? `#${x.id}` : x.name ? `name:${x.name}` : x.tag || "?"), x.target, x.slot, x.option].filter(Boolean).join(":");
    const n = (seen.get(base) || 0) + 1;
    seen.set(base, n);
    return n === 1 ? base : `${base}#${n}`;
  });
}
