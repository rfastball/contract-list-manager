import { afterEach, beforeEach, expect, it } from "vitest";
import mappingJson from "../../crates/pclm-core/src/erp/mapping.json?raw";
import captureSource from "../../extension/capture.js?raw";
import exportSource from "../../extension/export.js?raw";
import pageHtml from "../../scripts/fake-g2b/index.html?raw";
import dataJson from "../../scripts/fake-g2b/data.json?raw";

// 가짜 나라장터(scripts/fake-g2b)가 진짜 수집 규칙과 진짜 capture.js 로 읽히는지 — 매핑이 바뀌어 가짜 화면이 낡으면 여기서 붉어진다.
const profiles = JSON.parse(mappingJson).profiles;
const data = JSON.parse(dataJson);
const location = { origin: "https://www.g2b.go.kr" };
let page: any, history: any;

beforeEach(async () => {
  const parsed = new DOMParser().parseFromString(pageHtml, "text/html");
  document.body.innerHTML = parsed.body.innerHTML; // innerHTML 로 넣은 스크립트는 돌지 않는다 — 아래에서 돌린다.
  // jsdom 에는 레이아웃이 없다. 숨김은 hidden 속성으로만 가린다.
  Object.defineProperty(Element.prototype, "checkVisibility", { configurable: true, value(this: Element) { return !this.closest("[hidden]"); } });
  page = {}; page.top = page;
  history = { state: null, replaceState(state: unknown) { this.state = state; } };
  const script = parsed.querySelector("script")!.textContent!;
  new Function("window", "document", "history", "fetch", script)(page, document, history, async () => ({ json: async () => data }));
  await new Promise(resolve => setTimeout(resolve));
});

afterEach(() => {
  document.body.replaceChildren();
  delete (Element.prototype as Partial<Element>).checkVisibility;
});

const control = (name: string) => document.querySelector<HTMLInputElement>(`nav [data-control="${name}"]`)!;
const change = (name: string, value: string) => {
  const e = control(name); e.value = value; e.dispatchEvent(new Event("change"));
};
const read = () => new Function("document", "window", "location", "history", "profiles",
  captureSource + "; return pclmReadMapped(profiles);")(document, page, location, history, profiles);
const at = (row: any, path: string) => path.split(".").reduce((v, k) => v?.[k], row);
const sources = (fields: any[]) => fields.flatMap((f: any) => f.secondarySource ? [f.source, f.secondarySource] : [f.source]);

it("세 화면은 진짜 매핑으로 번호 · 차수 · 모든 칸 · 모든 표가 읽히고, 수집하지 않는 화면은 unsupported 다", () => {
  const read3: Record<string, any> = {};
  for (const screen of data.screens) {
    change("screen", screen.code);
    expect(page.com.gfnGetMenuNo()).toBe(screen.code);
    expect(history.state.data.menuNo).toBe(screen.code);
    const profile = profiles.find((p: any) => p.screen?.code === screen.code);
    if (!profile) {
      expect(read).toThrow(expect.objectContaining({ unsupported: true }));
      continue;
    }
    expect(screen.title).toBe(`${profile.screen.name}(개발용)`);
    const result = read();
    expect(result).toMatchObject({ profile: profile.id, screen: screen.code });
    const point = result.data.pointInfo;
    for (const key of sources(profile.fields)) expect(point, key).toHaveProperty([key]);
    expect(point[profile.baseField]).toBeTruthy();
    for (const table of profile.tables) {
      const id = Object.keys(result.data.tables).find(k => table.source.startsWith("*") ? k.endsWith(table.source.slice(1)) : k === table.source);
      expect(id, table.source).toBeDefined();
      const rows = result.data.tables[id!];
      expect(rows.length, table.source).toBeGreaterThan(0);
      for (const row of rows) for (const path of [...table.keys, ...sources(table.fields)]) expect(at(row, path), `${table.source} ${path}`).toBeDefined();
    }
    read3[profile.entityType] = { ...result.data, profile };
  }
  expect(Object.keys(read3).sort()).toEqual(["contract", "notice", "request"]);
  expect(data.screens.some((s: any) => !profiles.some((p: any) => p.screen?.code === s.code))).toBe(true);

  // 한 건이다 — explicit_links.rs 가 잇는 참조 칸이 서로 맞는다.
  const items = (d: any, target: string) => d.tables[d.profile.tables.find((t: any) => t.target === target).source];
  const { request, notice, contract } = read3;
  const base = request.pointInfo.ctrtDmndRcptNo, seq = request.pointInfo.ctrtDmndRcptOrd;
  for (const r of items(request, "request_item")) expect([r.ctrtDmndRcptNo, r.ctrtDmndRcptOrd]).toEqual([base, seq]);
  const refs = (rows: any[]) => rows.map(r => [r.ctrtDmndRcptNo, r.ctrtDmndRcptOrd, r.ctrtDmndRcptItemSqno].join("/"));
  expect(refs(items(notice, "notice_item"))).toEqual(refs(items(request, "request_item")));
  expect([contract.pointInfo.ctrtDmndRcptNo, contract.pointInfo.bidPbancNo]).toEqual([base, notice.pointInfo.bidPbancNo]);
  const [ctrtNo, ctrtChgOrd] = contract.pointInfo.ctrtNoOrd.split("-");
  for (const r of items(contract, "contract_item")) expect([r.ctrtNo, r.ctrtChgOrd]).toEqual([ctrtNo, ctrtChgOrd]);
});

it("조작 띠 — 차수 +1 · 품목 줄 수 · 로딩 표시 · 고친 칸이 수집에 그대로 닿는다", () => {
  change("screen", "01579");
  control("seq").click();
  change("rows", "5");
  const result = read();
  expect(result.data.pointInfo.ctrtNoOrd).toBe("R26TA00000001-01");
  const rows = result.data.tables.mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis;
  expect(rows.map((r: any) => [r.ctrtChgOrd, r.ctrtItemSqno])).toEqual([1, 2, 3, 4, 5].map(n => ["01", n]));

  const input = document.getElementById("mf_wfm_container_ctrtNm") as HTMLInputElement;
  input.value = "사람이 고친 계약명"; input.dispatchEvent(new Event("input"));
  expect(read().data.pointInfo.ctrtNm).toBe("사람이 고친 계약명");

  const loading = control("loading"); loading.checked = true; loading.dispatchEvent(new Event("change"));
  expect(read).toThrow("화면을 불러오는 중입니다.");
});

it("조작 띠의 글은 내보내기의 화면 필드에 섞이지 않는다", () => {
  change("screen", "01179");
  const result = new Function("document", "window", "location", exportSource + "; return pclmExportPage();")(document, page, { ...location, pathname: "/" });
  const fields = result.sheets.find((s: any) => s.source === "DOM").rows.slice(1);
  expect(fields.length).toBeGreaterThan(0);
  expect(fields.every(([, , id]: string[]) => id.startsWith("mf_wfm_container_"))).toBe(true);
  expect(JSON.stringify(result.sheets)).not.toMatch(/가짜 나라장터|품목 줄 수|차수 \+1|로딩 표시/);
});
