// @vitest-environment node
// 문서 만들기 패널(ADR-045) — 확장 안의 모듈을 Chrome 없이 본다: 무엇을 보일지 정하는 셈(panel/state.js), 엔진 wasm
// (engine.js, 나르미 컴팩트의 시험을 옮겼다), 서식 폴더 읽기(templates.js, ADR-049), 저장 실패 문구(saver.js). 엔진은 빌드 산출물이라
// ./scripts/build-engine.ps1 이 먼저 extension/engine.wasm 을 지어 두어야 한다(run.ps1 · publish.ps1 이 그 차례를 쥔다).
import { describe, expect, it, vi } from "vitest";

const root = new URL("../../", import.meta.url);
// 확장은 번들러 없는 ES 모듈이라 타입이 없다 — 경로를 글로 넘겨 any 로 받는다. 화면 쪽 tsconfig 에는 node 타입이 없어
// node:fs 도 같은 길로 받는다.
const load = (path: string): Promise<any> => import(/* @vite-ignore */ new URL(path, root).href);
const { existsSync, readFileSync, readdirSync } = await (import(/* @vite-ignore */ "node:fs" as string) as Promise<any>);
const state = await load("extension/panel/state.js");
const { loadEngine } = await load("extension/engine.js");
const { decodeText, accept, loadTemplates, NAME_PATTERNS } = await load("extension/templates.js");
const { explain, sha256, waitDownload, savedFiles, download, write, DEFAULT_LABEL } = await load("extension/saver.js");

const wasm = new URL("extension/engine.wasm", root);
if (!existsSync(wasm)) throw new Error("extension/engine.wasm 이 없습니다 — ./scripts/build-engine.ps1 을 먼저 돌리세요.");
const engine = await loadEngine(readFileSync(wasm));

const shipped = new URL("extension/templates/", root);
const fixtures = new URL("crates/pclm-doc-engine/tests/fixtures/", root);
const dirs: URL[] = [shipped, new URL("templates/", fixtures)];
const template = (name: string) => new Uint8Array(readFileSync(new URL(name, dirs.find((d) => readdirSync(d).includes(name)))));

// 호스트 document {op:"read"} 의 답 꼴 — 공고까지 온 건의 통합 뷰 한 줄(값은 지어낸 것).
const read = {
  entityType: "notice",
  entity: "R26BK00000003-001",
  values: {
    건명: "청사 복도 LED 조명 교체 물품 구매", 단계: "공고", 입찰공고번호: "R26BK00000003-001", 공고명: "청사 복도 LED 조명 교체 물품 구매",
    계약방법: "일반경쟁", 입찰방법: "제한경쟁", 사업예산: "67,100,000원", 추정가격: "55,454,545원", 담당부서: "청사관리과",
    담당자: "김수요", "담당자 전화번호": "000-1111-2222", 개찰일시: "2026. 11. 17. 14:00", 게시일시: "2026. 11. 3. 09:30",
    수요기관: "○○시 청사관리과", 세부품명: "LED 조명", 수량: "120", 단위: "개", 대표계약업체: "", 계약금액: "",
    세부품명_목록: "LED 조명", 세부품명_번호목록: "LED 조명(0000000000)", 세부품명_수량목록: "LED 조명 / 120 개",
    추정가격_한글: "오천오백사십오만사천오백사십오", 계약금액_한글: "",
  },
  rows: 1,
  older: false,
  caseKeys: ["notice:R26BK00000003"],
};
const context = { me: { dept: "회계과", name: "홍길동", phone: "000-0000-0000" } };
const today = "2026-11-09";

describe("패널이 보일 단계", () => {
  const answer = (fields: object) => ({ kind: "answer", tabId: 1, answer: { entity: "R26BK00000003-001", entityType: "notice", isNew: false, changedCount: 0, ...fields } });

  it("나라장터 수집 화면이 아니면 비운 패널에 한 줄만 보인다", () => {
    for (const reply of [{ kind: "elsewhere" }]) {
      expect(state.stage(reply)).toEqual({ stage: "empty", lines: ["나라장터 수집 화면을 열면 여기에 건이 보입니다"] });
    }
    // 그 아래 한 줄로 어떤 화면에서 쓰는지 말한다.
    expect(state.EMPTY_HINT).toBe("계약요청 접수 · 입찰공고 · 계약서 화면에서 이 탭의 건으로 서식을 채웁니다.");
  });

  it("연결이 없거나 준비가 필요하면 그 까닭을 말한다", () => {
    expect(state.stage({ kind: "offline" }).lines).toEqual(["계약 목록에 연결되지 않았습니다", "계약 목록을 실행하면 자동으로 이어집니다."]);
    // 중계기의 rpc() 가 준비 필요(setup_required)를 수집과 같은 글로 바꿔 둔다.
    const setup = "계약 목록 준비가 필요합니다. 계약 목록의 「Chrome 확장 상태」 를 확인해 주세요.";
    expect(state.stage({ kind: "failed", connection: true, message: setup })).toEqual({ stage: "offline", lines: [setup] });
    expect(state.stage(undefined).stage).toBe("offline");
    expect(state.stage({ kind: "failed", message: "저장 대상이 바뀌었습니다. 다시 확인하세요." }))
      .toEqual({ stage: "error", lines: ["저장 대상이 바뀌었습니다. 다시 확인하세요."] });
    expect(state.stage({ kind: "reading" }).stage).toBe("reading");
    expect(state.stage({ kind: "error", message: "화면에서 번호를 읽지 못했습니다." }).lines).toEqual(["화면에서 번호를 읽지 못했습니다."]);
  });

  it("같으면 읽으러 가고, 다르면 새 건인지 몇 칸인지 한 줄로 말하고, 막히면 까닭과 「지금 보는 화면」 을 가리킨다", () => {
    expect(state.stage(answer({ state: "same" }))).toEqual({ stage: "same", lines: [], entity: "R26BK00000003-001", entityType: "notice", saved: false });
    // 새 건은 무엇이 다른지 대신 건 카드에 「아직 없는 건」 을 보이고, 단추 위에는 아무 줄도 없다.
    const fresh = state.stage(answer({ state: "differs", isNew: true, changedCount: 12, changes: [{ label: "공고명", before: "", after: "합성 공고" }] }));
    expect(fresh).toMatchObject({ stage: "differs", isNew: true, changedCount: 12, lines: [] });
    expect(state.NEW_CASE).toBe("계약목록에 아직 없는 건(차수)입니다");
    // 무엇이 다른지는 호스트가 면 표기로 실은 그대로(패널은 값을 견주지도 다시 적지도 않는다). 옛 호스트처럼 changes 가 없으면 빈 목록이다.
    const changes = [{ label: "공고명", before: "합성 공고", after: "바뀐 합성 공고" }, { label: "개찰일시", before: "", after: "2026/11/17 14:00" }];
    const differs = state.stage(answer({ state: "differs", changedCount: 3, changes }));
    expect(differs.lines).toEqual(["계약목록에 저장된 것과 3칸이 다릅니다."]);
    expect(differs).toMatchObject({ isNew: false, changedCount: 3, changes });
    expect(state.stage(answer({ state: "differs", changedCount: 3 })).changes).toEqual([]);
    // 품목 금액과 총액이 다른 화면은 check 에서는 differs, save 에서야 blocked 다 — 어느 쪽이든 까닭을 그대로 보인다.
    const blocked = state.stage(answer({ state: "blocked", reason: "품목 금액의 합이 계약금액과 다릅니다." }));
    expect(blocked.stage).toBe("blocked");
    expect(blocked.lines).toEqual(["품목 금액의 합이 계약금액과 다릅니다.", state.OPEN_WINDOW]);
    expect(state.stage(answer({ state: "blocked", reason: "품목 금액의 합이 계약금액과 다릅니다.", changedCount: 2, changes })).changes).toEqual(changes);
    expect(state.OPEN_WINDOW).toMatch(/「지금 보는 화면」/);
    // 짝짓지도 못한 화면(수집 규칙이 바뀜)은 무슨 건인지 모른다.
    expect(state.stage(answer({ state: "blocked", entity: null, entityType: null, reason: "수집 규칙이 바뀌었습니다." })))
      .toMatchObject({ stage: "blocked", entity: null });
  });

  it("저장한 뒤의 답은 저장한 뒤 다시 본 check 꼴이다 — 같으면 읽고, 막혔으면 막힌 것을 보인다", () => {
    expect(state.stage(answer({ state: "same", saved: true }))).toMatchObject({ stage: "same", saved: true });
    expect(state.stage(answer({ state: "blocked", saved: false, reason: "미리보기 이후 자료가 바뀌었습니다." })))
      .toMatchObject({ stage: "blocked", lines: ["미리보기 이후 자료가 바뀌었습니다.", state.OPEN_WINDOW] });
  });

  it("읽은 값 위의 알림 — 계약이 여럿 · 옛 차수", () => {
    expect(state.readNotes(read)).toEqual({ older: false, lines: [] });
    // 통합 뷰가 계약마다 한 줄을 내면(rows) 계약마다 다른 칸은 호스트가 비워 보낸다.
    expect(state.readNotes({ ...read, rows: 3 }).lines)
      .toEqual(["이 건에 계약 3건이 이어져 있어 계약마다 다른 칸은 비워 둡니다. 계약 화면을 열면 그 계약으로 만듭니다."]);
    const older = state.readNotes({ ...read, older: true });
    expect(older.older).toBe(true);
    expect(older.lines).toHaveLength(1);
    expect(older.lines[0]).toMatch(/최신 차수를 여세요/);
    // 같다고 한 뒤에 지워져 답이 비면 만들지 않는다.
    expect(state.readNotes(null).older).toBe(true);
  });

  it("저장한 파일은 호스트가 실은 본번호로 그 건에 잇는다 — 차수가 올라도 같은 건이다", () => {
    expect(state.caseKeys(read)).toEqual(["notice:R26BK00000003"]);
    expect(state.caseKeys(null)).toEqual([]);
  });
});

describe("채워질 값 · 다시 그리기", () => {
  it("조사는 데이터 열로 찾지 않고 엔진의 선택·확인 필요·오류를 보인다", () => {
    const system = { name: "@조사:수요기관:은/는", kind: "josa", target: "수요기관", pair: "은/는", canonical: "수요기관" };
    const selected = { field: system.name, kind: "josa", target: "수요기관", pair: "은/는", value: "는", status: "selected", missing: false };
    const calls: string[] = [];
    const rows = state.fieldRows([{ name: "사용자 칸" }, system, system], { 사용자칸: "값", 수요기관: "국방부" }, (name: string) => {
      calls.push(name);
      return selected;
    });
    expect(rows).toEqual([{ field: "사용자 칸", canonical: "사용자칸", value: "값" }, selected]);
    expect(calls).toEqual([system.name]);
    expect(state.fieldRows([system], {}, () => { throw new Error("조사 대상 필드가 없습니다"); })[0])
      .toMatchObject({ kind: "josa", target: "수요기관", value: null, error: "조사 대상 필드가 없습니다" });
    const unresolved = { ...selected, value: "은(는)", status: "needs_review", missing: true, reason: "unknown_pronunciation" };
    expect(state.fieldRows([system], {}, () => unresolved)).toEqual([unresolved]);
    expect(state.reportCounts([selected, unresolved, { field: "계약금액", missing: true }]))
      .toEqual({ missing: 1, review: 1 });
  });

  it("조사 확인 표시는 미리보기에만 남고, 병기를 일반 빈 칸과 따로 센다", () => {
    const segments = [{ kind: "literal", text: "은(는) " }, { kind: "fill", text: "ABC" }, { kind: "fill", text: "은(는)", needs_review: true }];
    const pieces = state.draftPieces(segments, "【확인 필요】");
    expect(pieces).toEqual([{ text: "은(는) ", mark: null }, { text: "ABC", mark: "fill" }, { text: "은(는)", mark: "missing" }]);
    expect(pieces.map((p: any) => p.text).join("")).toBe("은(는) ABC은(는)");
    expect(state.reportCounts([{ kind: "josa", missing: true, status: "needs_review" }])).toEqual({ missing: 0, review: 1 });
  });

  it("빈 칸(【확인 필요】로 남을 칸)과 채워진 칸으로 가른다 — 서식의 차례를 지킨다", () => {
    const rows = [
      { field: "공고명", value: "합성 공고" }, { field: "수량", value: null }, { field: "담당자", value: "" },
      { field: "분할납품", value: null }, { field: "개찰일시", value: "2026/11/17 14:00" },
    ];
    const { missing, filled } = state.splitValues(rows);
    expect(missing.map((r: any) => r.field)).toEqual(["수량", "분할납품"]);
    // 빈 글은 채워진 값이다 — 계약목록에 빈 값으로 저장된 것을 문서에 그대로 찍는다.
    expect(filled.map((r: any) => r.field)).toEqual(["공고명", "담당자", "개찰일시"]);
    expect(state.splitValues([])).toEqual({ missing: [], filled: [] });
  });

  it("내 정보가 공고와 다른 항목 — 이름은 들어 있으면 같고, 연락처는 숫자만 견주고, 빈 값은 건너뛰고, 막지 않고 보이기만 한다", () => {
    const meNames = ["내_부서", "내_이름", "내_연락처", "내_팩스"];
    const fields = [{ canonical: "공고명" }, { canonical: "내_이름" }];
    const me = { dept: "합성과", name: "홍길동", phone: "02-000-0000", fax: "02-000-0001" };
    const at = (values: object, mine: object = me, f: object[] = fields) => state.meMismatch(f, meNames, mine, values);
    expect(at({ 공고담당자: "홍길동", "공고담당 전화번호": "02-000-0000" })).toEqual([]);
    // 공고담당자 값에 내 이름이 들어 있으면 같다(자동 수집의 이름 조건과 같다). 연락처는 꾸밈을 떼고 숫자만.
    expect(at({ 공고담당자: "주무관 홍길동", "공고담당 전화번호": "(02) 000 0000" })).toEqual([]);
    expect(at({ 공고담당자: "김공고", "공고담당 전화번호": "02-000-0009" })).toEqual([
      { label: "이름", mine: "홍길동", notice: "김공고" },
      { label: "연락처", mine: "02-000-0000", notice: "02-000-0009" },
    ]);
    // 어느 쪽이든 비면 그 항목은 견주지 않는다. 부서 · 팩스는 견줄 열이 없다.
    expect(at({ 공고담당자: "", "공고담당 전화번호": "02-000-0009" })).toEqual([{ label: "연락처", mine: "02-000-0000", notice: "02-000-0009" }]);
    expect(at({ 공고담당자: "김공고", "공고담당 전화번호": "02-000-0009" }, { name: " ", phone: "" })).toEqual([]);
    expect(at({ 담당부서: "다른과", 담당자: "김수요" })).toEqual([]);
    // 내 정보 칸을 하나도 쓰지 않는 서식이면 견주지 않는다.
    expect(at({ 공고담당자: "김공고" }, me, [{ canonical: "공고명" }, { canonical: null }])).toEqual([]);
    // 실린 공고서는 「① 계약담당자」 줄에 내 정보를 다른 이름(계약담당자 · 계약담당 전화번호 · 계약담당 팩스번호)으로 쓴다 —
    // 다른 이름도 canonical 이 내 정보라 견준다. 「② 수요기관 연락처」 줄은 통합 뷰의 열이다.
    const { report } = engine.prepare(template("물품 구매입찰 공고.hwpx"));
    const canonical = (name: string) => report.fields.find((f: any) => f.name === name)?.canonical;
    expect(["계약담당자", "계약담당 전화번호", "계약담당 팩스번호"].map(canonical)).toEqual(["내_이름", "내_연락처", "내_팩스"]);
    expect(["담당부서", "담당자", "담당자 전화번호"].map(canonical)).toEqual(["담당부서", "담당자", "담당자 전화번호"]);
    expect(report.fields.some((f: any) => f.name.startsWith("내_"))).toBe(false);
    expect(at({ 공고담당자: "김공고" }, me, report.fields)).toHaveLength(1);
  }, 20_000);

  it("누를 것의 열쇠는 다시 그려도 같고, 겹치면 차례로 가른다 — 포커스를 되돌릴 자리", () => {
    const list = [
      { act: "recheck" }, { act: "me" },
      { act: "template", target: "가.hwpx" }, { act: "template", target: "나.txt" }, { act: "grip", target: "가.hwpx" },
      { act: "toggle-hidden", target: "가.hwpx" }, { act: "back" },
      { act: "option", slot: "s1", option: "o1" }, { act: "option", slot: "s1", option: "o2" },
      { act: "me" }, { id: "copy", tag: "button" }, { name: "dept", tag: "input" }, { tag: "input" }, { tag: "input" },
    ];
    const keys = state.focusKeys(list);
    expect(keys).toEqual([
      "recheck", "me", "template:가.hwpx", "template:나.txt", "grip:가.hwpx", "toggle-hidden:가.hwpx", "back",
      "option:s1:o1", "option:s1:o2", "me#2", "#copy", "name:dept", "input", "input#2",
    ]);
    expect(new Set(keys).size).toBe(keys.length);
    expect(state.focusKeys(list)).toEqual(keys);
    // 앞에 누를 것이 하나 더 서도 다른 일의 열쇠는 그대로다.
    expect(state.focusKeys([{ act: "fold" }, ...list]).slice(1)).toEqual(keys);
  });
});

describe("엔진(wasm)", () => {
  it("열의 다른 이름과 내 정보를 읽고, 패널이 부르지 않는 요청(판 · 살피기 · 바로 채우기 · 수집)은 싣지 않는다", () => {
    const vocab = engine.vocabulary();
    expect(vocab.missing).toBe("【확인 필요】");
    expect(vocab).not.toHaveProperty("names");
    expect(vocab).not.toHaveProperty("derived");
    expect(vocab).not.toHaveProperty("won");
    expect(vocab.aka.계약금액_한글).toEqual(["한글금액"]);
    expect(vocab.me.map((m: any) => m.name)).toEqual(["내_부서", "내_이름", "내_연락처", "내_팩스"]);
    for (const gone of ["version", "inspect", "fill", "capture", "profiles"]) expect(engine).not.toHaveProperty(gone);
  });

  it("통합 뷰의 값은 그대로 두고 내 정보만 더한다", () => {
    const { values } = engine.resolve(read, context);
    const filled = Object.entries(read.values).filter(([, v]) => v !== "");
    for (const [k, v] of filled) expect(values[k], k).toBe(v);
    expect(values.내_이름).toBe("홍길동");
    expect(values).not.toHaveProperty("계약금액");
    expect(Object.keys(values).length).toBe(filled.length + 3);
  });

  it("통합 뷰의 건으로 만들기: 칸 이름이 열 이름이면 채우고 아니면 【확인 필요】", () => {
    const { report } = engine.make(template("계약체결 안내.hwpx"), read, context);
    expect(report.mismatch).toBe(0);
    const line = (f: string) => report.fields.find((x: any) => x.field === f);
    expect(line("수요기관").value).toBe("○○시 청사관리과");
    expect(line("사업명").canonical).toBe(null);
    expect(line("사업명").value).toBe("【확인 필요】");
    expect(line("계약금액").value).toBe("【확인 필요】");
  }, 20_000);

  it("항목 서식: 고른 선택만 남기고, 덜 고르면 까닭 글로 막는다", () => {
    const { report: inspected, bytes: t } = engine.prepare(template("물품 구매입찰 공고.hwpx"));
    expect(inspected.slots.map((s: any) => s.label)).toEqual(["입찰참가자격", "낙찰자 결정방법"]);
    expect(inspected.fields.filter((f: any) => !f.canonical).map((f: any) => f.name)).toEqual([]);

    // panel.js가 서식마다 저장한 고르기를 거른 뒤, 빠진 선택을 다시 받아 만든다.
    const stored = { "slot-i": "option-i1", "slot-n": "option-gone", "slot-old": "option-x" };
    expect(() => engine.make(t, read, context, stored)).toThrow();
    const kept = state.keptSelections(inspected.slots, stored);
    expect(kept).toEqual({ "slot-i": "option-i1" });
    expect(state.keptSelections(undefined, undefined)).toEqual({});
    expect(() => engine.make(t, read, context, kept)).toThrow(/「낙찰자 결정방법」을 골라 주세요/);
    const { report, doc } = engine.make(t, read, context, { ...kept, "slot-n": "option-n2" });
    expect(report.mismatch).toBe(0);
    expect(report.excluded).toHaveLength(4);
    expect(report.fields.find((f: any) => f.field === "공고명").value).toBe("청사 복도 LED 조명 교체 물품 구매");
    expect(engine.prepare(doc).report.slots.every((s: any) => s.options.length <= 1)).toBe(true);
  }, 20_000);

  // panel.js 의 save() · copy() 가 만들기 직전에 타는 셈(fresh → freshRead) — 다시 읽기는 check() 이고 그 뒤의 current() 가 답이다.
  it("읽은 날이 지났으면 만들기 전에 한 번 더 읽고, 문서에는 다시 읽은 값이 든다", async () => {
    const before = { ...read, values: { ...read.values, 오늘_년월: "2026. 11." } };
    const after = { ...read, values: { ...read.values, 오늘_년월: "2026. 12." } };
    let reads = 0;
    const reread = async () => { reads++; return after; };
    expect(await state.freshRead(before, "2026-11-30", "2026-11-30", reread)).toBe(before);
    expect(reads).toBe(0);
    const kase = await state.freshRead(before, "2026-11-30", "2026-12-01", reread);
    expect(reads).toBe(1);
    const report = engine.makeText("{{오늘_년월}}", kase, context);
    expect(report.fields.find((f: any) => f.field === "오늘_년월").value).toBe("2026. 12.");
    expect(report.text).toBe("2026. 12.");
    // 다시 읽어 만들 수 없게 되면(건이 없어짐 · 막힘) null — 만들지 않는다.
    expect(await state.freshRead(before, "2026-11-30", "2026-12-01", async () => null)).toBe(null);
  });

  it("파일 이름은 패턴이 비면 건명과 서식 이름이다", () => {
    const name = "청사 복도 LED 조명 교체 물품 구매_입찰 공고문.hwpx";
    expect(engine.fileName(read, "입찰 공고문.hwpx")).toEqual({ name });
    expect(engine.fileName(read, "입찰 공고문.hwpx", state.DEFAULT_NAME_PATTERN, today)).toEqual({ name });
  });

  it("파일 이름 패턴: 저장 이름에 들고, 없는 칸이면 이름 대신 한 줄 — 건이 없으면 문법만 본다", () => {
    // panel.js 의 fileName() 과 같은 셈 — 저장(save)과 아랫단 이름 줄, 「파일 이름」 칸 아래 한 줄이 모두 이것을 탄다.
    const named = (kase: any, pattern: string) => state.nameCheck(engine.fileName(kase || {}, "입찰 공고문", pattern, today), !!kase);
    expect(named(read, "{{date:YYYYMMDD}}_{{입찰공고번호}}_{{서식이름}}")).toEqual({ name: "20261109_R26BK00000003-001_입찰 공고문.hwpx" });
    // 빈 값(대표계약업체)은 자리를 비우고 남은 구분을 정리한다.
    expect(named(read, "{{대표계약업체}}_{{건명}}")).toEqual({ name: "청사 복도 LED 조명 교체 물품 구매.hwpx" });
    expect(named(read, "{{업체}}_{{건명}}_{{담당}}")).toEqual({ error: "「업체」 · 「담당」 칸이 없습니다" });
    expect(named(read, "{{seq}}_{{건명}}")).toEqual({ error: "「seq」 칸이 없습니다" });
    expect(named(read, "{{건명").error).toMatch(/닫히지 않았/);
    expect(named(null, "{{업체}}_{{건명}}")).toEqual({});
    expect(named(null, "{{건명").error).toMatch(/닫히지 않았/);
  });

  it("파일 이름 패턴은 서식(파일 이름)마다 — 사람이 정한 것 → 동봉 기본 → 엔진 기본", () => {
    const bid = "물품 구매입찰 공고.hwpx";
    expect(state.namePatternFor(bid, {}, NAME_PATTERNS)).toBe("공고서-({{입찰공고번호}})-{{공고명}}");
    expect(state.namePatternFor(bid, { [bid]: "{{건명}}" }, NAME_PATTERNS)).toBe("{{건명}}");
    // 다른 서식에 정한 것은 이 서식에 닿지 않는다. 동봉 기본이 없는 서식(사람이 폴더에 둔 서식)은 엔진 기본이다.
    expect(state.namePatternFor("내 공고.hwpx", { [bid]: "{{건명}}" }, NAME_PATTERNS)).toBe(state.DEFAULT_NAME_PATTERN);
    expect(state.namePatternFor("내 공고.hwpx", undefined, NAME_PATTERNS)).toBe("{{건명}}_{{서식이름}}");
    expect(state.defaultNamePattern(bid, NAME_PATTERNS)).toBe("공고서-({{입찰공고번호}})-{{공고명}}");
    const named = state.nameCheck(engine.fileName(read, "물품 구매입찰 공고", state.namePatternFor(bid, {}, NAME_PATTERNS), today), true);
    expect(named).toEqual({ name: "공고서-(R26BK00000003-001)-청사 복도 LED 조명 교체 물품 구매.hwpx" });
  });

  it("동봉 기본 파일 이름 패턴은 통합 뷰에 있는 칸만 쓴다 — 없는 칸이면 아무것도 실패하지 않은 채 그 서식의 저장이 막힌다", () => {
    const views = JSON.parse(readFileSync(new URL("crates/pclm-core/src/storage/presets/builtin-views.json", root), "utf8"));
    const columns: string[] = views.views.find((v: any) => v.name === "문서 서식").definition.columns.map((c: any) => c.name);
    const full = { ...read, values: Object.fromEntries(columns.map((c) => [c, `${c} 값`])) };
    const patterned = Object.entries(NAME_PATTERNS) as [string, string][];
    expect(patterned.length).toBeGreaterThan(0);
    for (const [file, pattern] of patterned) {
      expect(file).toMatch(/\.hwpx$/); // 글 서식에는 파일 이름이 없다
      expect(readdirSync(shipped)).toContain(file); // 동봉 서식의 파일 이름이 열쇠다 — 이름을 바꾸면 기본 패턴이 조용히 떨어진다
      expect(state.nameCheck(engine.fileName(full, file.replace(/\.hwpx$/, ""), pattern, today), true), file).toHaveProperty("name");
    }
  });

  it("여러 번 불러도 엔진 메모리가 늘지 않는다", () => {
    // 패널에서 문서를 연달아 만든다. 입력을 돌려주지 않으면 200번에 서식 크기 × 200(수 MB)만큼 는다.
    const t = template("계약체결 안내.hwpx");
    for (let i = 0; i < 5; i++) engine.make(t, read, context);
    const before = engine.memoryBytes;
    for (let i = 0; i < 200; i++) engine.make(t, read, context);
    expect(engine.memoryBytes).toBe(before);
  }, 60_000);

  const texts = (readdirSync(shipped) as string[]).filter((n) => n.endsWith(".txt")).sort();
  for (const name of texts) {
    it(`${name}: 기본 글 서식의 칸은 모두 통합 뷰의 열이고 항목 표시가 바르다`, () => {
      const report = engine.inspectText(decodeText(template(name)));
      expect(report.problems).toEqual([]);
      expect(report.fields.length).toBeGreaterThan(0);
      expect(report.fields.filter((f: any) => !f.canonical).map((f: any) => f.name)).toEqual([]);
    });
  }

  it("글 서식으로 만들기: 통합 뷰의 값과 【확인 필요】로 바꾼다", () => {
    const text = decodeText(template("낙찰자 선정 및 계약체결 안내.txt"));
    const report = engine.makeText(text, read, context);
    expect(report.mismatch).toBe(0);
    expect(report.path).toBe("txt");
    expect(report.text).toMatch(/^낙찰자 선정 및 계약체결 안내\(LED 조명\)\n/);
    expect(report.text).toMatch(/1\. 관련: 입찰공고번호 R26BK00000003-001\n/);
    expect(report.text).toMatch(/라\. 계약 상대자: 【확인 필요】\n/);
    expect(report.text).not.toMatch(/\{\{/);
    expect(report.fields.find((f: any) => f.field === "입찰방법").value).toBe("제한경쟁");

    // 미리보기는 칸마다 표시를 세운다 — 값이 들어간 자리는 fill, 값이 없는 자리는 missing. 복사하는 글(report.text)은 조각을
    // 이은 것 그대로라 표시가 섞이지 않는다.
    const pieces = state.draftPieces(report.segments, "【확인 필요】");
    expect(pieces.map((p: any) => p.text).join("")).toBe(report.text);
    expect(pieces.filter((p: any) => p.mark).length).toBe(text.match(/\{\{[^#/][^}]*\}\}/g).length);
    const fills = pieces.filter((p: any) => p.mark === "fill").map((p: any) => p.text);
    expect(fills).toContain("R26BK00000003-001");
    expect(fills).not.toContain("【확인 필요】");
    expect(pieces.filter((p: any) => p.mark === "missing").every((p: any) => p.text === "【확인 필요】")).toBe(true);
    expect(pieces.some((p: any) => p.mark === "missing")).toBe(true);
    expect(report.text).not.toMatch(/<mark/);

    // HWPX의 값 미리보기도 같은 TXT 엔진으로 조사만 평가한다 — 화면에서 받침을 다시 계산하지 않는다.
    const josaText = "‘{{수요기관}}’{{@조사:수요기관:은/는}} · {{@조사:수량:은/는}}";
    const fields = engine.inspectText(josaText).fields;
    const rows = state.fieldRows(fields, engine.resolve(read, context).values,
      (name: string) => engine.makeText(`{{${name}}}`, read, context).fields[0]);
    expect(rows.find((r: any) => r.field === "@조사:수요기관:은/는"))
      .toMatchObject({ kind: "josa", value: "는", status: "selected" });
    expect(rows.find((r: any) => r.field === "@조사:수량:은/는"))
      .toMatchObject({ kind: "josa", value: "은(는)", status: "needs_review", reason: "unknown_pronunciation" });
    const josaReport = engine.makeText(josaText, read, context);
    const josaPieces = state.draftPieces(josaReport.segments, "【확인 필요】");
    expect(josaPieces.filter((p: any) => p.mark === "missing")).toEqual([{ text: "은(는)", mark: "missing" }]);
    expect(josaPieces.map((p: any) => p.text).join("")).toBe("‘○○시 청사관리과’는 · 은(는)");
    expect(state.reportCounts(josaReport.fields)).toEqual({ missing: 0, review: 1 });
  });
});

describe("서식 폴더(ADR-049)", () => {
  const b64 = (bytes: Uint8Array) => btoa(Array.from(bytes, (b) => String.fromCharCode(b)).join(""));
  const text = (t: string) => new TextEncoder().encode(t);

  it("읽거나 받아들이지 못한 서식은 그 줄에만 까닭을 달고 고를 수 없다", () => {
    const broken = accept(engine, "깨진 공고.hwpx", new Uint8Array([1, 2, 3]));
    expect(broken).toMatchObject({ id: "깨진 공고.hwpx", name: "깨진 공고", kind: "hwpx" });
    expect(broken.error).toMatch(/서식을 읽지 못했습니다/);
    expect(state.usable(broken)).toBe(false);
    expect(accept(engine, "큰 서식.hwpx", null, "서식이 너무 큽니다: 큰 서식.hwpx")).toMatchObject({ error: "서식이 너무 큽니다: 큰 서식.hwpx" });
    expect(accept(engine, "빈 기안.txt", text("칸 없는 글")).error).toBe("서식에 칸이 없습니다");
    expect(accept(engine, "깨진 기안.txt", text("{{#항목 a 가}}\n{{건명}}")).error).toMatch(/^서식을 고쳐 주세요/);
    const ok = accept(engine, "합성 기안.TXT", text("제목: {{건명}}"));
    expect(ok).toMatchObject({ id: "합성 기안.TXT", name: "합성 기안", kind: "txt", text: "제목: {{건명}}" });
    expect(state.usable(ok)).toBe(true);
    expect(state.usable(null)).toBe(false);
  });

  it("목록과 파일을 호스트에서 받아 서식으로 — 하나가 깨져도 나머지는 그대로 쓴다", async () => {
    const files: Record<string, Uint8Array> = { "합성 기안.txt": text("제목: {{건명}}"), "깨진 공고.hwpx": new Uint8Array([9, 9]) };
    const asked: any[] = [];
    const ask = async (params: any) => {
      asked.push(params);
      if (params.action === "list") return { kind: "answer", answer: { folder: "C:\\합성\\서식", files: Object.keys(files).map((name) => ({ name, kind: name.endsWith(".txt") ? "txt" : "hwpx", size: files[name].length })) } };
      if (params.name === "깨진 공고.hwpx") return { kind: "failed", message: "서식이 너무 큽니다: 깨진 공고.hwpx" };
      return { kind: "answer", answer: { name: params.name, kind: "txt", data: b64(files[params.name]) } };
    };
    const loaded = await loadTemplates(engine, ask);
    expect(loaded.folder).toBe("C:\\합성\\서식");
    expect(loaded.templates.map((t: any) => [t.id, state.usable(t)])).toEqual([["합성 기안.txt", true], ["깨진 공고.hwpx", false]]);
    expect(loaded.templates[1].error).toBe("서식이 너무 큽니다: 깨진 공고.hwpx");
    expect(asked.filter((a) => a.action === "read").map((a) => a.name).sort()).toEqual(Object.keys(files).sort());
    expect(await loadTemplates(engine, async () => ({ kind: "offline" }))).toEqual({ error: "서식 폴더를 읽지 못했습니다" });
  });

  it("한글 서식의 컴파일본은 원본 바이트와 엔진이 같을 때만 다시 쓴다 — 사라진 파일 · 오류 서식은 캐시에 남지 않는다", async () => {
    const original = template("물품 구매입찰 공고.hwpx");
    const files: Record<string, Uint8Array> = { "공고.hwpx": original, "깨진 공고.hwpx": new Uint8Array([9, 9]), "합성 기안.txt": text("제목: {{건명}}") };
    const ask = async (params: any) => params.action === "list"
      ? { kind: "answer", answer: { folder: "C:\\합성\\서식", files: Object.keys(files).map((name) => ({ name })) } }
      : { kind: "answer", answer: { name: params.name, data: b64(files[params.name]) } };
    let prepared = 0;
    const counting = { ...engine, prepare: (b: Uint8Array) => { prepared++; return engine.prepare(b); } };
    const map = new Map<string, any>();
    const cache = (id: string) => ({
      engine: id,
      all: async () => Object.fromEntries(map),
      put: async (name: string, record: any) => void map.set(name, record),
      remove: async (name: string) => void map.delete(name),
    });

    const first = await loadTemplates(counting, ask, cache("엔진1"));
    const compiled = first.templates.find((t: any) => t.id === "공고.hwpx");
    expect(compiled).toMatchObject({ id: "공고.hwpx", name: "공고", kind: "hwpx" });
    expect(compiled).not.toHaveProperty("error");
    expect(compiled.slots.map((s: any) => s.label)).toEqual(["입찰참가자격", "낙찰자 결정방법"]);
    expect(compiled.bytes).not.toEqual(original); // 폴더의 원본은 컴파일 전 모습이다.
    expect(engine.prepare(compiled.bytes).bytes).toEqual(compiled.bytes);
    expect(prepared).toBe(2); // 공고 · 깨진 공고 — 글 서식은 prepare 를 타지 않는다
    expect([...map.keys()]).toEqual(["공고.hwpx"]); // 오류 서식은 담지 않는다
    expect(map.get("공고.hwpx")).toMatchObject({ source: await sha256(original), engine: "엔진1" });

    const second = await loadTemplates(counting, ask, cache("엔진1"));
    expect(prepared).toBe(3); // 깨진 공고만 다시
    expect(second.templates).toEqual(first.templates);

    const before = map.get("공고.hwpx").source;
    files["공고.hwpx"] = compiled.bytes; // 검증한 컴파일본으로 바꾸어 원본 바이트 지문이 달라진다.
    prepared = 0;
    await loadTemplates(counting, ask, cache("엔진1"));
    expect(prepared).toBe(2);
    expect(map.get("공고.hwpx").source).not.toBe(before);

    prepared = 0;
    await loadTemplates(counting, ask, cache("엔진2"));
    expect(prepared).toBe(2);
    expect(map.get("공고.hwpx").engine).toBe("엔진2");

    delete files["공고.hwpx"];
    await loadTemplates(counting, ask, cache("엔진2"));
    expect(map.size).toBe(0);

    files["공고.hwpx"] = original;
    const failing = { engine: "엔진2", all: async () => { throw new Error("막힘"); }, put: async () => { throw new Error("막힘"); }, remove: async () => {} };
    const loaded = await loadTemplates(counting, ask, failing);
    expect(loaded.templates.find((t: any) => t.id === "공고.hwpx")).not.toHaveProperty("error"); // 캐시가 실패해도 컴파일한다
  }, 60_000);

  it("순서와 숨김은 파일 이름을 열쇠로 — 새 파일은 맨 아래, 사라진 파일의 기록은 무시한다", () => {
    const list = ["가.hwpx", "나.txt", "다.txt", "새 서식.hwpx"].map((id) => ({ id }));
    const ids = (l: any[]) => l.map((t) => t.id);
    expect(ids(state.orderTemplates(list, ["다.txt", "사라진.hwpx", "가.hwpx", "나.txt"]))).toEqual(["다.txt", "가.hwpx", "나.txt", "새 서식.hwpx"]);
    expect(ids(state.orderTemplates(list, undefined))).toEqual(ids(list));
    expect(ids(state.shownTemplates(list, ["나.txt", "사라진.hwpx"]))).toEqual(["가.hwpx", "다.txt", "새 서식.hwpx"]);
    expect(ids(state.shownTemplates(list, undefined))).toEqual(ids(list));
    const order = ids(list);
    expect(state.moveTemplate(order, "다.txt", 0)).toEqual(["다.txt", "가.hwpx", "나.txt", "새 서식.hwpx"]);
    expect(state.moveTemplate(order, "가.hwpx", 1)).toEqual(["나.txt", "가.hwpx", "다.txt", "새 서식.hwpx"]); // ↓ 한 칸
    expect(state.moveTemplate(order, "가.hwpx", -1)).toEqual(order); // 맨 위에서 ↑
    expect(state.moveTemplate(order, "새 서식.hwpx", 9)).toEqual(order); // 맨 아래에서 ↓
    expect(state.moveTemplate(order, "없는.txt", 0)).toEqual(order);
  });
});

describe("서식 읽기 · 저장 문구", () => {
  it("글 서식 읽기: UTF-8(BOM)·EUC-KR, 줄 끝은 LF", () => {
    const utf8 = new Uint8Array([0xef, 0xbb, 0xbf, ...new TextEncoder().encode("제목: {{건명}}\r\n끝.")]);
    expect(decodeText(utf8)).toBe("제목: {{건명}}\n끝.");
    const euckr = new Uint8Array([0xc1, 0xa6, 0xb8, 0xf1, 0x0d, 0x0a]); // 「제목」\r\n
    expect(decodeText(euckr)).toBe("제목\n");
  });

  // 저장 실패를 사람 말로 — 컴팩트의 spike 에서 본 오류 이름마다. 이전 파일이 그대로라는 것을 꼭 말한다.
  const named = (name: string) => Object.assign(new Error(name), { name });
  it("한글에서 연 파일 · 다른 프로그램이 쥔 파일", () => {
    expect(explain(named("InvalidStateError"))).toMatch(/한글에서 .*닫은 뒤 다시 저장하세요\. 이전 파일은 그대로입니다\./);
    expect(explain(named("NotReadableError"))).toMatch(/다른 프로그램이 .*이전 파일은 그대로입니다\./);
  });

  it("권한 · 폴더 · 자리", () => {
    expect(explain(named("NotAllowedError"))).toMatch(/수정 허용/);
    expect(explain(named("NotFoundError"))).toMatch(/「설정」에서 다시 고르세요/);
    expect(explain(named("QuotaExceededError"))).toMatch(/자리가 없어/);
    expect(explain(new Error("알 수 없음"))).toMatch(/저장하지 못했습니다: 알 수 없음/);
  });

  // 고른 폴더가 없을 때의 길(chrome.downloads) — 가짜 downloads 로 끝남 · 취소 · 실패를 가른다.
  function fakeDownloads(item: any) {
    const listeners = new Set<(delta: any) => void>();
    const calls: any[] = [];
    return {
      calls, listeners, item,
      download: async (options: object) => { calls.push(options); return 7; },
      search: async ({ id }: { id: number }) => (id === 7 ? [item] : []),
      onChanged: { addListener: (f: any) => listeners.add(f), removeListener: (f: any) => listeners.delete(f) },
      fire: (delta: object) => [...listeners].forEach((f) => f(delta)),
    };
  }
  const tick = () => new Promise((r) => setTimeout(r, 0));

  it("기본 자리 내려받기: 끝나면 번호가 붙었을 수 있는 실제 이름을, 다른 내려받기의 소식은 듣지 않는다", async () => {
    expect(DEFAULT_LABEL).toBe("다운로드\\계약문서");
    const d = fakeDownloads({ state: "in_progress" });
    const options = { url: "blob:x", filename: "계약문서/합성 공고.hwpx", conflictAction: "uniquify", saveAs: false };
    const done = waitDownload(d, options);
    await tick();
    expect(d.calls).toEqual([options]);
    d.fire({ id: 8, state: { current: "complete" } }); // 남의 내려받기
    await tick();
    expect(d.listeners.size).toBe(1);
    d.item.state = "complete";
    d.item.filename = "C:\\Users\\합성\\Downloads\\계약문서\\합성 공고 (1).hwpx";
    d.fire({ id: 7, state: { current: "complete" } });
    expect(await done).toEqual({ id: 7, name: "합성 공고 (1).hwpx" });
    expect(d.listeners.size).toBe(0); // 끝나면 뗀다.
    // 듣기 전에 이미 끝난 것도 안다.
    expect(await waitDownload(fakeDownloads({ state: "complete", filename: "/home/합성/계약문서/합성.hwpx" }), options)).toEqual({ id: 7, name: "합성.hwpx" });
  });

  it("기본 자리 내려받기: 사람이 저장 창을 닫으면 null, 다른 중단은 사람 말로", async () => {
    const options = { url: "blob:x", filename: "계약문서/합성.hwpx" };
    expect(await waitDownload(fakeDownloads({ state: "interrupted", error: "USER_CANCELED" }), options)).toBeNull();
    const full = await waitDownload(fakeDownloads({ state: "interrupted", error: "FILE_NO_SPACE" }), options).catch((e: Error) => e);
    expect(full.name).toBe("FILE_NO_SPACE");
    expect(explain(full)).toMatch(/자리가 없어/);
    const denied = await waitDownload(fakeDownloads({ state: "interrupted", error: "FILE_ACCESS_DENIED" }), options).catch((e: Error) => e);
    expect(explain(denied)).toMatch(/「설정」에서 다른 폴더를/);
  });

  it("저장 위치를 기록하고, 삭제가 확인된 문서만 경고 대상에서 뺀다 — 조회 실패와 예전 기록은 보존한다", async () => {
    const persisted = new Map<string, any>();
    const db = { transaction: () => {
      const tx: any = { objectStore: () => ({ put: (record: any, name: string) => {
        persisted.set(name, record);
        queueMicrotask(() => tx.oncomplete());
        return {};
      } }) };
      return tx;
    } };
    vi.stubGlobal("indexedDB", { open: () => {
      const request: any = { result: db };
      queueMicrotask(() => request.onsuccess());
      return request;
    } });
    const d = fakeDownloads({ state: "complete", filename: "C:\\Downloads\\계약문서\\내려받은.hwpx" });
    vi.stubGlobal("chrome", { downloads: d, runtime: { id: "우리 확장" } });
    try {
      const bytes = new TextEncoder().encode("문서");
      const meta = { values: { 건명: "옛 값" }, caseKeys: ["notice:1"] };
      const downloaded = await download("내려받은.hwpx", bytes, meta);
      expect(persisted.get("내려받은.hwpx").downloadId).toBe(7);
      const file = { getFile: async () => ({}), createWritable: async () => ({ write: async () => {}, close: async () => {} }) };
      const written = await write({ getFileHandle: async () => file }, "폴더.hwpx", bytes, meta);
      expect(persisted.get("폴더.hwpx").fileHandle).toBe(file);
      const gone = () => { throw named("NotFoundError"); };
      const records = {
        ...Object.fromEntries(persisted),
        "삭제.hwpx": { fileHandle: { getFile: gone } },
        "권한.hwpx": { fileHandle: { getFile: () => { throw named("NotAllowedError"); } } },
        "일시오류.hwpx": { fileHandle: { getFile: () => { throw named("NotReadableError"); } } },
        "기록없음.hwpx": { downloadId: 8 },
        "검색실패.hwpx": { downloadId: 9 },
        "이전 (1).hwpx": { savedAt: downloaded.record.savedAt },
        "다른 확장.hwpx": { savedAt: downloaded.record.savedAt },
        "다른 시각.hwpx": { savedAt: downloaded.record.savedAt },
        "모호.hwpx": { savedAt: downloaded.record.savedAt },
      };
      const legacy = { state: "complete", byExtensionId: "우리 확장", endTime: new Date(downloaded.record.savedAt - 50).toISOString(), exists: false };
      d.search = async (query: any) => {
        if (query.id === 7) return [d.item];
        if (query.id === 8) return []; // 기록 삭제는 파일 삭제를 뜻하지 않는다.
        if (query.id === 9) throw new Error("조회 실패");
        const pattern = new RegExp(query.filenameRegex);
        const name = Object.keys(records).find((n) => pattern.test(`C:\\Downloads\\계약문서\\${n}`));
        expect(pattern.test("C:\\Downloads\\다른 폴더\\이전 (1).hwpx")).toBe(false);
        if (name === "이전 (1).hwpx") return [legacy];
        if (name === "다른 확장.hwpx") return [{ ...legacy, byExtensionId: "다른 확장" }];
        if (name === "다른 시각.hwpx") return [{ ...legacy, endTime: new Date(downloaded.record.savedAt - 60_000).toISOString() }];
        if (name === "모호.hwpx") return [legacy, legacy];
        return [];
      };
      const first = await savedFiles(records);
      expect(Object.keys(first)).toEqual(Object.keys(records).filter((n) => !["삭제.hwpx", "이전 (1).hwpx"].includes(n)));
      expect(first["폴더.hwpx"]).toBe(written); // 실제로 남은 파일은 옛 값 경고 대상이다.
      d.item.exists = false; // search 이후 도착하는 downloads.onChanged 의 확인 결과.
      expect(await savedFiles(records)).not.toHaveProperty("내려받은.hwpx");
      expect(persisted.size).toBe(2); // 경고만 거른다. 추적 기록은 지우지 않는다.
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("지문은 SHA-256", async () => {
    expect(await sha256(new TextEncoder().encode("abc"))).toBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
  });
});

describe("panel.js 저장", () => {
  it("덮어쓰기 확인을 띄운 사이 그 건을 다시 읽어 값이 바뀌었으면 덮어쓰기는 새 값으로 만든다", async () => {
    // 패널 통째로(panel.js) — DOM 은 jsdom, IndexedDB(store.js)는 메모리, 고른 폴더는 같은 이름 파일이 늘 있는 대역.
    const { JSDOM } = await (import(/* @vite-ignore */ "jsdom" as string) as Promise<any>);
    const dom = new JSDOM(`<div id="app"></div><p id="live"></p><div id="toast"></div>`, { pretendToBeVisual: true });
    const kept = new Map<string, any>();
    const id = "계약체결 안내.hwpx";
    const files = new Map<string, Uint8Array>();
    const dir = {
      name: "합성 폴더", queryPermission: async () => "granted",
      getFileHandle: async (name: string) => ({
        getFile: async () => ({ arrayBuffer: async () => (files.get(name) ?? new Uint8Array([1])).slice().buffer, lastModified: 0 }),
        createWritable: async () => ({ write: async (b: Uint8Array) => void files.set(name, b), close: async () => {}, abort: async () => {} }),
      }),
    };
    kept.set("settings/folder", dir);
    kept.set("settings/lastTemplate", id);
    vi.doMock("../../extension/store.js", () => {
      const all = async (s: string) => Object.fromEntries([...kept].filter(([k]) => k.startsWith(s + "/")).map(([k, v]) => [k.slice(s.length + 1), v]));
      const get = async (s: string, k: string) => kept.get(`${s}/${k}`);
      const put = async (s: string, k: string, v: any) => void kept.set(`${s}/${k}`, v);
      const remove = async (s: string, k: string) => void kept.delete(`${s}/${k}`);
      return { all, get, put, remove, setting: (k: string) => get("settings", k), setSetting: (k: string, v: any) => put("settings", k, v) };
    });
    let shown = read;
    let reads = 0;
    const listeners: any[] = [];
    const reply = async (m: any) => {
      if (m.op === "templates") return m.action === "list" ? { kind: "answer", answer: { folder: "C:\\합성\\서식", files: [{ name: id }] } }
        : { kind: "answer", answer: { name: id, data: btoa(Array.from(template(id), (b) => String.fromCharCode(b)).join("")) } };
      if (m.op === "check") return { kind: "answer", tabId: 1, read: 1, answer: { entity: read.entity, entityType: read.entityType, isNew: false, changedCount: 0, state: "same" } };
      if (m.op === "read") { reads++; return { kind: "answer", answer: shown }; }
      return { kind: "offline" };
    };
    vi.stubGlobal("document", dom.window.document);
    vi.stubGlobal("window", dom.window);
    vi.stubGlobal("requestAnimationFrame", dom.window.requestAnimationFrame);
    vi.stubGlobal("fetch", async () => ({ arrayBuffer: async () => readFileSync(wasm) }));
    vi.stubGlobal("chrome", {
      runtime: { id: "extension", getURL: (f: string) => f, sendMessage: reply, onMessage: { addListener: (fn: any) => listeners.push(fn) } },
      windows: { getCurrent: async () => ({ id: 10 }) },
      downloads: { onChanged: { addListener: () => {} }, search: async () => [] },
    });
    try {
      vi.resetModules();
      await load("extension/panel/panel.js");
      const $ = (sel: string): any => dom.window.document.querySelector(sel);
      await vi.waitFor(() => expect($("[data-act=save]")).not.toBe(null), { timeout: 20_000 });
      $("[data-act=save]").click();
      await vi.waitFor(() => expect($("[data-act=overwrite]")).not.toBe(null), { timeout: 20_000 });
      // 확인을 띄운 사이 계약목록에서 값을 고쳤다 — 같은 건을 다시 읽는다(탭이 바뀌었다는 알림 → 다시 확인).
      shown = { ...read, values: { ...read.values, 수요기관: "△△시 회계과" } };
      listeners.forEach((fn) => fn({ type: "pclm-document-changed", windowId: 10 }, { id: "extension" }));
      await vi.waitFor(() => expect(reads).toBe(2));
      await new Promise((r) => setTimeout(r, 0));
      expect($("[data-act=overwrite]")).not.toBe(null); // 같은 건이라 확인은 그대로 떠 있다.
      $("[data-act=overwrite]").click();
      await vi.waitFor(() => expect($("#toast").textContent).toBe("저장했습니다"), { timeout: 20_000 });
      const [[name, written]] = [...files];
      const compiled = engine.prepare(template(id)).bytes;
      expect(written).toEqual(engine.make(compiled, shown, { me: {} }, {}).doc);
      expect(written).not.toEqual(engine.make(compiled, read, { me: {} }, {}).doc);
      expect(kept.get(`saved/${name}`).values.수요기관).toBe("△△시 회계과");
    } finally {
      vi.unstubAllGlobals();
      vi.doUnmock("../../extension/store.js");
      vi.resetModules();
    }
  }, 60_000);
});
