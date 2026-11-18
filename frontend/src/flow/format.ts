/**
 * 「서식」 의 수 · 날짜 코드 — 엑셀 표시 형식의 부분집합(ADR-053). 꼴은 앞 단계가 정하고, 코드로 꼴을 짐작하지 않는다.
 *
 * <p><b>백엔드 <code>storage/flow_format.rs</code> 와 한 벌이다</b> — 받는 코드 · 오류 글 · 값이 같아야 한다. 공유 사례표가 둘을 함께 돈다.
 * 여기의 <code>formatValue</code> 는 목업 다리의 평가기가, <code>formatSql</code> 은 목업의 SQL 이 쓴다.</p>
 */

export type Piece = { text: string } | { spot: "y4" | "y2" | "m" | "mm" | "d" | "dd" | "h" | "hh" | "n" | "nn" | "s" | "ss" };
export type Code =
  | { kind: "number"; prefix: string; zeros: number; comma: boolean; dot: boolean; decimals: number; suffix: string }
  | { kind: "korean"; suffix: string }
  | { kind: "date"; pieces: Piece[] };

type Tok = { t: "text"; v: string } | { t: "digits"; v: string } | { t: "korean" } | { t: "date"; letter: string; n: number; written: string };

const literal = "$-+/():!^&'~{}<>=";
const maxDigits = 15;
const isAscii = (c: string) => c.codePointAt(0)! < 128;

/** 코드를 해석한다. 지원하지 않으면 오류 글. */
export function parseCode(code: string, date: boolean): Code | string {
  if (!code) return "서식을 적으세요";
  const toks = tokens([...code], date);
  if (typeof toks === "string") return toks;
  return date ? dateCode(toks) : numberCode(toks);
}

function tokens(chars: string[], date: boolean): Tok[] | string {
  const out: Tok[] = [];
  let i = 0;
  while (i < chars.length) {
    const c = chars[i];
    const close = (target: string) => { const at = chars.indexOf(target, i + 1); return at < 0 ? -1 : at; };
    if (c === "\"") {
      const end = close("\"");
      if (end < 0) return "서식의 따옴표가 닫히지 않았습니다";
      out.push({ t: "text", v: chars.slice(i + 1, end).join("") }); i = end + 1;
    } else if (c === "\\") {
      if (i + 1 >= chars.length) return "서식의 ‘\\’ 뒤에 글자가 없습니다";
      out.push({ t: "text", v: chars[i + 1] }); i += 2;
    } else if (c === "[") {
      const end = close("]");
      if (end < 0) return "지원하지 않는 서식 글자: [";
      const inner = chars.slice(i + 1, end).join("");
      if (inner.toLowerCase() !== "dbnum4") return `지원하지 않는 서식: [${inner}]`;
      if (date) return "[DBNum4] 는 수에만 씁니다";
      out.push({ t: "korean" }); i = end + 1;
    } else if (!isAscii(c) || c === " " || literal.includes(c) || (date && (c === "." || c === ","))) {
      out.push({ t: "text", v: c }); i++;
    } else if (!date && "0#,.".includes(c)) {
      let j = i;
      while (j < chars.length && "0#,.".includes(chars[j])) j++;
      out.push({ t: "digits", v: chars.slice(i, j).join("") }); i = j;
    } else if (date && (c === "0" || c === "#")) {
      return "날짜에는 수 자리(0 · #)를 쓸 수 없습니다";
    } else if ("ymdhs".includes(c.toLowerCase())) {
      if (!date) return "수에는 날짜 자리(y · m · d · h · s)를 쓸 수 없습니다";
      const letter = c.toLowerCase();
      let j = i;
      while (j < chars.length && chars[j].toLowerCase() === letter) j++;
      out.push({ t: "date", letter, n: j - i, written: chars.slice(i, j).join("") }); i = j;
    } else {
      return `지원하지 않는 서식 글자: ${c}`;
    }
  }
  return out;
}

function numberCode(toks: Tok[]): Code | string {
  let prefix = "", suffix = "", digits: string | null = null, korean = false;
  for (let i = 0; i < toks.length; i++) {
    const tok = toks[i];
    if (tok.t === "text") { if (digits === null && !korean) prefix += tok.v; else suffix += tok.v; }
    else if (tok.t === "korean") { if (i > 0) return "[DBNum4] 는 서식 맨 앞에만 씁니다"; korean = true; }
    else if (tok.t === "digits") {
      if (korean) return "[DBNum4] 뒤에는 글자만 씁니다";
      if (digits !== null) return "숫자 자리는 한 덩이로 씁니다";
      digits = tok.v;
    }
  }
  if (korean) return { kind: "korean", suffix };
  if (digits === null) return "숫자 자리(0)가 없습니다";
  const parts = digits.split(".");
  if (parts.length > 2) return "소수점은 하나만 씁니다";
  const [whole, fraction] = parts;
  if (fraction !== undefined && /[^0]/.test(fraction)) return "소수 자리에는 0 만 씁니다";
  if (whole.endsWith(",")) return "숫자 자리 끝의 쉼표는 쓸 수 없습니다";
  const zeros = [...whole].filter(c => c === "0").length;
  if (!zeros) return "정수 자리에 0 이 하나 이상 있어야 합니다";
  const decimals = fraction?.length ?? 0;
  if (zeros > maxDigits || decimals > maxDigits) return `자릿수는 ${maxDigits}개까지입니다`;
  return { kind: "number", prefix, zeros, comma: whole.includes(","), dot: fraction !== undefined, decimals, suffix };
}

function dateCode(toks: Tok[]): Code | string {
  const pieces: Piece[] = [];
  for (const tok of toks) {
    if (tok.t === "text") {
      const last = pieces.at(-1);
      if (last && "text" in last) last.text += tok.v; else pieces.push({ text: tok.v });
      continue;
    }
    if (tok.t !== "date") continue;
    const spot = ({ y4: "y4", y2: "y2", m1: "m", m2: "mm", d1: "d", d2: "dd", h1: "h", h2: "hh", s1: "s", s2: "ss" } as const)[`${tok.letter}${tok.n}` as "y4"];
    if (!spot) return `지원하지 않는 날짜 자리: ${tok.written}`;
    pieces.push({ spot });
  }
  // 엑셀처럼: 글자를 건너뛴 바로 앞 자리가 시이거나 바로 뒤 자리가 초면 m 은 분이다.
  const spots = pieces.filter((p): p is Extract<Piece, { spot: string }> => "spot" in p);
  spots.forEach((p, k) => {
    const afterHour = k > 0 && (spots[k - 1].spot === "h" || spots[k - 1].spot === "hh");
    const next = spots[k + 1]?.spot;
    if ((afterHour || next === "s" || next === "ss") && (p.spot === "m" || p.spot === "mm")) p.spot = p.spot === "m" ? "n" : "nn";
  });
  return { kind: "date", pieces };
}

/** 값 하나를 코드대로 글로. 수는 <code>number</code>, 날짜는 <code>YYYY-MM-DD[ HH:MM:SS]</code>. 값이 없으면 빈 글. */
export function formatValue(code: Code, value: number | string | null): string {
  if (value === null) return "";
  if (code.kind === "korean") return (koreanAmount(String(Math.trunc(value as number))) ?? "") + code.suffix;
  if (code.kind === "number") {
    const n = value as number, p = 10 ** code.decimals;
    // 사사오입(0 에서 먼 쪽) — SQL 의 CAST(abs(v) * p + 0.5 AS INTEGER) 와 같은 셈.
    const r = Math.floor(Math.abs(n) * p + 0.5);
    let digits = String(code.decimals ? Math.floor(r / p) : r).padStart(code.zeros, "0");
    if (code.comma) digits = digits.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
    const fraction = code.decimals ? `.${String(r % p).padStart(code.decimals, "0")}` : code.dot ? "." : "";
    return (n < 0 && r > 0 ? "-" : "") + code.prefix + digits + fraction + code.suffix;
  }
  const d = value as string, t = `${d} 00:00:00`;
  const bare = (two: string) => String(Number(two));
  return code.pieces.map(p => {
    if ("text" in p) return p.text;
    switch (p.spot) {
      case "y4": return d.slice(0, 4);
      case "y2": return d.slice(2, 4);
      case "mm": return d.slice(5, 7);
      case "m": return bare(d.slice(5, 7));
      case "dd": return d.slice(8, 10);
      case "d": return bare(d.slice(8, 10));
      case "hh": return t.slice(11, 13);
      case "h": return bare(t.slice(11, 13));
      case "nn": return t.slice(14, 16);
      case "n": return bare(t.slice(14, 16));
      case "ss": return t.slice(17, 19);
      case "s": return bare(t.slice(17, 19));
    }
  }).join("");
}

const sqlLit = (s: string) => `'${s.replace(/'/g, "''")}'`;

/** 값 하나 <code>v</code> 위의 SQL 식(목업의 SQL 이 쓴다 — 백엔드 flow_format::sql 과 같은 글). */
export function formatSql(code: Code): string {
  let body: string;
  if (code.kind === "korean") body = `한글금액(CAST(CAST(v AS INTEGER) AS TEXT))${code.suffix ? ` || ${sqlLit(code.suffix)}` : ""}`;
  else if (code.kind === "number") {
    const p = 10 ** code.decimals, int = code.decimals ? `r / ${p}` : "r";
    const top = 10 ** code.zeros, drop = code.zeros % 3 === 0 ? 3 : 2;
    const digits = code.comma && code.zeros > 1 ? `CASE WHEN ${int} < ${top} THEN substr(printf('%,d', ${int} + ${top}), ${drop}) ELSE printf('%,d', ${int}) END`
      : code.comma ? `printf('%,d', ${int})` : `printf('%0${code.zeros}d', ${int})`;
    const parts = ["CASE WHEN v < 0 AND r > 0 THEN '-' ELSE '' END", ...(code.prefix ? [sqlLit(code.prefix)] : []), digits,
      ...(code.decimals ? [`'.' || printf('%0${code.decimals}d', r % ${p})`] : code.dot ? ["'.'"] : []), ...(code.suffix ? [sqlLit(code.suffix)] : [])];
    body = `(SELECT ${parts.join(" || ")} FROM (SELECT CAST(${p === 1 ? "abs(v)" : `abs(v) * ${p}`} + 0.5 AS INTEGER) AS r))`;
  } else {
    const at = (from: number) => `substr(v || ' 00:00:00', ${from}, 2)`;
    const bare = (two: string) => `CAST(CAST(${two} AS INTEGER) AS TEXT)`;
    const sql: Record<Extract<Piece, { spot: string }>["spot"], string> = {
      y4: "substr(v, 1, 4)", y2: "substr(v, 3, 2)", mm: "substr(v, 6, 2)", m: bare("substr(v, 6, 2)"), dd: "substr(v, 9, 2)", d: bare("substr(v, 9, 2)"),
      hh: at(12), h: bare(at(12)), nn: at(15), n: bare(at(15)), ss: at(18), s: bare(at(18)),
    };
    body = code.pieces.map(p => "text" in p ? sqlLit(p.text) : sql[p.spot]).join(" || ") || "''";
  }
  return `CASE WHEN v IS NULL THEN '' ELSE ${body} END`;
}

const DIGITS = ["영", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"];
const SMALL = ["", "십", "백", "천"];
const LARGE = ["", "만", "억", "조", "경"];

/** 정수 부분을 한글로(114,500 → 일십일만사천오백) — 백엔드 <code>korean::amount</code> 와 같다. 숫자가 아니면 null. */
export function koreanAmount(text: string): string | null {
  const plain = text.trim().replace(/,/g, "");
  const negative = plain.startsWith("-");
  const whole0 = (negative ? plain.slice(1) : plain).split(".")[0];
  if (!whole0 || !/^\d+$/.test(whole0)) return null;
  const whole = whole0.replace(/^0+/, "");
  if (!whole) return "영";
  if (whole.length > 20) return null;
  const digits = [...whole].map(Number);
  let out = "";
  const groups = Math.ceil(digits.length / 4);
  for (let g = groups - 1; g >= 0; g--) {
    const end = digits.length - g * 4, chunk = digits.slice(Math.max(0, end - 4), end);
    let part = "";
    chunk.forEach((d, i) => { if (d) part += DIGITS[d] + SMALL[chunk.length - 1 - i]; });
    if (part) out += part + LARGE[g];
  }
  return negative ? `마이너스 ${out}` : out;
}
