import type { Op, Shape, Step } from "./ast";

/**
 * 동사 목록. 파서·표준 출력·꼴 검사·「+ 단계」 고르개가 모두 이 한 벌을 본다 — 두 벌이면 글에서는 되는
 * 동사가 고르개에는 없는 일이 생긴다.
 */
export type Group = "목록" | "셈" | "글" | "수" | "날짜";
export const groups: Group[] = ["목록", "셈", "글", "수", "날짜"];

export type Arg =
  | { k: "cond" }
  /** 행 필드(row) 또는 이 행의 기본 열(base). */
  | { k: "field"; scope: "row" | "base"; optional?: boolean; label: string }
  | { k: "fields"; label: string }
  | { k: "dir" }
  | { k: "str"; slot: "text" | "to" | "then" | "else"; label: string }
  | { k: "int"; slot: "n" | "length"; optional?: boolean; label: string }
  | { k: "unit" }
  | { k: "pairs" }
  /** 「경우」 의 갈래들과 그외. */
  | { k: "branches" }
  | { k: "numOrField"; label: string }
  | { k: "strOrField"; label: string }
  /** 날짜차이 끝의 「일」. 읽기만 하고 찍지 않는다. */
  | { k: "dayWord" };

export type Verb = {
  op: Op;
  word: string;
  group: Group;
  /** 들어감 → 나옴. */
  io: [Shape, Shape][];
  args: Arg[];
  desc: string;
};

const L = (a: Shape, b: Shape): [Shape, Shape] => [a, b];

export const verbs: Verb[] = [
  { op: "filter", word: "거르기", group: "목록", io: [L("rows", "rows")], args: [{ k: "cond" }], desc: "조건에 맞는 행만 남깁니다" },
  { op: "sort", word: "정렬", group: "목록", io: [L("rows", "rows"), L("values", "values")], args: [{ k: "field", scope: "row", optional: true, label: "기준 열" }, { k: "dir" }], desc: "열 값 순서로 다시 늘어놓습니다. 열을 고르지 않으면 원래 순서" },
  { op: "distinct", word: "고유", group: "목록", io: [L("rows", "rows"), L("values", "values")], args: [{ k: "fields", label: "기준 열" }], desc: "기준이 같은 행은 처음 것만 남깁니다" },
  { op: "take", word: "앞에서", group: "목록", io: [L("rows", "rows"), L("values", "values")], args: [{ k: "int", slot: "n", label: "개수" }], desc: "앞의 n개만 남깁니다" },
  { op: "nth", word: "번째", group: "목록", io: [L("rows", "row"), L("values", "value")], args: [{ k: "int", slot: "n", label: "몇 번째" }], desc: "n번째 하나를 꺼냅니다. 없으면 빈 값" },
  { op: "first", word: "처음", group: "목록", io: [L("rows", "row"), L("values", "value")], args: [], desc: "맨 앞 하나를 꺼냅니다" },
  { op: "last", word: "마지막", group: "목록", io: [L("rows", "row"), L("values", "value")], args: [], desc: "맨 뒤 하나를 꺼냅니다" },
  { op: "format", word: "서식", group: "목록", io: [L("rows", "values"), L("row", "value")], args: [{ k: "str", slot: "text", label: "서식" }], desc: "{필드} 자리에 값을 넣어 글로 만듭니다" },
  { op: "get", word: "꺼내기", group: "목록", io: [L("rows", "values"), L("row", "value")], args: [{ k: "field", scope: "row", label: "열" }], desc: "열 하나의 값만 꺼냅니다" },
  { op: "join", word: "잇기", group: "목록", io: [L("values", "value")], args: [{ k: "str", slot: "text", label: "구분자" }], desc: "값들을 구분자로 이어 한 칸에 씁니다" },
  { op: "abbrev", word: "줄여쓰기", group: "목록", io: [L("values", "value")], args: [{ k: "int", slot: "n", label: "보일 개수" }, { k: "str", slot: "text", label: "넘칠 때 꼴" }], desc: "n개까지는 잇고, 넘으면 「{앞} 외 {나머지}종」 꼴로 줄입니다" },
  { op: "count", word: "개수", group: "셈", io: [L("rows", "number"), L("values", "number")], args: [], desc: "몇 개인지 셉니다. 비었으면 0" },
  { op: "sum", word: "합계", group: "셈", io: [L("rows", "number"), L("values", "number")], args: [{ k: "field", scope: "row", optional: true, label: "열" }], desc: "숫자로 읽어 더합니다. 쉼표는 무시합니다" },
  { op: "min", word: "최소", group: "셈", io: [L("rows", "number"), L("values", "number")], args: [{ k: "field", scope: "row", optional: true, label: "열" }], desc: "가장 작은 수를 고릅니다" },
  { op: "max", word: "최대", group: "셈", io: [L("rows", "number"), L("values", "number")], args: [{ k: "field", scope: "row", optional: true, label: "열" }], desc: "가장 큰 수를 고릅니다" },
  { op: "avg", word: "평균", group: "셈", io: [L("rows", "number"), L("values", "number")], args: [{ k: "field", scope: "row", optional: true, label: "열" }], desc: "평균을 냅니다. 빈 값은 빼고 셉니다" },
  { op: "trim", word: "다듬기", group: "글", io: [L("value", "value")], args: [], desc: "앞뒤 공백을 지웁니다" },
  { op: "replace", word: "바꾸기", group: "글", io: [L("value", "value")], args: [{ k: "str", slot: "text", label: "찾을 글" }, { k: "str", slot: "to", label: "바꿀 글" }], desc: "글을 찾아 모두 바꿉니다" },
  { op: "before", word: "앞부분", group: "글", io: [L("value", "value")], args: [{ k: "str", slot: "text", label: "구분자" }], desc: "구분자 앞까지. 구분자가 없으면 전체" },
  { op: "after", word: "뒷부분", group: "글", io: [L("value", "value")], args: [{ k: "str", slot: "text", label: "구분자" }], desc: "구분자 뒤부터. 구분자가 없으면 빈 값" },
  { op: "substr", word: "자르기", group: "글", io: [L("value", "value")], args: [{ k: "int", slot: "n", label: "시작(1부터)" }, { k: "int", slot: "length", label: "길이" }], desc: "시작 글자부터 길이만큼 자릅니다" },
  { op: "padStart", word: "앞채우기", group: "글", io: [L("value", "value")], args: [{ k: "int", slot: "length", label: "길이" }, { k: "str", slot: "text", label: "채울 글자" }], desc: "길이가 될 때까지 앞을 채웁니다" },
  { op: "ifEmpty", word: "비면", group: "글", io: [L("value", "value")], args: [{ k: "strOrField", label: "대신할 값" }], desc: "빈 값이면 글이나 이 행의 열로 대신합니다" },
  { op: "map", word: "대응", group: "글", io: [L("value", "value")], args: [{ k: "pairs" }], desc: "정해 둔 값으로 바꿉니다. 목록에 없으면 그외, 그외가 없으면 그대로" },
  { op: "if", word: "만약", group: "글", io: [L("row", "value"), L("value", "value")], args: [{ k: "cond" }, { k: "str", slot: "then", label: "맞으면" }, { k: "str", slot: "else", label: "아니면" }], desc: "조건에 따라 두 글 중 하나를 씁니다" },
  { op: "cases", word: "경우", group: "글", io: [L("value", "value")], args: [{ k: "branches" }], desc: "조건을 차례로 견주어 처음 맞는 경우의 글을 씁니다. 맞는 것이 없으면 그외, 그외가 없으면 빈 값" },
  { op: "toNumber", word: "숫자로", group: "수", io: [L("value", "number")], args: [], desc: "쉼표를 빼고 수로 읽습니다. 못 읽으면 빈 값" },
  { op: "add", word: "더하기", group: "수", io: [L("number", "number")], args: [{ k: "numOrField", label: "더할 것" }], desc: "수나 이 행의 열을 더합니다" },
  { op: "sub", word: "빼기", group: "수", io: [L("number", "number")], args: [{ k: "numOrField", label: "뺄 것" }], desc: "수나 이 행의 열을 뺍니다" },
  { op: "mul", word: "곱하기", group: "수", io: [L("number", "number")], args: [{ k: "numOrField", label: "곱할 것" }], desc: "수나 이 행의 열을 곱합니다" },
  { op: "div", word: "나누기", group: "수", io: [L("number", "number")], args: [{ k: "numOrField", label: "나눌 것" }], desc: "나눕니다. 결과는 소수일 수 있습니다" },
  { op: "round", word: "반올림", group: "수", io: [L("number", "number")], args: [{ k: "int", slot: "n", optional: true, label: "소수 자리" }], desc: "자리에서 반올림합니다. 기본은 정수" },
  { op: "floor", word: "버림", group: "수", io: [L("number", "number")], args: [{ k: "int", slot: "n", optional: true, label: "소수 자리" }], desc: "자리 아래를 버립니다. 기본은 정수" },
  { op: "thousands", word: "천단위", group: "수", io: [L("number", "value")], args: [], desc: "164,872,340처럼 쉼표를 넣은 정수 글로 바꿉니다" },
  { op: "plain", word: "그대로", group: "수", io: [L("number", "value")], args: [], desc: "쉼표 없는 숫자 글로 바꿉니다" },
  { op: "toDate", word: "날짜로", group: "날짜", io: [L("value", "date")], args: [], desc: "2026/09/20이나 2026-09-20 꼴을 날짜로 읽습니다. 뒤의 시각은 무시합니다" },
  { op: "dateAdd", word: "날짜더하기", group: "날짜", io: [L("date", "date")], args: [{ k: "int", slot: "n", label: "얼마나" }, { k: "unit" }], desc: "일·개월·년을 더합니다" },
  { op: "dateDiff", word: "날짜차이", group: "날짜", io: [L("date", "number")], args: [{ k: "field", scope: "base", label: "뺄 날짜 열" }, { k: "dayWord" }], desc: "이 행의 날짜 열과 며칠 차이인지 셉니다" },
  { op: "year", word: "연도", group: "날짜", io: [L("date", "number")], args: [], desc: "연도를 수로 꺼냅니다" },
  { op: "month", word: "월", group: "날짜", io: [L("date", "number")], args: [], desc: "월을 수로 꺼냅니다" },
  { op: "quarter", word: "분기", group: "날짜", io: [L("date", "number")], args: [], desc: "분기(1–4)를 수로 꺼냅니다" },
  { op: "dateFormat", word: "날짜서식", group: "날짜", io: [L("date", "value")], args: [{ k: "str", slot: "text", label: "서식" }], desc: "YYYY·MM·M·DD·D 자리에 날짜를 넣어 글로 만듭니다" },
];

export const verbByOp = new Map(verbs.map(v => [v.op, v]));
/** 「n번째」 는 수와 붙어 쓰므로 낱말로 찾지 않는다. */
export const verbByWord = new Map(verbs.filter(v => v.op !== "nth").map(v => [v.word, v]));

/** 이 꼴에서 쓸 수 있는가, 쓰면 무엇이 나오는가. */
export function output(op: Op, input: Shape): Shape | null {
  return verbByOp.get(op)?.io.find(([a]) => a === input)?.[1] ?? null;
}

export const verbsFor = (input: Shape) => verbs.filter(v => v.io.some(([a]) => a === input));

/** 표준 출력에서 낱말이 아니라 문법으로 읽히는 말. 필드 이름이 이것이면 [ ] 로 감싼다. */
export const reserved = new Set([
  ...verbs.map(v => v.word), "품목", "연결공고", "연결접수", "연결계약",
  "오름", "내림", "그리고", "또는", "포함", "비어있음", "안비어있음", "그외", "일", "개월", "년",
]);

/** 고르개가 새 단계를 넣을 때의 기본 인자. 필드는 지금 행의 것에서 고른다. */
export function defaultStep(op: Op, rowFields: string[], baseFields: string[]): Step {
  const f = rowFields[0] ?? "";
  switch (op) {
    case "filter": return { op, cond: [{ field: f, cmp: "안비어있음" }] };
    case "sort": return { op };
    case "distinct": return { op };
    case "take": return { op, n: 3 };
    case "nth": return { op, n: 1 };
    case "format": return { op, text: rowFields.length ? rowFields.slice(0, 2).map(x => `{${x}}`).join(" ") : "" };
    case "get": return { op, field: f };
    case "join": return { op, text: ", " };
    case "abbrev": return { op, n: 1, text: "{앞} 외 {나머지}종" };
    case "sum": case "min": case "max": case "avg":
      return rowFields.length ? { op, field: rowFields.find(x => /금액|단가|수량/.test(x)) ?? f } : { op };
    case "replace": return { op, text: "", to: "" };
    case "before": case "after": return { op, text: " " };
    case "substr": return { op, n: 1, length: 4 };
    case "padStart": return { op, length: 3, text: "0" };
    case "ifEmpty": return { op, text: "없음" };
    case "map": return { op, pairs: [["", ""]] };
    case "if": return { op, cond: [{ field: rowFields.length ? f : "", cmp: "비어있음" }], then: "", else: "" };
    case "cases": return { op, branches: [{ cond: [{ field: "", cmp: "안비어있음" }], then: "" }] };
    case "add": case "sub": case "mul": case "div": return { op, value: 1 };
    case "dateAdd": return { op, n: 30, unit: "day" };
    case "dateDiff": return { op, field: baseFields.find(x => /일자|기한|일시/.test(x)) ?? baseFields[0] ?? "" };
    case "dateFormat": return { op, text: "YYYY년 M월 D일" };
    default: return { op };
  }
}
