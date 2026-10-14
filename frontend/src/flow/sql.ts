import type { Clause, Flow, FlowEnv, LinkKind, Step } from "./ast";
import { docWord, qualified } from "./ast";
import { checkFlow } from "./check";
import { toNumber } from "./evaluate";

/**
 * 흐름식 → SQL. <b>목업 다리 전용</b>이다: 앱에서는 백엔드 컴파일러(FlowCompiler.Eject)가 SQL 을 내고 화면은 그 결과를 받아 찍는다.
 * 목업 다리가 그럴듯한 SQL 을 내려고 쓴다. 바깥 행은 별칭 <code>현재</code>(§5).
 */
export type EntityKind = LinkKind;

/** 계약면 뷰. SQL 탈출구가 읽을 수 있는 것은 이것뿐이다(§5). */
export const contractViews = ["v_통합", "v_통합차수", "v_접수", "v_접수품목", "v_공고", "v_공고품목", "v_공고차수", "v_계약", "v_계약차수", "v_계약품목"];

export const entityViews: Record<EntityKind, { view: string; key: string; items: string }> = {
  contract: { view: "v_계약", key: "계약번호", items: "v_계약품목" },
  notice: { view: "v_공고", key: "입찰공고번호", items: "v_공고품목" },
  request: { view: "v_접수", key: "접수번호", items: "v_접수품목" },
};

/** 유니코드 낱말 글자면 그대로, 아니면 큰따옴표. */
export const sqlIdent = (name: string) => /^[\p{L}_][\p{L}\p{N}_]*$/u.test(name) ? name : `"${name.replace(/"/g, "\"\"")}"`;
const lit = (s: string) => `'${s.replace(/'/g, "''")}'`;
// C# FlowCompiler 와 같은 숫자·공백 규칙. CAST('abc' AS REAL) 이 0 이 되는 것을 막는다.
const spaces = "char(9,10,11,12,13,32,160,5760,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8232,8233,8239,8287,12288,65279)";
const num = (x: string) => {
  const t = `trim(replace(${x}, ',', ''), ${spaces})`;
  const u = `(CASE WHEN ${t} GLOB '[+-]*' THEN substr(${t}, 2) ELSE ${t} END)`;
  return `(CASE WHEN ${u} <> '' AND ${u} <> '.' AND ${u} NOT GLOB '*[^0-9.]*' AND ${u} NOT GLOB '*.*.*' THEN CAST(${t} AS REAL) END)`;
};
const date = (x: string) => `(SELECT CASE WHEN t GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(t) = t THEN t END FROM (SELECT replace(substr(${x}, 1, 10), '/', '-') AS t))`;
// 들여쓰기는 SQL 글자·식별자 안의 줄바꿈을 바꾸지 않는다.
const indent = (s: string) => s.replace(/'(?:[^']|'')*'|"(?:[^"]|"")*"|\n/g, part => part === "\n" ? "\n  " : part);
const from = (sql: string) => `FROM (\n  ${indent(sql)}\n)`;
const columns = (fields: string[]) => fields.map(sqlIdent).join(", ");

type S =
  | { k: "rel"; sql: string; values: boolean; fields: string[] }
  | { k: "one"; sql: string; fields: string[] }
  | { k: "expr"; sql: string; shape: "value" | "number" | "date" };

function cond(clauses: Clause[], subject: (field: string) => string): string {
  return "(" + clauses.map((c, i) => {
    const x = subject(c.field), text = String(c.value ?? "");
    const number = typeof c.value === "number" ? c.value : toNumber(text);
    const op = ({ "=": "=", "≠": "<>", ">": ">", "<": "<", "≥": ">=", "≤": "<=" } as Partial<Record<Clause["cmp"], string>>)[c.cmp];
    const body = c.cmp === "비어있음" ? `${x} = ''` : c.cmp === "안비어있음" ? `${x} <> ''`
      : c.cmp === "포함" ? `instr(${x}, ${lit(text)}) > 0`
      : number === null ? `${x} ${op} ${lit(text)}`
      : `(CASE WHEN ${num(x)} IS NOT NULL THEN ${num(x)} ${op} ${number} ELSE ${x} ${op} ${lit(text)} END)`;
    return (i ? c.join === "or" ? " OR " : " AND " : "") + body;
  }).join("") + ")";
}

function template(text: string, field: (f: string) => string): string {
  const out: string[] = [];
  let end = 0;
  for (const m of text.matchAll(/\{([^{}]+)\}/g)) {
    if (m.index! > end) out.push(lit(text.slice(end, m.index)));
    out.push(field(m[1]));
    end = m.index! + m[0].length;
  }
  if (end < text.length) out.push(lit(text.slice(end)));
  return out.join(" || ") || "''";
}

export function compileFlow(flow: Flow, env: FlowEnv, entity: EntityKind): string {
  const checked = checkFlow(flow, env);
  if (!checked.ok) return `-- 식에 오류가 있어 SQL을 만들지 않았습니다: ${checked.errors[0].message}`;
  const self = entityViews[entity], f = flow.from;
  const outer = (field: string) => {
    const q = env.view ? qualified(field) : null;
    return q ? `coalesce(${docWord[q.link]}.${sqlIdent(q.column)}, '')` : `현재.${sqlIdent(field)}`;
  };
  const items = (kind: LinkKind, key: string): S => ({ k: "rel", values: false, fields: checked.fields[0],
    sql: `SELECT 순번 AS __order, ${columns(checked.fields[0])} FROM ${entityViews[kind].items} WHERE ${sqlIdent(entityViews[kind].key)} = ${key}` });
  let s: S;
  switch (f.kind) {
    case "items": s = items(entity, `현재.${sqlIdent(self.key)}`); break;
    case "field": s = { k: "expr", sql: outer(f.field), shape: "value" }; break;
    case "text": s = { k: "expr", sql: lit(f.text), shape: "value" }; break;
    case "number": s = { k: "expr", sql: String(f.value), shape: "number" }; break;
    case "linked": {
      const o = entityViews[f.link], doc = docWord[f.link];
      if (env.view) {
        s = f.items ? items(f.link, `${doc}.${sqlIdent(o.key)}`)
          : f.field !== undefined ? { k: "expr", sql: `coalesce(${doc}.${sqlIdent(f.field)}, '')`, shape: "value" }
          : { k: "one", fields: checked.fields[0], sql: `SELECT 0 AS __order, ${checked.fields[0].map(field => `coalesce(${doc}.${sqlIdent(field)}, '') AS ${sqlIdent(field)}`).join(", ")} WHERE ${doc}.${sqlIdent(o.key)} IS NOT NULL` };
        break;
      }
      const unified = entity === "notice" ? "v_통합차수" : "v_통합";
      const key = `(SELECT ${sqlIdent(o.key)} FROM ${unified} WHERE ${sqlIdent(self.key)} = 현재.${sqlIdent(self.key)} AND ${sqlIdent(o.key)} <> '' LIMIT 1)`;
      const many = env.linked[f.link]?.many;
      const fields = f.field === undefined ? checked.fields[0] : [f.field];
      const rows = many
        ? `SELECT 계약본번호 AS __order, ${columns(fields)} FROM ${o.view} WHERE ${sqlIdent(o.key)} IN (SELECT ${sqlIdent(o.key)} FROM ${unified} WHERE ${sqlIdent(self.key)} = 현재.${sqlIdent(self.key)})`
        : `SELECT 0 AS __order, ${columns(fields)} FROM ${o.view} WHERE ${sqlIdent(o.key)} = ${key}`;
      s = f.items ? items(f.link, key)
        : f.field !== undefined ? { k: "expr", sql: `coalesce((SELECT ${sqlIdent(f.field)} ${from(rows)}), '')`, shape: "value" }
        : many ? { k: "rel", values: false, fields, sql: rows } : { k: "one", fields, sql: rows };
    }
  }
  for (const st of flow.steps) s = step(st, s, outer);
  return finish(s);
}

function finish(s: S): string {
  const e = s as Extract<S, { k: "expr" }>;
  const out = e.shape === "number" ? `(SELECT CASE WHEN v IS NULL THEN '' ELSE (SELECT CASE WHEN c = CAST(c AS INTEGER) THEN printf('%,d', CAST(c AS INTEGER)) ELSE printf('%.15g', c) END FROM (SELECT CAST(printf('%.15g', v) AS REAL) AS c)) END ${from(`SELECT ${e.sql} AS v`)})`
    : e.shape === "date" ? `replace(${e.sql}, '-', '/')` : e.sql;
  return `SELECT\n  coalesce(${indent(out)}, '') AS 결과`;
}

function step(st: Step, s: S, outer: (field: string) => string): S {
  const col = sqlIdent;
  const e = (sql: string, shape: "value" | "number" | "date" = "value"): S => ({ k: "expr", sql, shape });
  if (s.k === "rel") {
    const r = s, cols = r.values ? "__value" : columns(r.fields);
    switch (st.op) {
      case "filter": return { ...r, sql: `SELECT * ${from(r.sql)}\nWHERE ${cond(st.cond ?? [], col)}` };
      case "sort": {
        const desc = st.dir === "desc" ? " DESC" : "";
        if (!r.values && st.field === undefined) return desc ? { ...r, sql: `SELECT row_number() OVER (ORDER BY __order DESC) AS __order, ${cols} ${from(r.sql)}` } : r;
        const key = r.values ? "__value" : col(st.field!);
        return { ...r, sql: `SELECT row_number() OVER (ORDER BY CASE WHEN __a = 1 THEN __k END${desc}, CASE WHEN __a = 1 THEN NULL ELSE __t END${desc}, __order) AS __order, ${cols}\n${from(`SELECT *, ${num(key)} AS __k, ${key} AS __t, min(CASE WHEN ${num(key)} IS NULL THEN 0 ELSE 1 END) OVER () AS __a ${from(r.sql)}`)}` };
      }
      case "distinct": {
        const keys = r.values ? "__value" : columns(st.fields?.length ? st.fields : r.fields);
        return { ...r, sql: `SELECT __order, ${cols} ${from(`SELECT *, row_number() OVER (PARTITION BY ${keys} ORDER BY __order) AS __rn ${from(r.sql)}`)}\nWHERE __rn = 1` };
      }
      case "take": return { ...r, sql: `SELECT * ${from(r.sql)}\nORDER BY __order LIMIT ${st.n}` };
      case "nth": case "first": case "last": {
        const pick = st.op === "last" ? "ORDER BY __order DESC LIMIT 1" : `ORDER BY __order LIMIT 1 OFFSET ${st.op === "nth" ? (st.n ?? 1) - 1 : 0}`;
        return r.values ? e(`coalesce((SELECT __value ${from(r.sql)} ${pick}), '')`)
          : { k: "one", fields: r.fields, sql: `SELECT * ${from(r.sql)}\n${pick}` };
      }
      case "format": case "get": return { k: "rel", values: true, fields: [], sql: `SELECT __order, ${st.op === "format" ? template(st.text ?? "", col) : col(st.field!)} AS __value ${from(r.sql)}` };
      case "join": return e(`coalesce((SELECT group_concat(__value, ${lit(st.text ?? "")} ORDER BY __order) ${from(r.sql)}), '')`);
      case "abbrev": {
        const n = st.n ?? 1;
        return e(`coalesce((SELECT CASE WHEN count(*) <= ${n} THEN group_concat(__value, ', ' ORDER BY __order) ELSE replace(replace(${lit(st.text ?? "")}, '{앞}', group_concat(CASE WHEN __rn <= ${n} THEN __value END, ', ' ORDER BY __order)), '{나머지}', CAST(count(*) - ${n} AS TEXT)) END ${from(`SELECT __order, __value, row_number() OVER (ORDER BY __order) AS __rn ${from(r.sql)}`)}), '')`);
      }
      case "count": return e(`(SELECT count(*) ${from(r.sql)})`, "number");
      case "sum": case "min": case "max": case "avg": return e(`(SELECT ${st.op}(${num(r.values ? "__value" : col(st.field!))}) ${from(r.sql)})`, "number");
    }
  }
  if (s.k === "one") {
    if (st.op === "format") return e(`coalesce((SELECT ${template(st.text ?? "", col)} ${from(s.sql)}), '')`);
    if (st.op === "get") return e(`coalesce((SELECT ${col(st.field!)} ${from(s.sql)}), '')`);
    if (st.op === "if") return e(`coalesce((SELECT CASE WHEN ${cond(st.cond ?? [], col)} THEN ${lit(st.then ?? "")} ELSE ${lit(st.else ?? "")} END ${from(s.sql)}), ${lit(st.else ?? "")})`);
  }
  const x = (s as Extract<S, { k: "expr" }>).sql;
  // 앞 단계는 한 번만 읽어 긴 식에서도 SQL 크기가 단계 수에 비례한다.
  const over = (body: string, shape: "value" | "number" | "date" = "value") => e(`(SELECT ${body} ${from(`SELECT ${x} AS v`)})`, shape);
  const arg = st.field !== undefined ? num(outer(st.field)) : String(st.value ?? 0);
  const scale = 10 ** (st.n ?? 0);
  switch (st.op) {
    case "trim": return over(`trim(v, ${spaces})`);
    case "replace": return over(`replace(v, ${lit(st.text ?? "")}, ${lit(st.to ?? "")})`);
    case "before": return over(`CASE WHEN instr(v, ${lit(st.text ?? "")}) > 0 THEN substr(v, 1, instr(v, ${lit(st.text ?? "")}) - 1) ELSE v END`);
    case "after": return over(`CASE WHEN instr(v, ${lit(st.text ?? "")}) > 0 THEN substr(v, instr(v, ${lit(st.text ?? "")}) + length(${lit(st.text ?? "")})) ELSE '' END`);
    case "substr": return over(`substr(v, ${st.n}, ${st.length})`);
    case "padStart": return over(`CASE WHEN length(v) >= ${st.length} THEN v ELSE substr(replace(hex(zeroblob(${st.length})), '00', ${lit(st.text!)}), 1, ${st.length} - length(v)) || v END`);
    case "ifEmpty": return over(`CASE WHEN v = '' THEN ${st.field !== undefined ? outer(st.field) : lit(st.text ?? "")} ELSE v END`);
    case "map": return over(`CASE v ${(st.pairs ?? []).map(([a, b]) => `WHEN ${lit(a)} THEN ${lit(b)}`).join(" ")} ELSE ${st.otherwise !== undefined ? lit(st.otherwise) : "v"} END`);
    case "if": return over(`CASE WHEN ${cond(st.cond ?? [], f => f ? outer(f) : "v")} THEN ${lit(st.then ?? "")} ELSE ${lit(st.else ?? "")} END`);
    case "cases": return over(`CASE ${(st.branches ?? []).map(b => `WHEN ${cond(b.cond, f => f ? outer(f) : "v")} THEN ${lit(b.then)}`).join(" ")} ELSE ${lit(st.otherwise ?? "")} END`);
    case "toNumber": return over(num("v"), "number");
    case "add": return over(`v + (${arg})`, "number");
    case "sub": return over(`v - (${arg})`, "number");
    case "mul": return over(`v * (${arg})`, "number");
    case "div": return over(`v * 1.0 / (${arg})`, "number");
    case "round": return over(`CASE WHEN v IS NULL THEN NULL ELSE (CASE WHEN v < 0 THEN -1 ELSE 1 END) * CAST(abs(v) * ${scale} + 0.5 AS INTEGER) * 1.0 / ${scale} END`, "number");
    case "floor": return over(`(SELECT CASE WHEN x IS NULL THEN NULL WHEN x < CAST(x AS INTEGER) THEN CAST(x AS INTEGER) - 1 ELSE CAST(x AS INTEGER) END * 1.0 / ${scale} FROM (SELECT v * ${scale} AS x))`, "number");
    case "thousands": return over("CASE WHEN v IS NULL THEN '' ELSE printf('%,d', v) END");
    case "plain": return over("CASE WHEN v IS NULL THEN '' ELSE printf('%.15g', v) END");
    case "toDate": return e(date(x), "date");
    case "dateAdd": return over(`date(v, ${lit(`${st.n ?? 0} ${{ day: "days", month: "months", year: "years" }[st.unit ?? "day"]}`)})`, "date");
    case "dateDiff": return over(`CAST(round(julianday(v) - julianday(${date(outer(st.field!))})) AS INTEGER)`, "number");
    case "year": return over("CAST(substr(v, 1, 4) AS INTEGER)", "number");
    case "month": return over("CAST(substr(v, 6, 2) AS INTEGER)", "number");
    case "quarter": return over("(CAST(substr(v, 6, 2) AS INTEGER) + 2) / 3", "number");
    case "dateFormat": {
      const piece: Record<string, string> = { YYYY: "substr(v, 1, 4)", MM: "substr(v, 6, 2)", DD: "substr(v, 9, 2)",
        M: "CAST(CAST(substr(v, 6, 2) AS INTEGER) AS TEXT)", D: "CAST(CAST(substr(v, 9, 2) AS INTEGER) AS TEXT)" };
      const parts = (st.text ?? "").split(/(YYYY|MM|M|DD|D)/).filter(Boolean).map(p => piece[p] ?? lit(p));
      return over(`CASE WHEN v IS NULL THEN '' ELSE ${parts.join(" || ") || "''"} END`);
    }
  }
  return e(x);
}

// ── 목업의 SQL 검사 ───────────────────────────────────
// 진짜 검사는 백엔드가 읽기 전용 연결과 authorizer 로 한다(§5). 목업은 모양만 본다.

export type SqlIssue = { from: number; to: number; message: string };

/** 주석과 글자를 같은 길이의 빈칸으로 덮는다 — 위치를 지키면서 그 안의 낱말을 무시하려고. */
function blankOut(sql: string): string {
  return sql.replace(/--[^\n]*|\/\*[\s\S]*?(\*\/|$)|'(?:[^']|'')*'?/g, m => " ".repeat(m.length));
}

export function checkSqlMock(sql: string): SqlIssue[] {
  const text = blankOut(sql);
  const issues: SqlIssue[] = [];
  const lead = /\S/.exec(text);
  if (!lead) return [{ from: 0, to: 0, message: "SQL이 비었습니다" }];
  if (!/^select\b/i.test(text.slice(lead.index))) issues.push({ from: lead.index, to: lead.index + (/^\S+/.exec(text.slice(lead.index))?.[0].length ?? 1), message: "SELECT로 시작하는 문장 하나만 쓸 수 있습니다" });
  const semi = text.indexOf(";");
  if (semi >= 0 && text.slice(semi + 1).trim()) issues.push({ from: semi, to: semi + 1, message: "SQL 문장은 하나만 쓸 수 있습니다" });
  for (const m of text.matchAll(/\b(from|join)\s+("(?:[^"]|"")+"|\[[^\]]+\]|[\p{L}_][\p{L}\p{N}_]*)/giu)) {
    const raw = m[2];
    const name = raw.startsWith("\"") ? raw.slice(1, -1).replace(/""/g, "\"") : raw.startsWith("[") ? raw.slice(1, -1) : raw;
    if (contractViews.includes(name)) continue;
    const at = m.index! + m[0].length - raw.length;
    issues.push({ from: at, to: at + raw.length, message: `공개 뷰(v_공고 등)만 읽을 수 있습니다: ${name}` });
  }
  return issues;
}

