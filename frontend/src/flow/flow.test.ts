import { describe, expect, it } from "vitest";
import type { Flow, FlowEnv, Step } from "./ast";
import { checkFlow } from "./check";
import { evaluate } from "./evaluate";
import { migrateV1, toV2 } from "./migrate";
import { compileFlow, checkSqlMock } from "./sql";
import { FlowSyntaxError, normalizeFlow, parseFlow, printFlow } from "./syntax";
import { verbs } from "./verbs";
import { 품목행 } from "../mock";
import builtinViews from "../../../src/Pclm.Core/Storage/Presets/builtin-views.json";

/**
 * 흐름식의 위험은 <b>조용히 뜻이 바뀌는 것</b>이다 — 글 ↔ 단계 왕복이 인자를 하나 떨어뜨리거나, 꼴에 맞지 않는
 * 단계가 저장되거나. 값은 모두 지어낸 것이다.
 */

const env: FlowEnv = {
  base: ["계약번호", "계약건명", "계약일자", "계약금액", "납품기한", "담당 부서"],
  items: ["세부품명", "세부품명번호", "수량", "단가", "금액"],
  linked: { notice: { fields: ["입찰공고번호", "기초금액", "납품기한"], items: ["세부품명", "추정단가"] } },
};

const round = (text: string) => {
  const flow = parseFlow(text).flow;
  const printed = printFlow(flow);
  expect(normalizeFlow(parseFlow(printed).flow)).toEqual(normalizeFlow(flow));
  expect(printFlow(parseFlow(printed).flow)).toBe(printed);
  return { flow, printed };
};

describe("글 ↔ 단계 왕복", () => {
  it.each([
    "품목\n  |> 고유 세부품명, 세부품명번호\n  |> 서식 \"{세부품명}({세부품명번호})\"\n  |> 잇기 \", \"",
    "품목 |> 꺼내기 세부품명 |> 고유 |> 줄여쓰기 1 \"{앞} 외 {나머지}종\"",
    "품목 |> 합계 금액 |> 천단위",
    "연결공고.기초금액",
    "연결공고.납품기한 |> 날짜로 |> 날짜차이 계약일자 |> 그대로",
  ])("§4.3 의 예: %s", text => {
    const { flow } = round(text);
    expect(flow.steps.length).toBe(text.split("|>").length - 1);
  });

  /** 동사마다 인자 자리를 모두 채운 단계를 하나씩 둔다 — 하나라도 빠지면 그 동사의 자리가 왕복에서 샌다. */
  const every: Record<string, Step> = {
    filter: { op: "filter", cond: [{ field: "수량", cmp: "≥", value: 2 }, { join: "or", field: "세부품명", cmp: "포함", value: "책" }, { join: "and", field: "금액", cmp: "안비어있음" }] },
    sort: { op: "sort", field: "금액", dir: "desc" },
    distinct: { op: "distinct", fields: ["세부품명", "세부품명번호"] },
    take: { op: "take", n: 2 },
    nth: { op: "nth", n: 3 },
    first: { op: "first" }, last: { op: "last" },
    format: { op: "format", text: "{세부품명} \"따옴표\" \\ 끝" },
    get: { op: "get", field: "담당 부서" },
    join: { op: "join", text: " / " },
    abbrev: { op: "abbrev", n: 2, text: "{앞} 등 {나머지}건" },
    count: { op: "count" },
    sum: { op: "sum", field: "금액" }, min: { op: "min" }, max: { op: "max", field: "단가" }, avg: { op: "avg", field: "수량" },
    trim: { op: "trim" },
    replace: { op: "replace", text: "㈜", to: "" },
    before: { op: "before", text: "(" }, after: { op: "after", text: ")" },
    substr: { op: "substr", n: 1, length: 4 },
    padStart: { op: "padStart", length: 5, text: "0" },
    ifEmpty: { op: "ifEmpty", text: "없음" },
    map: { op: "map", pairs: [["가", "1"], ["나", "2"]], otherwise: "?" },
    if: { op: "if", cond: [{ field: "", cmp: "비어있음" }], then: "없음", else: "있음" },
    cases: { op: "cases", branches: [
      { cond: [{ field: "", cmp: "안비어있음" }], then: "가" },
      { cond: [{ field: "계약금액", cmp: ">", value: 10 }, { join: "or", field: "", cmp: "포함", value: "→" }], then: "\"나\"" },
    ], otherwise: "다" },
    toNumber: { op: "toNumber" },
    add: { op: "add", value: 1.5 }, sub: { op: "sub", field: "계약금액" }, mul: { op: "mul", value: -2 }, div: { op: "div", value: 3 },
    round: { op: "round", n: 2 }, floor: { op: "floor" },
    thousands: { op: "thousands" }, plain: { op: "plain" },
    toDate: { op: "toDate" },
    dateAdd: { op: "dateAdd", n: 3, unit: "month" },
    dateDiff: { op: "dateDiff", field: "계약일자" },
    year: { op: "year" }, month: { op: "month" }, quarter: { op: "quarter" },
    dateFormat: { op: "dateFormat", text: "YYYY년 M월 D일" },
  };

  it("동사가 하나도 빠지지 않았다", () => expect(Object.keys(every).sort()).toEqual(verbs.map(v => v.op).sort()));

  it.each(Object.entries(every))("%s", (_, step) => {
    const flow: Flow = { from: { kind: "items" }, steps: [step] };
    const printed = printFlow(flow);
    expect(parseFlow(printed).flow).toEqual(normalizeFlow(flow));
  });

  it("원천 다섯 가지와 감싼 필드 이름", () => {
    for (const text of ["품목", "연결계약", "연결접수.품목", "[담당 부서]", "\"고정 글\"", "42", "[품목]", "[개수]", "연결공고.[담당 전화]"]) round(text);
    expect(parseFlow("[품목]").flow.from).toEqual({ kind: "field", field: "품목" });
    expect(printFlow({ from: { kind: "field", field: "개수" }, steps: [] })).toBe("[개수]");
  });

  it("-> 와 != 같은 다른 표기는 표준 꼴로 찍힌다", () => {
    expect(printFlow(parseFlow("계약건명 |> 대응 \"a\"->\"b\"").flow)).toBe("계약건명 |> 대응 \"a\"→\"b\"");
    expect(printFlow(parseFlow("품목 |> 거르기 수량 != 0").flow)).toBe("품목 |> 거르기 수량 ≠ 0");
  });
});

describe("구문 오류는 자리를 짚는다", () => {
  const at = (text: string) => {
    try { parseFlow(text); } catch (e) {
      expect(e).toBeInstanceOf(FlowSyntaxError);
      const x = e as FlowSyntaxError;
      return { message: x.message, from: x.from, word: text.slice(x.from, x.to) };
    }
    throw new Error("오류가 나야 합니다");
  };
  it("모르는 동사", () => expect(at("품목 |> 꺼내 세부품명")).toMatchObject({ word: "꺼내", message: "알 수 없는 동사: 꺼내" }));
  it("닫히지 않은 따옴표", () => expect(at("품목 |> 잇기 \", ")).toMatchObject({ from: 9 }));
  it("| 하나", () => expect(at("품목 | 개수")).toMatchObject({ from: 3 }));
  it("남는 인자", () => expect(at("품목 |> 개수 세부품명")).toMatchObject({ word: "세부품명" }));
  it("동사로 시작", () => expect(at("개수")).toMatchObject({ word: "개수", message: "‘개수’ 는 동사입니다 — 맨 앞에는 시작 데이터가 옵니다" }));
  it("빈 |>", () => expect(at("품목 |>").message).toContain("동사"));
  it("단위", () => expect(at("계약일자 |> 날짜로 |> 날짜더하기 3 주").word).toBe("주"));
});

describe("꼴 검사는 오류를 만든 단계에 붙인다", () => {
  const issue = (text: string) => checkFlow(parseFlow(text).flow, env).errors[0];
  it("목록이 아닌 것에 잇기", () => expect(issue("품목 |> 개수 |> 잇기 \", \"")).toMatchObject({ index: 2, message: expect.stringContaining("수에 쓸 수 없습니다") }));
  it("없는 필드", () => expect(issue("품목 |> 꺼내기 품번")).toMatchObject({ index: 1, message: "없는 필드: 품번" }));
  it("서식의 짝 안 맞는 {", () => expect(issue("품목 |> 서식 \"{세부품명\" |> 잇기 \",\"")).toMatchObject({ index: 1 }));
  it("뒤 단계는 꼴을 모른다", () => {
    const c = checkFlow(parseFlow("품목 |> 꺼내기 품번 |> 잇기 \",\"").flow, env);
    expect(c.shapes).toEqual(["rows", null, null]);
  });
  it("끝이 목록이면 저장되지 않는다", () => {
    const c = checkFlow(parseFlow("품목 |> 고유").flow, env);
    expect(c.ok).toBe(false);
    expect(c.errors[0]).toMatchObject({ index: 1, final: true });
    expect(c.shapes).toEqual(["rows", "rows"]);
  });
  it("자기 종류가 아닌 연결만", () => expect(issue("연결계약.계약번호")).toMatchObject({ index: 0 }));
  it("꼴이 맞으면 통과", () => {
    const c = checkFlow(parseFlow("연결공고.납품기한 |> 날짜로 |> 날짜차이 계약일자 |> 그대로").flow, env);
    expect(c).toMatchObject({ ok: true, shapes: ["value", "date", "number", "value"] });
  });
});

/** 값 하나를 견주는 조건의 마디 — 필드가 비면 지금 값, 적히면 「비면」 이 읽는 바깥의 열이다. */
describe("값 조건의 바깥 열", () => {
  const row = { 계약건명: "시험 계약", 계약금액: "2,000,000", 계약일자: "" };
  const run = (text: string) => evaluate(parseFlow(text).flow, env, { row, items: [], linked: {} });

  it("글 꼴은 비면과 같고 왕복이 선다", () => {
    const { flow } = round("계약건명 |> 만약 계약금액 > 1000000 그리고 비어있음 \"큼\" \"작음\"");
    expect(flow.steps[0].cond).toEqual([{ field: "계약금액", cmp: ">", value: 1000000 }, { join: "and", field: "", cmp: "비어있음" }]);
    round("계약건명 |> 만약 [담당 부서] 안비어있음 \"a\" \"b\"");
  });
  it("검사는 비면과 같은 말을 한다", () => {
    expect(checkFlow(parseFlow("계약건명 |> 만약 없는열 비어있음 \"a\" \"b\"").flow, env).errors[0])
      .toMatchObject({ index: 1, message: "이 행에 없는 열: 없는열" });
    expect(checkFlow(parseFlow("계약건명 |> 만약 계약금액 안비어있음 \"a\" \"b\"").flow, env).ok).toBe(true);
  });
  it("평가와 SQL 은 바깥 열을 같은 자리에서 읽는다", () => {
    expect(run("계약건명 |> 만약 계약금액 > 1000000 \"큼\" \"작음\"").final).toBe("큼");
    expect(run("계약건명 |> 만약 계약일자 비어있음 그리고 안비어있음 \"기한 없음\" \"\"").final).toBe("기한 없음");
    const sql = compileFlow(parseFlow("계약건명 |> 만약 계약금액 > 1000000 \"큼\" \"작음\"").flow, env, "contract");
    expect(sql).toContain("현재.계약금액");
    expect(checkSqlMock(sql)).toEqual([]);
  });
});

/** 경우 — 갈래마다 값 조건과 글, 처음 맞는 것이 이기고 맞는 것이 없으면 그외(없으면 빈 값). */
describe("경우", () => {
  const row = { 계약건명: "", 계약금액: "2,000,000", 계약일자: "" };
  const run = (text: string, r: Record<string, string> = row) => evaluate(parseFlow(text).flow, env, { row: r, items: [], linked: {} });

  it("갈래는 쉼표로 잇고 그외 앞의 쉼표·-> 도 읽는다", () => {
    const text = "계약건명 |> 경우 안비어있음 → \"건명\", 계약금액 > 100 → \"금액\", 그외 \"없음\"";
    const { flow } = round(text);
    expect(printFlow(flow)).toBe(text);
    expect(flow.steps[0]).toEqual({ op: "cases", otherwise: "없음", branches: [
      { cond: [{ field: "", cmp: "안비어있음" }], then: "건명" },
      { cond: [{ field: "계약금액", cmp: ">", value: 100 }], then: "금액" }] });
    expect(printFlow(parseFlow("계약건명 |> 경우 안비어있음->\"a\" 그외 \"b\"").flow)).toBe("계약건명 |> 경우 안비어있음 → \"a\", 그외 \"b\"");
    expect(parseFlow("계약건명 |> 경우 비어있음 → \"a\"").flow.steps[0]).toEqual({ op: "cases", branches: [{ cond: [{ field: "", cmp: "비어있음" }], then: "a" }] });
  });
  it("구문 오류는 화살표와 글 자리를 짚는다", () => {
    expect(() => parseFlow("계약건명 |> 경우 비어있음 \"a\"")).toThrow("조건 뒤에 ‘→’ 가 와야 합니다");
    expect(() => parseFlow("계약건명 |> 경우 비어있음 → 계약")).toThrow("‘→’ 뒤에는 따옴표로 감싼 글이 와야 합니다");
    expect(() => parseFlow("계약건명 |> 경우 그외 없음")).toThrow("‘그외’ 뒤에는 따옴표로 감싼 글이 와야 합니다");
  });
  it("검사는 갈래 번호를 붙여 말하고 값에만 쓴다", () => {
    const issue = (text: string) => checkFlow(parseFlow(text).flow, env).errors[0];
    expect(issue("계약건명 |> 경우 그외 \"x\"")).toMatchObject({ index: 1, message: "경우를 하나 이상 적으세요" });
    expect(issue("계약건명 |> 경우 비어있음 → \"a\", 없는열 비어있음 → \"b\"")).toMatchObject({ index: 1, message: "2번째 경우: 이 행에 없는 열: 없는열" });
    expect(issue("품목 |> 경우 비어있음 → \"a\"").message).toContain("행목록에 쓸 수 없습니다");
    const many = { from: { kind: "field", field: "계약건명" }, steps: [{ op: "cases", branches: Array.from({ length: 21 }, () => ({ cond: [{ field: "", cmp: "비어있음" }], then: "" })) }] } as Flow;
    expect(checkFlow(many, env).errors[0].message).toBe("경우는 20개까지입니다");
  });
  it("평가: 처음 맞는 갈래, 없으면 그외, 그외가 없으면 빈 값", () => {
    const text = "계약건명 |> 경우 안비어있음 → \"건명\", 계약금액 > 1000000 → \"큰 금액\", 계약금액 안비어있음 → \"금액\"";
    expect(run(text).final).toBe("큰 금액");
    expect(run(text, { ...row, 계약건명: "시험" }).final).toBe("건명");
    expect(run(text, { ...row, 계약금액: "10" }).final).toBe("금액");
    expect(run(text, { ...row, 계약금액: "" }).final).toBe("");
    expect(run(`${text}, 그외 \"없음\"`, { ...row, 계약금액: "" }).final).toBe("없음");
  });
  it("SQL 은 갈래 차례의 CASE WHEN 이고 그외가 없으면 빈 글이다", () => {
    const sql = compileFlow(parseFlow("계약건명 |> 경우 안비어있음 → \"건명\", 계약금액 > 10 → \"금액\"").flow, env, "contract");
    expect(sql).toContain("CASE WHEN (v <> '') THEN '건명' WHEN (");
    expect(sql).toContain("현재.계약금액");
    expect(sql).toContain("THEN '금액' ELSE '' END");
    expect(checkSqlMock(sql)).toEqual([]);
  });
});

describe("v1 → v2", () => {
  it("pick 은 뒤 단계의 필드로 흡수된다", () => {
    expect(migrateV1({ version: 1, id: "x", steps: [
      { op: "pick", fields: ["세부품명", "세부품명번호"] }, { op: "distinct" },
      { op: "format", text: "{세부품명} · {세부품명번호}" }, { op: "join", text: ", " },
    ] }).flow).toEqual({ from: { kind: "items" }, steps: [
      { op: "distinct", fields: ["세부품명", "세부품명번호"] }, { op: "format", text: "{세부품명} · {세부품명번호}" }, { op: "join", text: ", " },
    ] });
  });
  it("at+get 은 n번째+꺼내기", () => {
    const f = migrateV1({ version: 1, id: "x", steps: [{ op: "pick", fields: ["세부품명"] }, { op: "at", index: 2 }, { op: "get", text: "세부품명" }] });
    expect(printFlow((f as { flow: Flow }).flow)).toBe("품목 |> 2번째 |> 꺼내기 세부품명");
  });
  it("count 와 id 를 지킨다, v2 는 그대로", () => {
    const f = toV2({ version: 1, id: "keep", steps: [{ op: "pick", fields: ["세부품명"] }, { op: "distinct" }, { op: "count" }] });
    expect(f).toMatchObject({ version: 2, id: "keep", source: "flow" });
    expect(toV2(f)).toBe(f);
  });
});

describe("평가(목업)", () => {
  const items = 품목행("notice", 0);
  const notice: FlowEnv = { ...env, items: Object.keys(items[0]) };
  const run = (text: string, input = items) => evaluate(parseFlow(text).flow, notice, { row: { 계약일자: "2026/08/01" }, items: input, linked: {} });

  it("고유는 겹친 책상 한 줄을 빼고, 그 수를 남긴다", () => {
    const r = run("품목 |> 고유 세부품명, 세부품명번호 |> 서식 \"{세부품명}({세부품명번호})\" |> 잇기 \", \"");
    expect(r.values[0]).toMatchObject({ kind: "list", count: 3 });
    expect(r.values[1]).toMatchObject({ kind: "list", count: 2, dropped: 1 });
    expect(r.values[2]).toMatchObject({ kind: "list", items: ["책상(001)", "의자(002)"] });
    expect(r.final).toBe("책상(001), 의자(002)");
  });
  it("번호가 빈 품목은 서식 단계에 빈 값 수로 드러난다", () => {
    const r = run("품목 |> 서식 \"{세부품명}({세부품명번호})\" |> 잇기 \", \"", 품목행("notice", 1));
    expect(r.values[1]).toMatchObject({ items: ["의자()"], blank: 1 });
  });
  it("줄여쓰기·개수·합계·날짜", () => {
    expect(run("품목 |> 꺼내기 세부품명 |> 고유 |> 줄여쓰기 1 \"{앞} 외 {나머지}종\"").final).toBe("책상 외 1종");
    expect(run("품목 |> 개수").final).toBe("3");
    expect(run("품목 |> 합계 수량").final).toBe("10");
    expect(run("\"2026/09/20 10:00\" |> 날짜로 |> 날짜차이 계약일자").final).toBe("50");
    expect(run("\"2026/01/31\" |> 날짜로 |> 날짜더하기 1 개월").final).toBe("2026/03/03");
    expect(run("\"계약 후 60일\" |> 날짜로").final).toBe("");
    expect(run("\"1,234,567.5\" |> 숫자로 |> 버림").final).toBe("1,234,567");
    expect(run("\"10\" |> 숫자로 |> 나누기 4").final).toBe("2.5");
  });
  it("빈 목록은 잇기 '' · 개수 0, 범위 밖 n번째는 ''", () => {
    expect(run("품목 |> 꺼내기 세부품명 |> 잇기 \",\"", []).final).toBe("");
    expect(run("품목 |> 개수", []).final).toBe("0");
    expect(run("품목 |> 9번째 |> 꺼내기 세부품명").final).toBe("");
  });
  it("연결이 없으면 빈 값이지 오류가 아니다", () => {
    const r = evaluate(parseFlow("연결공고.기초금액").flow, env, { row: {}, items: [], linked: { notice: null } });
    expect(r).toMatchObject({ final: "", values: [{ kind: "value", blank: true }] });
  });
});

describe("SQL(시안)", () => {
  it("컴파일된 SQL 은 계약면 뷰만 읽고 바깥 행을 현재로 부른다", () => {
    const sql = compileFlow(parseFlow("품목 |> 꺼내기 세부품명 |> 잇기 \", \"").flow, env, "contract");
    expect(sql).toContain("FROM v_계약품목 WHERE 계약번호 = 현재.계약번호");
    expect(checkSqlMock(sql)).toEqual([]);
  });
  it.each(["꺼내기 세부품명", '서식 "{세부품명}"'])("정렬 후 %s 와 마지막은 순서 열을 보존한다", projection => {
    const sql = compileFlow(parseFlow(`품목 |> 정렬 수량 내림 |> ${projection} |> 마지막`).flow, env, "contract");
    expect(sql).toContain("row_number() OVER (ORDER BY");
    expect(sql).toContain("SELECT __order,");
    expect(sql).toContain("ORDER BY __order DESC LIMIT 1");
    expect(sql).not.toContain("DESC DESC");
    expect(checkSqlMock(sql)).toEqual([]);
  });
  it("음수 날짜 더하기는 유효한 SQLite 날짜 수정자를 낸다", () => {
    const sql = compileFlow(parseFlow('"2026/03/01" |> 날짜로 |> 날짜더하기 -3 일').flow, env, "contract");
    expect(sql).toContain("date(v, '-3 days')");
    expect(sql).not.toContain("+-3");
  });
  it("숫자 출력과 값 가공은 앞 단계 SQL을 반복하지 않는다", () => {
    const sql = compileFlow(parseFlow('계약건명 |> 앞부분 "(" |> 앞부분 "[" |> 숫자로 |> 반올림 2').flow, env, "contract");
    expect(sql.match(/현재\.계약건명/g)).toHaveLength(1);
    expect(sql).toContain("printf('%.15g'");
  });
  it("SQL 들여쓰기는 글자 안의 줄바꿈과 따옴표를 보존한다", () => {
    const sql = compileFlow({ from: { kind: "text", text: "첫줄\n둘째 '문구'" }, steps: [{ op: "trim" }] }, env, "contract");
    expect(sql).toContain("'첫줄\n둘째 ''문구''' AS v");
  });
  it("목업 검사: SELECT 하나, 계약면 뷰만", () => {
    expect(checkSqlMock("DELETE FROM v_계약")[0].message).toContain("SELECT");
    expect(checkSqlMock("SELECT 1; SELECT 2")[0].message).toContain("하나");
    const bad = "SELECT x FROM notice_item";
    expect(checkSqlMock(bad)).toEqual([{ from: 14, to: bad.length, message: "공개 뷰(v_공고 등)만 읽을 수 있습니다: notice_item" }]);
    expect(checkSqlMock("SELECT \"세부품명\" FROM \"v_공고품목\" -- FROM 내부표")).toEqual([]);
  });
});

/**
 * 맞춤 뷰의 글 꼴(docs/user-views.md §3) — 「이 행」 이 없고 문서를 붙여 읽는다. 저장 꼴은 linked 그대로이고 글만 공고.공고명 이다.
 * 오류의 말은 백엔드 Flow 와 한 글자도 다르지 않아야 한다(UserViewTests 가 같은 말을 붙든다).
 */
describe("맞춤 뷰의 글 꼴", () => {
  const view: FlowEnv = {
    base: [], items: [], view: true,
    linked: {
      request: { fields: ["접수번호", "요청명"], items: ["세부품명", "금액"] },
      notice: { fields: ["입찰공고번호", "공고명", "메모"], items: ["세부품명", "추정단가"] },
      contract: { fields: ["계약번호", "계약일자"], items: ["품명", "금액"] },
    },
  };
  const mode = { view: true };

  it("문서 이름으로 읽고 쓰며, 단계가 읽는 행의 열도 문서를 붙인다", () => {
    const text = "\"\" |> 비면 공고.메모 |> 바꾸기 \"-\" \"\"";
    const flow = parseFlow(text, mode).flow;
    expect(flow.from).toEqual({ kind: "text", text: "" });
    expect(flow.steps[0]).toEqual({ op: "ifEmpty", field: "공고.메모" });
    expect(printFlow(flow, mode)).toBe(text);
    expect(parseFlow("공고.품목 |> 개수", mode).flow.from).toEqual({ kind: "linked", link: "notice", items: true });
    expect(parseFlow("계약.계약번호", mode).flow.from).toEqual({ kind: "linked", link: "contract", field: "계약번호" });
    expect(printFlow({ from: { kind: "linked", link: "request", field: "접수번호" }, steps: [] }, mode)).toBe("접수.접수번호");
    // 가공열의 글 꼴은 그대로다.
    expect(printFlow({ from: { kind: "linked", link: "request", field: "접수번호" }, steps: [] })).toBe("연결접수.접수번호");
    expect(() => parseFlow("계약.계약번호")).toThrow(FlowSyntaxError);
  });

  it("「이 행」 의 원천과 문서를 붙이지 않은 행의 열은 문서를 붙이라고 말한다", () => {
    expect(checkFlow({ from: { kind: "field", field: "공고명" }, steps: [] }, view).errors[0].message)
      .toBe("맞춤 뷰에는 「이 행」 이 없습니다. ‘공고.공고명’처럼 문서를 붙여 읽으세요");
    expect(checkFlow({ from: { kind: "items" }, steps: [] }, view).errors[0].message)
      .toBe("맞춤 뷰에는 「이 행」 이 없습니다. ‘공고.품목’처럼 문서를 붙여 읽으세요");
    expect(checkFlow({ from: { kind: "linked", link: "notice", field: "없는열" }, steps: [] }, view).errors[0].message).toBe("공고에 없는 필드: 없는열");
    expect(checkFlow({ from: { kind: "text", text: "" }, steps: [{ op: "ifEmpty", field: "메모" }] }, view).errors[0].message)
      .toBe("문서를 붙여 적으세요 — ‘공고.메모’처럼 읽습니다");
    expect(checkFlow({ from: { kind: "text", text: "" }, steps: [{ op: "ifEmpty", field: "계약.메모" }] }, view).errors[0].message)
      .toBe("계약에 없는 필드: 메모");
    expect(checkFlow({ from: { kind: "text", text: "" }, steps: [{ op: "ifEmpty", field: "공고.메모" }] }, view).ok).toBe(true);
    // 세 문서가 모두 한 건이다 — 계약도 품목을 바로 읽는다.
    expect(checkFlow({ from: { kind: "linked", link: "contract", items: true }, steps: [{ op: "count" }] }, view).ok).toBe(true);
  });

  it("값 조건의 바깥 열은 문서를 붙여 읽고 찍는다", () => {
    const text = "계약.계약번호 |> 만약 안비어있음 또는 공고.[입찰공고번호] = \"x\" \"있음\" \"\"";
    const flow = parseFlow(text, mode).flow;
    expect(flow.steps[0].cond).toEqual([{ field: "", cmp: "안비어있음" }, { join: "or", field: "공고.입찰공고번호", cmp: "=", value: "x" }]);
    expect(printFlow(flow, mode)).toBe("계약.계약번호 |> 만약 안비어있음 또는 공고.입찰공고번호 = \"x\" \"있음\" \"\"");
    expect(checkFlow(flow, view).ok).toBe(true);
    const bare = parseFlow("계약.계약번호 |> 만약 메모 비어있음 \"a\" \"b\"", mode).flow;
    expect(checkFlow(bare, view).errors[0].message).toBe("문서를 붙여 적으세요 — ‘공고.메모’처럼 읽습니다");
    const r = evaluate(parseFlow("\"\" |> 만약 공고.메모 = \"확인함\" \"예\" \"아니오\"", mode).flow, view, { row: { "공고.메모": "확인함" }, items: [], linked: {} });
    expect(r.final).toBe("예");
    expect(compileFlow(flow, view, "contract")).toContain("coalesce(공고.입찰공고번호, '')");
  });

  it("경우는 문서를 붙인 바깥 열로 왕복하고 세 문서의 단계를 가른다", () => {
    const text = "계약.계약번호 |> 경우 안비어있음 → \"계약\", 공고.입찰공고번호 안비어있음 → \"공고\", 그외 \"접수\"";
    const flow = parseFlow(text, mode).flow;
    expect(printFlow(flow, mode)).toBe(text);
    expect(printFlow(parseFlow(printFlow(flow, mode), mode).flow, mode)).toBe(text);
    expect(flow.steps[0].branches?.[1].cond).toEqual([{ field: "공고.입찰공고번호", cmp: "안비어있음" }]);
    expect(checkFlow(flow, view).ok).toBe(true);
    const at = (계약번호: string, 입찰공고번호: string) => evaluate(flow, view, {
      row: { "공고.입찰공고번호": 입찰공고번호 }, items: [],
      linked: { contract: { row: { 계약번호 }, items: [] } },
    }).final;
    expect([at("K1", "N1"), at("", "N1"), at("", "")]).toEqual(["계약", "공고", "접수"]);
    expect(compileFlow(flow, view, "contract")).toContain("WHEN (coalesce(공고.입찰공고번호, '') <> '') THEN '공고' ELSE '접수' END");
  });

  it("서식용 예시의 단계는 경우 한 단계로 찍힌다", () => {
    const column = builtinViews.views[0].definition.columns.find(c => c.name === "단계")!;
    const flow = (column.formula as unknown as { flow: Flow }).flow;
    expect(printFlow(flow, mode)).toBe("계약.계약번호 |> 경우 안비어있음 → \"계약\", 공고.입찰공고번호 안비어있음 → \"공고\", 그외 \"접수\"");
    expect(normalizeFlow(parseFlow(printFlow(flow, mode), mode).flow)).toEqual(normalizeFlow(flow));
  });

  it("평가는 문서마다의 값을 읽고, 꺼낸 SQL 은 바깥 행을 문서 이름으로 부른다", () => {
    const flow = parseFlow("\"\" |> 비면 공고.메모", mode).flow;
    expect(evaluate(flow, view, { row: { "공고.메모": "확인함" }, items: [], linked: {} }).final).toBe("확인함");
    expect(compileFlow(parseFlow("공고.공고명", mode).flow, view, "notice")).toContain("공고.공고명");
    expect(compileFlow(flow, view, "notice")).toContain("공고.메모");
    expect(compileFlow(flow, view, "notice")).not.toContain("현재.");
  });
});
