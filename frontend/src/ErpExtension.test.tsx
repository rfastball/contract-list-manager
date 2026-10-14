import { afterEach, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";
import mappingJson from "../../src/Pclm.Core/Erp/mapping.json?raw";
import captureSource from "../../extension/capture.js?raw";
import popupSource from "../../extension/popup.js?raw";
import popupHtml from "../../extension/popup.html?raw";
import backgroundSource from "../../extension/background.js?raw";
import exportSource from "../../extension/export.js?raw";
import optionsSource from "../../extension/options.js?raw";
import optionsHtml from "../../extension/options.html?raw";
import manifestJson from "../../extension/manifest.json?raw";

afterEach(() => {
  document.body.replaceChildren();
  delete (Element.prototype as Partial<Element>).checkVisibility;
  vi.restoreAllMocks(); vi.useRealTimers();
});

function screen() {
  document.body.innerHTML = `<h2 id="mf_wfm_cntsHeader_spnHeaderTitle">입찰공고상세[내자]</h2>
    <div id="mf_wfm_container_mainWframe"><table><tbody><tr><td data-title="공고명"><span class="w2textbox" id="mf_wfm_container_mainWframe_title">시험 공고</span></td></tr></tbody></table></div>`;
  const root = document.getElementById("mf_wfm_container_mainWframe")!;
  for (const [label, value] of Object.entries({ 입찰공고번호: "R26BK00000001 - 001", 공고종류: "실공고", 게시일시: "2026/09/24 10:00:00",
    입찰방식: "전자입찰", 낙찰방법: "적격심사제", 계약방법: "제한경쟁", 공고기관: "시험기관", 공고담당자: "읽지 않을 연락처" })) {
    const input = document.createElement("input"); input.title = label; input.value = value;
    input.readOnly = true; input.id = "mf_wfm_container_mainWframe_" + root.children.length; root.append(input);
  }
  // jsdom에는 레이아웃 엔진이 없다. 화면 표시 여부만 대역으로 둔다.
  Object.defineProperty(Element.prototype, "checkVisibility", { configurable: true, value() { return true; } });
}

const popupSender = { id: "extension", url: "chrome-extension://extension/popup.html" };

function worker() {
  const local: Record<string, any> = {};
  const native = vi.fn(async (_host: string, request: any) => {
    let result: any;
    if (request.method === "hello") result = { datasetId: "test", mapping: { profiles: [] }, mappingRevision: "v1" };
    return { protocolVersion: 2, requestId: request.requestId, ok: true, result };
  });
  // 상시 연결 포트 대역. 호스트 쪽 끊김은 시험이 drop() 으로 일으킨다 — 확장이 스스로 disconnect() 한 것은
  // 브라우저가 그 포트의 onDisconnect 로 알리지 않는다.
  const ports: { host: string; posted: any[]; disconnect: () => void; drop: () => void; closed: boolean;
    onMessage: { addListener: ReturnType<typeof vi.fn> } }[] = [];
  const connectNative = vi.fn((host: string) => {
    const listeners: (() => void)[] = [];
    const port = { host, posted: [] as any[], closed: false,
      postMessage: (message: any) => { port.posted.push(message); },
      onMessage: { addListener: vi.fn() }, onDisconnect: { addListener: (fn: () => void) => listeners.push(fn) },
      disconnect: vi.fn(() => { port.closed = true; }),
      drop: () => { port.closed = true; listeners.forEach(fn => fn()); } };
    ports.push(port);
    return port;
  });
  const chrome = {
    runtime: { id: "extension", getURL: (file: string) => "chrome-extension://extension/" + file,
      sendNativeMessage: native, connectNative, lastError: undefined, onStartup: { addListener: vi.fn() },
      onMessage: { addListener: vi.fn() }, onInstalled: { addListener: vi.fn() }, openOptionsPage: vi.fn(),
      getManifest: () => ({ version: "0.4.1" }), reload: vi.fn() },
    tabs: { query: vi.fn(async (_?: object): Promise<any[]> => [{ id: 1 }]),
      onRemoved: { addListener: vi.fn() }, onActivated: { addListener: vi.fn() }, onUpdated: { addListener: vi.fn() },
      update: vi.fn(async (..._: any[]) => ({})), create: vi.fn(async (..._: any[]) => ({})),
      sendMessage: vi.fn(async (..._: any[]): Promise<any> => { throw new Error("수신자 없음"); }) },
    windows: { WINDOW_ID_NONE: -1, update: vi.fn(async (..._: any[]) => ({})), create: vi.fn(async (..._: any[]) => ({})), onFocusChanged: { addListener: vi.fn() } },
    // 옛 판이 쓰던 자리. 지금 판은 닿지 않아야 한다.
    commands: { getAll: vi.fn(async () => []), onCommand: { addListener: vi.fn() } },
    action: { setBadgeText: vi.fn(async (_: object) => {}), setBadgeBackgroundColor: vi.fn(async (_: object) => {}), openPopup: vi.fn(async () => {}) },
    permissions: { contains: vi.fn(async (_: object) => true), onAdded: { addListener: vi.fn() }, onRemoved: { addListener: vi.fn() } },
    scripting: { executeScript: vi.fn(async (args: any): Promise<any[]> => args.files ? [] : [{ result: { error: "화면을 읽지 못했습니다." } }]) },
    storage: { onChanged: { addListener: vi.fn() }, local: { get: vi.fn(async (_?: unknown): Promise<Record<string, unknown>> => ({ ...local })),
      set: vi.fn(async (value: object) => { Object.assign(local, value); }) } },
  };
  new Function("chrome", backgroundSource)(chrome);
  // 팝업이 중계기에 묻는 길 — 받지 않으면(true 를 돌려주지 않으면) 답이 없다.
  const ask = (message: object, sender: object = popupSender): Promise<any> => new Promise(resolve => {
    if (chrome.runtime.onMessage.addListener.mock.lastCall![0](message, sender, resolve) !== true) resolve(undefined);
  });
  const command = (message: object) => ports[0].onMessage.addListener.mock.calls[0][0](message);
  return { chrome, native, ports, connectNative, local, ask, command };
}

const runUi = new Function("chrome", "document", exportSource + "; return " + popupSource);
const popupBody = () => { document.body.innerHTML = popupHtml.match(/<body>([\s\S]*)<\/body>/)![1]; };
const text = (id: string) => document.getElementById(id)!.textContent;
const shown = (id: string) => !document.getElementById(id)!.hidden;

it("임의 업무 화면은 이름과 ID를 보존하고 원천 표만 내보내며 내부 값과 빈 열을 제외한다", () => {
  screen();
  document.body.innerHTML = `<label for="number">문서번호</label><input id="number" value="00123">
    <input id="password" type="password" value="비밀"><input id="hidden" type="hidden" value="내부값">
    <input id="token" value="인증값"><input id="blank" value=""><input id="quantity" title="수량" value="0">
    <div hidden><input id="invisible" value="숨긴값"></div><select id="method" title="방식"><option value="01">일반</option></select>
    <div id="grid" aria-label="품목"><input id="gridEditor" value="중복 표시"></div><div id="gridCopy"></div>
    <div id="brokenGrid"></div><table id="schedule"><tr><th>일정</th><th>날짜</th><th hidden>내부 열</th><th>작업</th></tr>
    <tr><td>접수<span hidden>숨긴값</span></td><td><input value="2026-09-27"></td><td hidden>내부값</td><td><button>삭제</button></td></tr></table>`;
  const list = { getAllJSON: () => [{ itemNo: "001", amount: 0, empty: "", rowStatus: "R", secret: { session: "내부세션" }, detail: { name: "품목" } },
    { itemNo: "001", amount: 0, empty: "", detail: { name: "품목" } }] };
  const fakeWindow: any = { $p: { getComponentById: (id: string) => id === "grid" || id === "gridCopy" ? { getDataList: () => list } :
    id === "brokenGrid" ? { getDataList: () => { throw new Error("미로딩"); } } : null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", exportSource + "; return pclmExportPage();");
  const location = { origin: "https://www.g2b.go.kr", pathname: "/work", search: "?token=비밀" };
  const result = read(document, fakeWindow, location);
  expect(result.sheets.map((s: any) => s.name)).toEqual(["화면 필드", "품목", "schedule", "수집 정보"]);
  expect(result.sheets[0].rows).toContainEqual(["문서번호", "00123", "number", ""]);
  expect(result.sheets[0].rows).toContainEqual(["수량", "0", "quantity", ""]);
  expect(result.sheets[0].rows).toContainEqual(["방식", "일반", "method", ""]);
  expect(result.sheets[1].rows).toEqual([["itemNo", "amount", "detail.name"], ["001", "0", "품목"], ["001", "0", "품목"]]);
  expect(result.sheets[2].rows).toEqual([["일정", "날짜"], ["접수", "2026-09-27"]]);
  expect(result.warnings).toEqual(["표를 읽지 못함: brokenGrid"]);
  expect(JSON.stringify(result)).not.toMatch(/비밀|내부값|인증값|숨긴값|중복 표시|내부세션|rowStatus|empty/);
  expect(() => read(document, fakeWindow, { origin: "https://other.example" })).toThrow("나라장터");
  document.body.replaceChildren();
  expect(() => read(document, fakeWindow, location)).toThrow("내보낼 필드나 표가 없습니다");
  screen();
  const mapped = new Function("document", "window", "location", captureSource + "; return pclmReadMapped([]);");
  expect(() => mapped(document, fakeWindow, location)).toThrow("앱을 새 판으로 바꿔 주세요");
});

it("WebSquare 표는 보이는 열을 화면 순서·화면 머리글·표시값으로 내보내고 단추·행 번호·숨긴 열과 DataList 전용 키를 뺀다", () => {
  screen();
  document.body.innerHTML = `<div id="grid" aria-label="공고 목록"></div><div id="gridCopy"></div>`;
  // 열 정의: [ID, 머리글, 종류, 보임, 화면 순서, 행마다 표시값]
  const columns: [string, string, string, boolean, number, string[]][] = [
    ["rowNum", "No", "text", true, 0, ["1", "2"]],
    ["bidNm", "공고명", "link", true, 2, ["시험 공고 &amp; 부속", '둘째 공고<div style="display: none"><p>숨긴 풍선</p></div>']],
    ["pstDt", "게시일시<br>(마감일시)", "text", true, 3, ["2026/09/21 10:00<br />(2026/09/28 10:00)", "2026/09/22 09:00"]],
    ["hiddenCol", "숨긴 열", "text", false, 4, ["숨긴값", "숨긴값"]],
    ["btnView", "보기", "button", true, 5, ["<button>보기</button>", "<button>보기</button>"]],
    ["dtl", "상세", "text", true, 6, ['<button type="button" class="btn"></button>', '<button type="button" class="btn"></button>']],
    ["rrno", "번호", "text", true, 7, ["000000-0000000", "000000-0000000"]],
    ["instNm", "공고기관", "text", true, 1, ["시험기관", "둘째 기관"]],
    ["dmndInstNm", "공고기관", "text", true, 8, ["수요기관", ""]],
  ];
  const list = { getAllJSON: () => [{ bidNm: "시험 공고", extraKey: "원천전용", ntceInsttCd: "X0000001" }] };
  const grid = {
    getDataList: () => list,
    getColumnCount: () => columns.length,
    getColumnID: (i: number) => columns[i][0],
    getHeaderID: (i: number) => "column" + i,
    getHeaderValue: (id: string) => columns[Number(id.slice(6))][1],
    getColumnType: (i: number) => columns[i][2],
    getColumnVisible: (i: number) => columns[i][3],
    getDisplayColumnIndex: (i: number) => columns[i][4],
    getDataLength: () => 2,
    getCellDisplayData: (r: number, i: number) => columns[i][5][r],
  };
  const fakeWindow: any = { $p: { getComponentById: (id: string) => id === "grid" || id === "gridCopy" ? grid : null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", exportSource + "; return pclmExportPage();");
  const result = read(document, fakeWindow, { origin: "https://www.g2b.go.kr", pathname: "/work" });
  expect(result.sheets.map((s: any) => s.name)).toEqual(["공고 목록", "수집 정보"]);
  expect(result.sheets[0].rows).toEqual([
    ["공고기관", "공고명", "게시일시 (마감일시)", "공고기관 2"],
    ["시험기관", "시험 공고 & 부속", "2026/09/21 10:00\n(2026/09/28 10:00)", "수요기관"],
    ["둘째 기관", "둘째 공고", "2026/09/22 09:00", ""],
  ]);
  expect(result.warnings).toEqual([]);
  expect(JSON.stringify(result.sheets[0])).not.toMatch(/extraKey|원천전용|ntceInsttCd|X0000001|숨긴|보기|상세|000000|rowNum|"No"/);
});

it("상시 연결은 포트 하나로 인사하고, 끊기면 물러서며 다시 잇고, 개발 전환이 바뀌면 그쪽 호스트로 옮긴다", async () => {
  vi.useFakeTimers();
  const w = worker();
  await vi.advanceTimersByTimeAsync(0);
  // 워커가 뜰 때와 브라우저 시작 알림이 겹쳐도 포트는 하나다.
  w.chrome.runtime.onStartup.addListener.mock.lastCall![0]();
  await vi.advanceTimersByTimeAsync(0);
  expect(w.ports.map(p => p.host)).toEqual(["kr.rfastball.pclm.erp"]);
  const [hello] = w.ports[0].posted;
  expect(hello).toMatchObject({ protocolVersion: 2, method: "presence", params: { extensionVersion: "0.4.1", browser: "기타" } });
  expect(hello.requestId).toMatch(/^[0-9a-f-]{36}$/);
  expect(w.native).not.toHaveBeenCalled(); // 수집 RPC 는 따로 간다.

  // 호스트가 죽으면 1초 → 2초 → 5초로 물러서며 다시 잇는다.
  for (const [index, delay] of [[0, 1000], [1, 2000], [2, 5000]] as const) {
    w.ports[index].drop();
    await vi.advanceTimersByTimeAsync(delay - 1);
    expect(w.ports).toHaveLength(index + 1);
    await vi.advanceTimersByTimeAsync(1);
    expect(w.ports).toHaveLength(index + 2);
    expect(w.ports[index + 1].posted[0].method).toBe("presence");
  }
  // 한동안(60초) 버틴 포트가 끊기면 다시 1초부터.
  await vi.advanceTimersByTimeAsync(60_000);
  w.ports[3].drop();
  await vi.advanceTimersByTimeAsync(1000);
  expect(w.ports).toHaveLength(5);

  // 개발 호스트로 바꾸면 지금 포트를 놓고 그쪽에 잇는다.
  w.local.erpDevelopment = true;
  for (const [listener] of w.chrome.storage.onChanged.addListener.mock.calls)
    listener({ erpDevelopment: { newValue: true } }, "local");
  await vi.advanceTimersByTimeAsync(0);
  expect(w.ports[4].disconnect).toHaveBeenCalled();
  expect(w.ports.map(p => p.host).slice(5)).toEqual(["kr.rfastball.pclm.erp.dev"]);
  expect(w.ports[5].posted[0].method).toBe("presence");
  await vi.advanceTimersByTimeAsync(120_000);
  expect(w.ports).toHaveLength(6); // 놓은 포트는 다시 잇지 않는다.
});

it("지금 보는 화면: 포트가 열린 동안 나라장터 탭을 알고 읽어 묶어 보고하고, 앞 탭만 지문으로 다시 읽으며, 앱의 명령을 따른다", async () => {
  vi.useFakeTimers();
  const w = worker();
  // Chrome 의 탭 셋 — 앞(1, 계약 상세)과 뒤(2, 개찰 결과), 그리고 나라장터가 아닌 탭 하나.
  const g2b = [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a", title: "나라장터" },
    { id: 2, windowId: 11, url: "https://www.g2b.go.kr/b", title: "나라장터" }];
  let active: any = g2b[0];
  w.chrome.tabs.query.mockImplementation(async (filter?: any) => filter?.active ? [active] : [...g2b, { id: 3, url: "https://other.example/" }]);
  const prints: Record<number, string> = { 1: "계약 상세|R26TA00000001", 2: "개찰 결과|R26BK00000001" };
  const reads: number[] = [];
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => {
    const tabId = args.target.tabId;
    if (args.files) return [];
    if (!args.args) return [{ result: prints[tabId] }]; // 지문
    reads.push(tabId);
    return [{ documentId: "doc" + tabId, result: tabId === 1
      ? { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: { ctrtNoOrd: "R26TA00000001-00" }, tables: {} }, scope: "live", rowMatches: {} } }
      : { screen: "개찰 결과", menu: "01175", error: "메뉴 01175 화면은 수집하지 않습니다. 화면 자료는 엑셀로 내보낼 수 있습니다.", unsupported: true } }];
  });
  const reports = () => w.ports.flatMap(p => p.posted).filter(m => m.method === "tabs");

  await vi.advanceTimersByTimeAsync(0);
  await vi.advanceTimersByTimeAsync(300);
  // 처음 한 번 둘 다 읽는다. 수집 규칙은 한 번만 묻는다.
  expect(reads.sort()).toEqual([1, 2]);
  expect(w.native.mock.calls.filter(([, r]) => r.method === "hello")).toHaveLength(1);
  const last = reports().at(-1)!;
  expect(last).toMatchObject({ protocolVersion: 2, method: "tabs", params: { front: 1 } });
  expect(last.params.tabs.map((t: any) => [t.tabId, t.windowId, t.state, t.screen])).toEqual(
    [[1, 10, "supported", "계약 상세"], [2, 11, "unsupported", "개찰 결과"]]);
  expect(last.params.tabs[0].snapshot).toMatchObject({ profile: "g2b-contract-v1", mappingRevision: "v1" });
  expect(last.params.tabs[0].readAt).toMatch(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d$/);
  // 바뀌지 않으면 다시 보내지 않는다. 묶음 사이의 여러 변화는 한 번으로 간다.
  const sent = reports().length;
  await vi.advanceTimersByTimeAsync(4000);
  expect(reports()).toHaveLength(sent);
  expect(reads).toHaveLength(2); // 지문이 그대로면 앞 탭도 다시 읽지 않는다.

  // 앞 탭의 화면이 주소 없이 바뀌면 지문으로 알고 다시 읽는다. 뒤 탭은 지문이 바뀌어도 읽지 않는다.
  prints[1] = "계약 상세|R26TA00000002"; prints[2] = "개찰 결과|바뀜";
  await vi.advanceTimersByTimeAsync(2000);
  expect(reads).toEqual([1, 2, 1]);

  // 다른 탭이 앞으로 오면 그 탭을 읽고 앞을 옮긴다.
  active = g2b[1];
  w.chrome.tabs.onActivated.addListener.mock.lastCall![0]({ tabId: 2, windowId: 11 });
  await vi.advanceTimersByTimeAsync(300);
  expect(reads.at(-1)).toBe(2);
  expect(reports().at(-1)!.params.front).toBe(2);
  // 나라장터가 아닌 탭이 앞이면 앞은 없다.
  active = { id: 3, windowId: 10, url: "https://other.example/" };
  w.chrome.windows.onFocusChanged.addListener.mock.lastCall![0](10);
  await vi.advanceTimersByTimeAsync(300);
  expect(reports().at(-1)!.params.front).toBeNull();
  // 탭을 닫으면 보고에서 빠진다.
  w.chrome.tabs.onRemoved.addListener.mock.calls.forEach(([fn]: any) => fn(2));
  await vi.advanceTimersByTimeAsync(300);
  expect(reports().at(-1)!.params.tabs.map((t: any) => t.tabId)).toEqual([1]);

  // 앱의 명령. 확장이 아는 나라장터 탭에만 닿는다.
  const command = (message: object) => w.ports[0].onMessage.addListener.mock.calls[0][0](message);
  command({ protocolVersion: 2, command: "focusTab", tabId: 1 });
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.tabs.update).toHaveBeenCalledWith(1, { active: true });
  expect(w.chrome.windows.update).toHaveBeenCalledWith(10, { focused: true });
  const before = reads.length;
  command({ protocolVersion: 2, command: "read", tabId: 1 });
  await vi.advanceTimersByTimeAsync(0);
  expect(reads).toHaveLength(before + 1);
  command({ protocolVersion: 2, command: "export", tabId: 1 });
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.scripting.executeScript).toHaveBeenCalledWith({ target: { tabId: 1 }, world: "MAIN", files: ["export.js"] });
  expect(w.chrome.tabs.sendMessage).not.toHaveBeenCalled();
  command({ protocolVersion: 2, command: "focusTab", tabId: 3 });
  command({ protocolVersion: 2, command: "read", tabId: 99 });
  command({ command: "read", tabId: 1 });
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.tabs.update).toHaveBeenCalledTimes(1);
  expect(reads).toHaveLength(before + 1);

  // 포트가 끊기면 읽지도 보고하지도 않는다.
  w.ports[0].drop();
  const settled = reads.length;
  w.chrome.tabs.onUpdated.addListener.mock.lastCall![0](1, { status: "complete" }, g2b[0]);
  prints[1] = "또 바뀜";
  await vi.advanceTimersByTimeAsync(900);
  expect(reads).toHaveLength(settled);
});

it("활성 매핑은 표의 전체 행과 복합키를 읽고 허용하지 않은 값을 보내지 않는다", () => {
  screen();
  const p = JSON.parse(mappingJson).profiles.find((p: any) => p.id === "g2b-request-v1");
  const table = p.tables.find((t: any) => t.required);
  document.body.innerHTML = `<div id="mf_wfm_container"><input id="ctrtDmndBizNm" value="시험 접수" readonly><div id="${table.source}"></div></div>`;
  const rows = ["001", "002"].map(key => ({ ctrtDmndRcptNo: "RC001", ctrtDmndRcptOrd: "000", ctrtDmndRcptItemSqno: key,
    ndfsPrcmDmndNo: "REPEATED", ctrtDmndQty: "0", giveActno: "보내면 안 되는 값" }));
  const fakeWindow: any = { com: { gfnGetMenuNo: () => "01117" },
    $p: { getComponentById: (id: string) => id === table.source ? { getDataList: () => ({ getAllJSON: () => rows }) } : null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", "profiles", captureSource + "; return pclmReadMapped(profiles);");
  const result = read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, [p]);
  expect(result.data.tables[table.source]).toHaveLength(2);
  expect(result.data.tables[table.source][1].ctrtDmndRcptItemSqno).toBe("002");
  expect(result.data.tables[table.source][0].ctrtDmndQty).toBe("0");
  expect(JSON.stringify(result)).not.toContain("보내면 안 되는 값");
});

it("변경계약서 작성 화면은 번호·차수를 품목 표에서 읽고, 바뀌기 전 업체 표는 보내지 않는다", () => {
  screen();
  // 메뉴 01608 — 머리에는 차수가 없다. 업체는 바뀌기 전(앞 차수) 표와 지금 표로 나뉘어 온다.
  const item = { ctrtNo: "R26TA00000005", ctrtChgOrd: "01", ctrtItemSqno: 1, ctrtAmt: 2000 };
  const grids: Record<string, unknown[]> = {
    mf_wfm_container_grdCtrtLis: [item], mf_wfm_container_grdCtrtLisExcel: [item],
    mf_wfm_container_grdBeoeEtpsLst: [{ ctrtChgOrd: "00", ctentUntyGrpNo: "OLD0001", ctentUntyGrpNm: "바뀌기 전 업체" }],
    mf_wfm_container_grdCrntEtpsLst: [{ ctrtChgOrd: "01", ctentUntyGrpNo: "NEW0001", ctentUntyGrpNm: "시험 업체" }],
  };
  document.body.innerHTML = `<div id="mf_wfm_container">${Object.keys(grids).map(id => `<div id="${id}"></div>`).join("")}</div>`;
  const fakeWindow: any = { com: { gfnGetMenuNo: () => "01608" },
    $p: { getComponentById: (id: string) => id in grids ? { getDataList: () => ({ getAllJSON: () => grids[id] }) } : null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", "profiles", captureSource + "; return pclmReadMapped(profiles);");
  const run = () => read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, JSON.parse(mappingJson).profiles);
  const result = run();
  expect(result).toMatchObject({ profile: "g2b-contract-change-v1", screen: "01608" });
  expect(Object.keys(result.data.tables).sort()).toEqual(["mf_wfm_container_grdCrntEtpsLst", "mf_wfm_container_grdCtrtLis"]);
  expect(JSON.stringify(result)).not.toContain("바뀌기 전 업체");
  // 품목 표가 비면 번호를 읽지 못한 것이다 — 머리의 계약번호만으로 짐작하지 않는다.
  grids.mf_wfm_container_grdCtrtLis = [];
  expect(run).toThrow("화면에서 번호를 읽지 못했습니다.");
});

it("받는지는 hello 매핑에서 화면을 단 프로필만으로 정하고, 화면 하나를 그 프로필 하나로 읽으며, 표가 없어도 받는다", () => {
  screen();
  const profiles = JSON.parse(mappingJson).profiles;
  // 계약 번호와 공고 번호·차수가 함께 보이는 화면. 품목표는 없다.
  const values: Record<string, string> = { ctrtNoOrd: "R26TA00000001-00", bidPbancNo: "R26BK00000001", bidPbancOrd: "000" };
  const render = () => {
    document.body.innerHTML = `<div id="mf_wfm_container">${Object.keys(values).map(k => `<span id="mf_wfm_container_${k}"></span>`).join("")}</div>`;
  };
  render();
  let menu: unknown = "01579";
  const fakeWindow: any = { com: { gfnGetMenuNo: () => menu }, $p: { getComponentById: (id: string) => {
    const key = id.replace("mf_wfm_container_", "");
    return key in values ? { getRef: () => "data:dma_pointInfo." + key, getValue: () => values[key] } : null;
  } } };
  fakeWindow.top = fakeWindow;
  const fakeHistory: any = { state: null };
  const read = new Function("document", "window", "location", "history", "profiles",
    captureSource + "; return pclmReadMapped(profiles);");
  const run = (given: any[] = profiles) => read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, fakeHistory, given);
  const refusal = (given?: any[]) => {
    try { run(given); } catch (error) { return error as Error & { unsupported?: boolean }; }
    throw new Error("거절되지 않았다");
  };

  expect(run()).toMatchObject({ profile: "g2b-contract-v1", screen: "01579", data: { tables: {} } });
  // 공고 화면이면 계약 번호가 보여도 공고 프로필 하나로만 읽는다.
  menu = "01179";
  expect(run()).toMatchObject({ profile: "g2b-notice-a-v1", screen: "01179" });
  // 나라장터의 함수가 없으면 주소 상태의 번호를 쓴다.
  delete fakeWindow.com;
  fakeHistory.state = { data: { menuNo: "01579" } };
  expect(run()).toMatchObject({ profile: "g2b-contract-v1", screen: "01579" });
  fakeWindow.com = { gfnGetMenuNo: () => menu };

  // 목록에 없는 화면은 번호 칸이 읽혀도 받지 않는다 — 지금 보는 화면이 「수집 안 함」 으로 세우는 표시를 단다.
  menu = "01175";
  expect(refusal()).toMatchObject({ message: "메뉴 01175 화면은 수집하지 않습니다. 화면 자료는 엑셀로 내보낼 수 있습니다.", unsupported: true });
  // 번호를 읽지 못하면 번호 칸으로 짐작하지 않는다.
  menu = "메뉴"; fakeHistory.state = null;
  expect(refusal()).toMatchObject({ message: expect.stringContaining("화면 번호를 읽지 못해"), unsupported: true });
  // 목록은 프로필이 단 screen 에서 끌어낸다(ADR-037) — 공고 프로필이 화면을 내려놓으면 공고 화면도 받지 않는다.
  menu = "01179";
  const noNotice = profiles.map((p: any) => p.id === "g2b-notice-a-v1" ? { ...p, screen: undefined } : p);
  expect(refusal(noNotice)).toMatchObject({ message: "메뉴 01179 화면은 수집하지 않습니다. 화면 자료는 엑셀로 내보낼 수 있습니다.", unsupported: true });
  // 화면을 싣지 않는 옛 앱이면 어느 화면도 받지 않고 앱을 바꾸라고 한다.
  menu = "01579";
  const old = profiles.map(({ screen: _, ...p }: any) => p);
  expect(refusal(old)).toMatchObject({ message: expect.stringContaining("앱을 새 판으로 바꿔 주세요"), unsupported: true });

  // 목록의 화면인데 그 프로필의 번호를 읽지 못하면 평범한 오류다 — 다시 읽으면 된다.
  menu = "01179";
  delete values.bidPbancOrd; render();
  const missing = refusal();
  expect(missing.message).toBe("화면에서 번호를 읽지 못했습니다. 화면이 다 열린 뒤 다시 읽어 주세요.");
  expect(missing.unsupported).toBeFalsy();
});

it("확장은 hello 매핑의 프로필만을 지금 보는 화면 읽기에 넘기고, 탭 보고에 메뉴 번호를 싣는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  // 수집하는 화면은 프로필이 단 screen 이다 — 따로 목록을 싣지 않는다.
  const profiles = [{ id: "g2b-contract-v1", screen: { code: "01579", name: "계약 상세" } }];
  const original = w.native.getMockImplementation()!;
  w.native.mockImplementation(async (host: string, request: any) => request.method === "hello"
    ? { protocolVersion: 2, requestId: request.requestId, ok: true, result: { datasetId: "test", mapping: { profiles }, mappingRevision: "v1" } }
    : original(host, request));
  const g2b = [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a", title: "나라장터" },
    { id: 2, windowId: 10, url: "https://www.g2b.go.kr/b", title: "나라장터" },
    { id: 3, windowId: 10, url: "https://www.g2b.go.kr/c", title: "나라장터" }];
  w.chrome.tabs.query.mockImplementation(async (filter?: any) => filter?.active ? [g2b[0]] : g2b);
  const passed: any[] = [];
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => {
    if (args.files) return [];
    if (!args.args) return [{ result: "지문" }];
    passed.push(args.args);
    return [{ documentId: "doc" + args.target.tabId, result: args.target.tabId === 1
      ? { screen: "계약 상세", menu: "01579", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} }, scope: "live", rowMatches: {}, screen: "01579" } }
      : args.target.tabId === 2
        ? { screen: "개찰 결과", menu: "01175", error: "메뉴 01175 화면은 수집하지 않습니다. 화면 자료는 엑셀로 내보낼 수 있습니다.", unsupported: true }
        : { screen: "계약 상세", menu: "01579", error: "화면에서 번호를 읽지 못했습니다. 화면이 다 열린 뒤 다시 읽어 주세요.", unsupported: false } }];
  });
  await vi.advanceTimersByTimeAsync(0);
  await vi.advanceTimersByTimeAsync(300);
  const last = w.ports.flatMap(p => p.posted).filter(m => m.method === "tabs").at(-1)!;
  expect(last.params.tabs.map((t: any) => [t.tabId, t.state, t.menu])).toEqual(
    [[1, "supported", "01579"], [2, "unsupported", "01175"], [3, "error", "01579"]]);
  expect(last.params.tabs[0].snapshot).toMatchObject({ screen: "01579", mappingRevision: "v1" });
  expect(last.params.tabs[2].message).toBe("화면에서 번호를 읽지 못했습니다. 화면이 다 열린 뒤 다시 읽어 주세요.");
  expect(passed).toEqual([[profiles], [profiles], [profiles]]);
});

it("원값을 먼저 읽고, 숨은 칸의 차수를 쓰고, 부가 필드의 충돌은 그 필드만 비운다", () => {
  screen();
  Object.defineProperty(Element.prototype, "checkVisibility", { configurable: true, value(this: Element) { return !this.hasAttribute("data-hidden"); } });
  const contract = JSON.parse(mappingJson).profiles.find((p: any) => p.id === "g2b-contract-v1");
  const items = contract.tables.find((t: any) => t.required).source;
  document.body.innerHTML = `<div id="mf_wfm_container"><div class="grp"><div id="${items}"></div></div>
    <input id="mf_wfm_container_ibxCtrtNm" value="시험 계약">
    <input id="mf_wfm_container_calCtrtDt" value="2026/09/21"><span id="mf_wfm_container_spnCtrtDt"></span>
    <span id="mf_wfm_container_ctrtNoOrd" data-hidden></span>
    <span id="mf_wfm_container_dmstA"></span><span id="mf_wfm_container_dmstB"></span></div>`;
  const bound = (key: string, value: string) => ({ getRef: () => "data:dma_pointInfo." + key, getValue: () => value });
  const parts: Record<string, any> = {
    [items]: { getDataList: () => ({ getAllJSON: () => [{ ctrtNo: "R26TA00000001", ctrtChgOrd: "00", ctrtItemSqno: 1, ctrtAmt: 1000 }] }) },
    mf_wfm_container_ibxCtrtNm: bound("ctrtNm", "시험 계약"),
    mf_wfm_container_calCtrtDt: bound("ctrtDt", "20260921"), mf_wfm_container_spnCtrtDt: bound("ctrtDt", "20260921"),
    mf_wfm_container_ctrtNoOrd: bound("ctrtNoOrd", "R26TA00000001-00"),
    mf_wfm_container_dmstA: bound("dmstUntyGrpNm", "갑 기관"), mf_wfm_container_dmstB: bound("dmstUntyGrpNm", "을 기관"),
  };
  const fakeWindow: any = { com: { gfnGetMenuNo: () => "01579" }, $p: { getComponentById: (id: string) => parts[id] ?? null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", "profiles", captureSource + "; return pclmReadMapped(profiles);");
  const run = () => read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, [contract]);
  const result = run();
  expect(result.data.pointInfo).toMatchObject({ ctrtNm: "시험 계약", ctrtDt: "20260921", ctrtNoOrd: "R26TA00000001-00" });
  expect(result.data.pointInfo).not.toHaveProperty("dmstUntyGrpNm");
  parts.mf_wfm_container_dmstB = bound("ctrtNoOrd", "R26TA00000001-01");
  document.getElementById("mf_wfm_container_dmstB")!.setAttribute("data-hidden", "");
  expect(run).toThrow("필드가 여러 값으로 관찰됩니다: ctrtNoOrd"); // 문서 번호의 충돌은 막는다.
});

// 무압축 ZIP 을 풀어 항목마다 CRC 와 XML 을 확인한다.
async function unzip(bytes: Uint8Array) {
  const nodeVerifier = "node:zlib";
  const { crc32 } = await import(/* @vite-ignore */ nodeVerifier);
  const view = new DataView(bytes.buffer), decoder = new TextDecoder(), files = new Map<string, Document>();
  const end = bytes.length - 22;
  expect(view.getUint32(end, true)).toBe(0x06054b50);
  let at = view.getUint32(end + 16, true);
  for (let i = 0; i < view.getUint16(end + 10, true); i++) {
    expect(view.getUint32(at, true)).toBe(0x02014b50);
    const nameSize = view.getUint16(at + 28, true), size = view.getUint32(at + 24, true), local = view.getUint32(at + 42, true);
    const name = decoder.decode(bytes.slice(at + 46, at + 46 + nameSize));
    expect(view.getUint32(local, true)).toBe(0x04034b50);
    const start = local + 30 + view.getUint16(local + 26, true) + view.getUint16(local + 28, true);
    const data = bytes.slice(start, start + size);
    expect(crc32(data)).toBe(view.getUint32(at + 16, true));
    const xml = new DOMParser().parseFromString(decoder.decode(data), "application/xml");
    expect(xml.querySelector("parsererror")).toBeNull(); files.set(name, xml);
    at += 46 + nameSize + view.getUint16(at + 30, true) + view.getUint16(at + 32, true);
  }
  expect(at).toBe(end);
  return files;
}

it("엑셀 파일은 유효한 ZIP과 XML로 생성하고 번호·수식 원문·중복 시트 이름을 보존한다", async () => {
  const workbook = new Function(exportSource + "; return pclmWorkbook;")();
  const bytes: Uint8Array = workbook([
    { name: "업무/자료", rows: [["번호", "내용"], ["00123", '=HYPERLINK("https://example.invalid")'], ["한글 & <원문>", "_x0041_"]] },
    { name: "업무:자료", rows: [["번호"], ["002"]] },
  ]);
  const files = await unzip(bytes);
  expect([...files.get("xl/workbook.xml")!.querySelectorAll("sheet")].map(s => s.getAttribute("name"))).toEqual(["업무 자료", "업무 자료 2"]);
  const sheet = files.get("xl/worksheets/sheet1.xml")!;
  expect(sheet.querySelector('c[r="A2"]')!.textContent).toBe("00123");
  expect(sheet.querySelector('c[r="B2"]')!.getAttribute("t")).toBe("inlineStr");
  expect(sheet.querySelector('c[r="A3"]')!.textContent).toBe("한글 & <원문>");
  expect(sheet.querySelector('c[r="B3"]')!.textContent).toBe("_x005F_x0041_");
  expect(sheet.querySelector("f")).toBeNull();
  expect(files.has("[Content_Types].xml")).toBe(true);
  expect(() => workbook([{ name: "초과", rows: [["x".repeat(32768)]] }])).toThrow("너무 큽니다");
});

it("엑셀 열은 모든 값이 자릿점 숫자나 날짜일 때만 숫자·날짜로 쓰고, 번호·선행 0·섞인 열은 문자열로 두며 너비를 한글 폭으로 잰다", async () => {
  const workbook = new Function(exportSource + "; return pclmWorkbook;")();
  const columns: [string, string[]][] = [
    ["금액", ["1,234,567", "5", "0"]],
    ["원", ["1,000원", "20원", ""]],
    ["소수", ["1.5", "2,000.25", "3"]],
    ["코드", ["00123", "1", "2"]],
    ["일자번호", ["20260921", "20260922", "20260923"]],
    ["사업자", ["123-45-67890", "123-45-67891", "123-45-67892"]],
    ["섞임", ["1,234", "없음", "5"]],
    ["날짜", ["2026/09/21", "2026/09/22", ""]],
    ["일시", ["2026-09-21 10:30", "2026-09-22 08:00:15", "2026-09-23"]],
    ["없는날", ["2026/02/30", "2026/03/01", "2026/03/02"]],
    ["한글", ["가나다라마바사아자차", "", ""]],
    ["영문", ["abcdefghij", "", ""]],
    ["구분자", ["2026/09/21", "2026-09-22", "2026.09.23"]],
  ];
  const rows = [columns.map(([name]) => name), ...[0, 1, 2].map(r => columns.map(([, values]) => values[r]))];
  const files = await unzip(workbook([{ name: "형식", rows }]));
  const sheet = files.get("xl/worksheets/sheet1.xml")!, styles = files.get("xl/styles.xml")!;
  const cell = (ref: string) => sheet.querySelector(`c[r="${ref}"]`);
  const value = (ref: string) => Number(cell(ref)!.querySelector("v")!.textContent);
  const format = (ref: string) => {
    const xf = styles.querySelectorAll("cellXfs > xf")[Number(cell(ref)!.getAttribute("s"))];
    return styles.querySelector(`numFmt[numFmtId="${xf.getAttribute("numFmtId")}"]`)?.getAttribute("formatCode");
  };
  const day = (y: number, m: number, d: number) => (Date.UTC(y, m - 1, d) - Date.UTC(1899, 11, 30)) / 864e5;

  expect(["A2", "A3", "A4"].map(value)).toEqual([1234567, 5, 0]);
  expect(cell("A2")!.hasAttribute("t")).toBe(false);
  expect(format("A2")).toBe("#,##0");
  expect([value("B2"), value("B3"), format("B2")]).toEqual([1000, 20, '#,##0"원"']);
  expect(cell("B4")).toBeNull(); // 빈 칸은 쓰지 않는다.
  expect([value("C2"), value("C3"), format("C2")]).toEqual([1.5, 2000.25, "#,##0.00"]);
  for (const ref of ["D2", "E2", "F2", "G2", "G3", "J2", "J3", "M2"]) expect(cell(ref)!.getAttribute("t")).toBe("inlineStr");
  expect(cell("D2")!.textContent).toBe("00123");
  expect([value("H2"), format("H2")]).toEqual([day(2026, 9, 21), "yyyy\\/mm\\/dd"]);
  expect(value("I2")).toBeCloseTo(day(2026, 9, 21) + 10.5 / 24, 9);
  expect(value("I4")).toBe(day(2026, 9, 23));
  expect(format("I2")).toBe("yyyy\\-mm\\-dd hh:mm:ss");
  expect(cell("A1")!.getAttribute("t")).toBe("inlineStr"); // 머리글은 늘 문자열이다.

  const widths = [...sheet.querySelectorAll("cols > col")].map(c => Number(c.getAttribute("width")));
  expect(widths).toHaveLength(columns.length);
  expect(widths[10]).toBeGreaterThan(widths[11]);
  expect(widths[10]).toBe(22);
  expect(Math.min(...widths)).toBeGreaterThanOrEqual(8);
  expect(styles.querySelector("parsererror")).toBeNull();
  expect(styles.documentElement.firstElementChild!.tagName).toBe("numFmts");
  const xfs = styles.querySelector("cellXfs")!;
  expect(Number(xfs.getAttribute("count"))).toBe(xfs.children.length);
  expect(Number(styles.querySelector("numFmts")!.getAttribute("count"))).toBe(styles.querySelectorAll("numFmt").length);
});

it("화면 그대로: 수집 규칙의 칸마다 이름표(aria-label·앞 th·없음)와 보이는 글을, 품목은 줄마다 원값으로 읽는다", () => {
  screen();
  const contract = JSON.parse(mappingJson).profiles.find((p: any) => p.id === "g2b-contract-v1");
  const items = contract.tables.find((t: any) => t.required).source;
  document.body.innerHTML = `<h2 id="mf_wfm_cntsHeader_spnHeaderTitle">계약 상세</h2><div id="mf_wfm_container">
    <h3>계약 기본정보</h3>
    <table><tbody>
      <tr><th>계약번호</th><td><span id="mf_wfm_container_ctrtNoOrd">R26TA00000001-00</span></td></tr>
      <tr><th>계약명</th><td><input id="mf_wfm_container_ibxCtrtNm" value="시험 계약"></td></tr>
      <tr><th>무시할 머리</th><td><input id="mf_wfm_container_calCtrtDt" aria-label="계약일자" value="2026-09-21"></td></tr>
      <tr><td><select id="mf_wfm_container_selMthd"><option>일반</option><option selected>제한경쟁</option></select></td></tr>
    </tbody></table>
    <h3>품목내역</h3><div id="${items}"></div></div>`;
  const bound = (key: string, value: string) => ({ getRef: () => "data:dma_pointInfo." + key, getValue: () => value });
  const parts: Record<string, any> = {
    [items]: { getDataList: () => ({ getAllJSON: () => [
      { ctrtNo: "R26TA00000001", ctrtChgOrd: "00", ctrtItemSqno: 7, ctrtItemNm: "시험 품목", ctrtUntVal: "세트", ctrtQty: 6, ctrtAmt: 12000000, secret: "x" },
      { ctrtNo: "R26TA00000001", ctrtChgOrd: "00", ctrtItemSqno: 8, ctrtItemNm: "둘째 품목", ctrtQty: 1 }] }) },
    mf_wfm_container_ctrtNoOrd: bound("ctrtNoOrd", "R26TA00000001-00"),
    mf_wfm_container_ibxCtrtNm: bound("ctrtNm", "시험 계약"),
    mf_wfm_container_calCtrtDt: bound("ctrtDt", "20260921"),
    mf_wfm_container_selMthd: bound("ctrtMthdCd", "02"),
  };
  const fakeWindow: any = { com: { gfnGetMenuNo: () => "01579" }, $p: { getComponentById: (id: string) => parts[id] ?? null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", "profiles", captureSource + exportSource +
    "; const snapshot = pclmReadMapped(profiles); return pclmScreenRows(profiles[0], snapshot);");
  const rows = read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, [contract]);
  expect(rows).toEqual([
    { group: "계약 기본정보", label: "계약번호", text: "R26TA00000001-00", source: "ctrtNoOrd" },
    { group: "계약 기본정보", label: "계약명", text: "시험 계약", source: "ctrtNm" },
    // 칸에 붙은 이름표가 앞 th 보다 먼저다. 보이는 글은 화면의 서식 그대로(원값 20260921 이 아니다).
    { group: "계약 기본정보", label: "계약일자", text: "2026-09-21", source: "ctrtDt" },
    // 이름표가 없으면 빈 글. select 는 고른 글.
    { group: "계약 기본정보", label: "", text: "제한경쟁", source: "ctrtMthdCd" },
    { group: "품목내역", label: "7 시험 품목", text: "6세트 · 12000000", source: "table:contract_item:1" },
    { group: "품목내역", label: "8 둘째 품목", text: "1", source: "table:contract_item:2" },
  ]);
  expect(JSON.stringify(rows)).not.toContain("secret");

  // 수집 규칙에 없는 화면 — 내보내기의 화면 필드를 앞에서 40줄까지. 내보내기가 거르는 칸은 오지 않는다.
  document.body.innerHTML = `<input id="password" type="password" value="비밀">` +
    Array.from({ length: 50 }, (_, i) => `<input id="f${i}" title="칸 ${i}" value="값 ${i}">`).join("");
  const plain = new Function("document", "window", "location", captureSource + exportSource + "; return pclmScreenRows(null);");
  const unsupported = plain(document, fakeWindow, { origin: "https://www.g2b.go.kr", pathname: "/" });
  expect(unsupported).toHaveLength(40);
  expect(unsupported[0]).toEqual({ group: "", label: "칸 0", text: "값 0", source: "" });
  expect(JSON.stringify(unsupported)).not.toContain("비밀");
});

// ── 다리 확장(0.10): 저장은 창의 「가져오기」 만 한다 ──────────────────────────

it("확장은 나라장터 화면에 아무것도 심지 않는다 — 콘텐츠 스크립트·단축키·다시 넣기·저장 메시지가 없다", () => {
  const manifest = JSON.parse(manifestJson);
  for (const key of ["content_scripts", "web_accessible_resources", "commands"]) expect(manifest).not.toHaveProperty(key);
  expect(manifest.permissions).toEqual(["scripting", "nativeMessaging", "storage"]);
  expect(manifest.host_permissions).toEqual(["https://www.g2b.go.kr/*"]);
  expect(manifest.action).toEqual({ default_popup: "popup.html", default_title: "계약 목록" });
  const w = worker();
  expect(w.chrome.runtime.onInstalled.addListener).not.toHaveBeenCalled();
  expect(w.chrome.commands.onCommand.addListener).not.toHaveBeenCalled();
  const listener = w.chrome.runtime.onMessage.addListener.mock.lastCall![0];
  const reply = vi.fn();
  for (const type of ["pclm-capture", "pclm-shortcut", "pclm-options", "pclm-command-result"])
    expect(listener({ type, action: "save" }, popupSender, reply)).toBeUndefined();
  expect(reply).not.toHaveBeenCalled();
  expect(backgroundSource).not.toMatch(/pendingCapture|setBadgeText|panelMode|chrome\.commands|method: "event"/);
});

it("탭 보고의 설정은 사이트 접근 하나이고, 창의 명령 중 확장 관리만 열며 옛 설정 명령은 듣지 않는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  w.chrome.tabs.query.mockImplementation(async () => [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }]);
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : !args.args ? [{ result: "지문" }]
    : [{ result: { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } },
      screenRows: [{ group: "계약 기본정보", label: "계약명", text: "시험 계약", source: "ctrtNm" }] } }]);
  const reports = () => w.ports.flatMap(p => p.posted).filter(m => m.method === "tabs");
  await vi.advanceTimersByTimeAsync(300);
  const last = reports().at(-1)!;
  expect(last.params.settings).toEqual({ siteAccess: true });
  expect(w.chrome.permissions.contains).toHaveBeenCalledWith({ origins: ["https://www.g2b.go.kr/*"] });
  expect(last.params.tabs[0].screenRows).toEqual([{ group: "계약 기본정보", label: "계약명", text: "시험 계약", source: "ctrtNm" }]);

  // 페이지 수집기도 단축키도 없다 — 옛 창이 보내도 아무것도 하지 않는다.
  w.command({ protocolVersion: 2, command: "setPanelMode", mode: "always" });
  w.command({ protocolVersion: 2, command: "openShortcuts" });
  await vi.advanceTimersByTimeAsync(300);
  expect(w.chrome.storage.local.set).not.toHaveBeenCalled();
  expect(w.chrome.windows.create).not.toHaveBeenCalled();
  // 확장 관리 — 명령줄로는 chrome:// 를 열 수 없어 창이 확장에 맡긴다. 새 창으로 연다 — 앱이 새로 선 창을 알아보고 앞으로 가져온다.
  w.command({ protocolVersion: 2, command: "openExtensions" });
  expect(w.chrome.windows.create).toHaveBeenCalledWith({ url: "chrome://extensions/", focused: true });
  expect(w.chrome.tabs.create).not.toHaveBeenCalled();

  // 사이트 접근이 바뀌면(다음 확인에서) 다시 보고한다.
  w.chrome.permissions.contains.mockResolvedValue(false);
  await vi.advanceTimersByTimeAsync(2600);
  expect(reports().at(-1)!.params.settings).toEqual({ siteAccess: false });
});

it("호스트가 알려 준 디스크 판이 다르면 확장이 스스로 다시 불러오고, 같은 쌍으로 두 번은 하지 않는다", async () => {
  vi.useFakeTimers();
  Object.defineProperty(navigator, "userAgentData", { configurable: true, value: { brands: [{ brand: "Chromium" }, { brand: "Microsoft Edge" }] } });
  try {
    const w = worker();
    const original = w.native.getMockImplementation()!;
    let disk: string | undefined = "0.4.2";
    w.native.mockImplementation(async (host, request) => {
      const response = await original(host, request);
      if (request.method === "hello") response.result.extensionVersion = disk;
      return response;
    });
    w.chrome.tabs.query.mockImplementation(async () => [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }]);
    w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : !args.args ? [{ result: "지문" }]
      : [{ result: { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } } } }]);
    const tab = () => w.ports.flatMap(p => p.posted).filter(m => m.method === "tabs").at(-1)?.params.tabs[0];
    const reads = () => w.chrome.scripting.executeScript.mock.calls.filter(([args]: any) => args.args).length;

    await vi.advanceTimersByTimeAsync(0);
    expect(w.chrome.runtime.reload).not.toHaveBeenCalled(); // 보고가 먼저 나갈 틈을 준다.
    await vi.advanceTimersByTimeAsync(300);
    expect(tab()).toMatchObject({ state: "error", message: "확장을 새 판(0.4.2)으로 다시 불러옵니다. 잠시 뒤 화면을 다시 확인해 주세요." });
    expect(w.native.mock.calls.find(([, request]) => request.method === "hello")![1].params).toEqual({ extensionVersion: "0.4.1", browser: "Edge" });
    expect(reads()).toBe(0); // 옛 판으로는 화면을 읽지 않는다.
    expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);
    expect(w.local.reloadAttempt).toBe("0.4.1→0.4.2");

    // 다시 불러와도 판이 그대로면(다른 폴더에서 로드한 확장) 되풀이하지 않고 평소대로 읽는다.
    w.command({ protocolVersion: 2, command: "read", tabId: 1 });
    await vi.advanceTimersByTimeAsync(300);
    expect(tab()).toMatchObject({ state: "supported" });
    // 같은 판이거나 옛 호스트라 판을 모르면 다시 불러오지 않는다.
    for (const version of ["0.4.1", undefined]) {
      disk = version;
      await vi.advanceTimersByTimeAsync(61_000); // 쥐고 있던 hello 가 낡았다.
      w.command({ protocolVersion: 2, command: "read", tabId: 1 });
      await vi.advanceTimersByTimeAsync(300);
      expect(tab()).toMatchObject({ state: "supported" });
    }
    expect(w.native.mock.calls.filter(([, request]) => request.method === "hello")).toHaveLength(4);
    expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);
  } finally { delete (navigator as any).userAgentData; }
});

it("앱이 부른 내보내기는 콘텐츠 스크립트 없이 그 탭에 export.js 를 넣고, 팝업과 같은 이름 규칙으로 그 탭에서 내려받는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  w.chrome.tabs.query.mockImplementation(async () => [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }]);
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : !args.args ? [{ result: "지문" }]
    : [{ result: { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } } } }]);
  await vi.advanceTimersByTimeAsync(300);
  w.chrome.scripting.executeScript.mockClear();
  w.command({ protocolVersion: 2, command: "export", tabId: 1 });
  await vi.advanceTimersByTimeAsync(0);
  const calls = w.chrome.scripting.executeScript.mock.calls.map(([args]: any) => args);
  expect(calls[0]).toEqual({ target: { tabId: 1 }, world: "MAIN", files: ["export.js"] });
  expect(calls[1]).toMatchObject({ target: { tabId: 1 }, world: "MAIN" });
  expect(calls[1].args).toBeUndefined();
  expect(w.chrome.tabs.sendMessage).not.toHaveBeenCalled(); // 받을 콘텐츠 스크립트가 없다.
  // 확장이 모르는 탭은 내보내지 않는다.
  w.command({ protocolVersion: 2, command: "export", tabId: 7 });
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.scripting.executeScript).toHaveBeenCalledTimes(2);

  // 그 탭 안에서 도는 함수 — export.js 가 놓은 두 함수로 파일을 짓고 고른 이름으로 내려받는다.
  const page = globalThis as any;
  page.pclmExportPage = () => ({ title: "시험/화면", sheets: [{ name: "필드", rows: [["번호"], ["001"]] }], warnings: [] });
  page.pclmWorkbook = vi.fn(() => new Uint8Array([1, 2]));
  const create = vi.fn((_blob: Blob) => "blob:export"), revoke = vi.fn();
  Object.defineProperty(URL, "createObjectURL", { configurable: true, value: create });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: revoke });
  const download = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
    expect(this.download).toMatch(/^시험_화면_\d{4}-\d\d-\d\d\.xlsx$/);
    expect(this.href).toBe("blob:export");
  });
  try {
    calls[1].func();
    expect(download).toHaveBeenCalledTimes(1);
    expect(page.pclmWorkbook).toHaveBeenCalledWith([{ name: "필드", rows: [["번호"], ["001"]] }]);
    expect(document.querySelector("a")).toBeNull(); // 단 고리를 걷는다.
    await vi.advanceTimersByTimeAsync(30000);
    expect(revoke).toHaveBeenCalledWith("blob:export");
  } finally {
    delete page.pclmExportPage; delete page.pclmWorkbook;
    delete (URL as any).createObjectURL; delete (URL as any).revokeObjectURL;
  }
});

it("앱이 부른 내보내기의 결과는 그 탭의 보고에 실려 돌아간다 — 표가 없으면 그 까닭을, 지으면 성공을, 다시 읽어도 남긴다", async () => {
  vi.useFakeTimers();
  const w = worker();
  let page: any = { error: "내보낼 표가 없습니다." };
  w.chrome.tabs.query.mockImplementation(async () => [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }]);
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? []
    : String(args.func).includes("pclmExportPage") && !args.args ? [{ result: page }]
    : !args.args ? [{ result: "지문" }]
    : [{ result: { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } } } }]);
  const reported = () => w.ports.flatMap(p => p.posted).filter(m => m.method === "tabs").at(-1)!.params.tabs[0];
  await vi.advanceTimersByTimeAsync(300);
  expect(reported().export).toBeUndefined();

  w.command({ protocolVersion: 2, command: "export", tabId: 1 });
  await vi.advanceTimersByTimeAsync(300);
  expect(reported().export).toMatchObject({ ok: false, message: "내보낼 표가 없습니다." });
  const failedAt = reported().export.at;
  expect(typeof failedAt).toBe("number");

  page = { ok: true };
  await vi.advanceTimersByTimeAsync(5);
  w.command({ protocolVersion: 2, command: "export", tabId: 1 });
  await vi.advanceTimersByTimeAsync(300);
  expect(reported().export).toMatchObject({ ok: true, message: "" });
  expect(reported().export.at).toBeGreaterThan(failedAt);

  // 다시 읽어도 결과는 남는다 — 앱이 아직 보지 못했을 수 있다.
  w.command({ protocolVersion: 2, command: "read", tabId: 1 });
  await vi.advanceTimersByTimeAsync(300);
  expect(reported().export).toMatchObject({ ok: true });
});

it("팝업은 연결 상태와 이 탭을 「지금 보는 화면」 에서 어떻게 보는지를 알리고, 저장 단추는 두지 않는다", async () => {
  const open = async (status: any) => {
    popupBody();
    const chrome = { runtime: { openOptionsPage: vi.fn(), sendMessage: vi.fn(async (message: any) => {
      if (message.type !== "pclm-status") throw new Error("묻지 않을 것");
      if (status instanceof Error) throw status;
      return status;
    }) } };
    await runUi(chrome, document);
    return chrome;
  };
  const exportable = () => !(document.getElementById("export") as HTMLButtonElement).disabled;

  const chrome = await open({ connected: true, destination: "D:\\시험\\시험자료.pclm", environment: "development",
    tab: { g2b: true, state: "supported", screen: "계약 상세", kind: "g2b-contract-v1" } });
  expect(text("connection")).toBe("계약 목록과 이어져 있습니다");
  expect(text("destination")).toBe("작업자료 시험자료.pclm");
  expect(text("environment")).toBe("개발용");
  expect(text("tab")).toBe("이 화면은 계약 목록 창 오른쪽 「지금 보는 화면」에서 가져옵니다. 창이 닫혀 있으면 계약 목록을 여세요.");
  expect(exportable()).toBe(true);
  expect(text("export-status")).toBe("");
  // 저장·검토·단축키·도움말은 없다 — 가져오기는 앱이 한다.
  for (const id of ["action", "inspect", "changes", "shortcut", "help", "pin", "collapse"]) expect(document.getElementById(id)).toBeNull();
  expect(popupHtml).not.toMatch(/단축키|저장 대상|<details/);
  const settings = document.getElementById("settings")!;
  settings.click(); expect(chrome.runtime.openOptionsPage).not.toHaveBeenCalled();
  settings.onclick!.call(settings, { isTrusted: true } as MouseEvent);
  expect(chrome.runtime.openOptionsPage).toHaveBeenCalledTimes(1);

  await open({ connected: true, tab: { g2b: true, state: "unsupported", screen: "개찰 결과" } });
  expect(shown("destination")).toBe(false); // 작업자료를 아직 모른다.
  expect(text("environment")).toBe("");
  expect(text("tab")).toBe("이 화면은 수집하지 않습니다. 엑셀로는 내보낼 수 있습니다.");
  await open({ connected: true, tab: { g2b: true, state: "reading" } });
  expect(text("tab")).toBe("화면을 읽는 중입니다.");
  await open({ connected: true, tab: { g2b: true } }); // 거울이 아직 모르는 탭
  expect(text("tab")).toBe("화면을 읽는 중입니다.");
  await open({ connected: true, tab: { g2b: true, state: "error", message: "화면에서 번호를 읽지 못했습니다." } });
  expect(text("tab")).toBe("화면에서 번호를 읽지 못했습니다.");

  // 연결이 없으면 이 탭의 수집 여부는 뜻이 없다. 내보내기는 앱 없이도 된다.
  await open({ connected: false, tab: { g2b: true, state: "supported" } });
  expect(text("connection")).toBe("계약 목록에 연결되지 않았습니다");
  expect(text("destination")).toBe("계약 목록을 실행하면 자동으로 이어집니다.");
  expect(shown("tab")).toBe(false);
  expect(exportable()).toBe(true);

  // 개발 전환이 켜져 있으면 업무용 앱을 켜도 이어지지 않는다 — 그 까닭과 설정 단추를 가리킨다.
  await open({ connected: false, environment: "development", tab: { g2b: true, state: "supported" } });
  expect(text("connection")).toBe("계약 목록에 연결되지 않았습니다");
  expect(text("environment")).toBe("개발용");
  expect(text("destination")).toBe("확장 설정에서 「개발 DB 사용」 이 켜져 있어 개발용 계약 목록만 찾습니다. 업무용이면 설정에서 끄세요.");
  expect(shown("destination")).toBe(true);
  expect(document.getElementById("settings")!.textContent).toBe("설정");

  await open({ connected: true, destination: "D:\\시험\\시험자료.pclm", tab: { g2b: false } });
  expect(text("tab")).toBe("나라장터 화면이 아닙니다.");
  expect(exportable()).toBe(false);
  await open({ connected: false, tab: { g2b: false } });
  expect(text("tab")).toBe("나라장터 화면이 아닙니다.");

  await open(new Error("중계기 없음"));
  expect(text("connection")).toBe("계약 목록에 연결되지 않았습니다");
  expect(exportable()).toBe(false);
});

it("팝업은 열려 있는 동안 2초마다 상태를 다시 읽어 그리고, 내보내는 중에는 단추를 되살리지 않으며, 걷히면 멈춘다", async () => {
  vi.useFakeTimers();
  popupBody();
  let status: any = { connected: false, tab: { g2b: true, state: "supported" } };
  const sendMessage = vi.fn(async (message: any): Promise<any> => {
    if (message.type === "pclm-export") return new Promise(() => {}); // 끝나지 않는 내보내기
    return status;
  });
  await runUi({ runtime: { sendMessage, openOptionsPage: vi.fn() } }, document);
  expect(text("connection")).toBe("계약 목록에 연결되지 않았습니다");
  expect(shown("tab")).toBe(false);

  // 팝업을 연 뒤에 앱이 켜졌다 — 다시 열지 않아도 2초 안에 바뀐다.
  status = { connected: true, destination: "D:\\시험\\시험자료.pclm", tab: { g2b: true, state: "supported" } };
  await vi.advanceTimersByTimeAsync(2000);
  expect(text("connection")).toBe("계약 목록과 이어져 있습니다");
  expect(text("destination")).toBe("작업자료 시험자료.pclm");
  expect(text("tab")).toBe("이 화면은 계약 목록 창 오른쪽 「지금 보는 화면」에서 가져옵니다. 창이 닫혀 있으면 계약 목록을 여세요.");

  const button = document.getElementById("export") as HTMLButtonElement;
  void button.onclick!.call(button, { isTrusted: true } as MouseEvent);
  expect(button.disabled).toBe(true);
  await vi.advanceTimersByTimeAsync(2000);
  expect(button.disabled).toBe(true); // 다시 그려도 내보내는 중인 단추는 그대로다.

  // 다른 팝업이 같은 문서에 섰으면 옛 것은 묻지 않는다.
  popupBody();
  const asked = sendMessage.mock.calls.length;
  await vi.advanceTimersByTimeAsync(4000);
  expect(sendMessage.mock.calls.length).toBe(asked);
});

it("중계기는 팝업에 상시 포트·hello·거울이 아는 것만으로 상태를 답하고, 탭에서 온 물음에는 답하지 않는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  w.local.erpDevelopment = false;
  const original = w.native.getMockImplementation()!;
  w.native.mockImplementation(async (host, request) => {
    const response = await original(host, request);
    if (request.method === "hello") Object.assign(response.result, { destination: "C:\\시험\\시험자료.pclm", environment: "production" });
    return response;
  });
  let active: any = { id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" };
  w.chrome.tabs.query.mockImplementation(async (filter?: any) => filter?.active ? [active] : [active]);
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : !args.args ? [{ result: "지문" }]
    : [{ result: { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } } } }]);
  await vi.advanceTimersByTimeAsync(300);

  expect(await w.ask({ type: "pclm-status" })).toEqual({ connected: true, destination: "C:\\시험\\시험자료.pclm", environment: "production",
    tab: { g2b: true, state: "supported", screen: "계약 상세", kind: "g2b-contract-v1", message: undefined } });
  expect(await w.ask({ type: "pclm-status" }, { id: "extension", tab: { id: 1 }, url: "https://www.g2b.go.kr/a" })).toBeUndefined();
  expect(await w.ask({ type: "pclm-status" }, { id: "other", url: "chrome-extension://other/popup.html" })).toBeUndefined();
  const reads = w.chrome.scripting.executeScript.mock.calls.length;

  // 팝업이 실제로 그리는 모양.
  popupBody();
  await runUi({ runtime: { sendMessage: (message: object) => w.ask(message), openOptionsPage: vi.fn() } }, document);
  expect(text("connection")).toBe("계약 목록과 이어져 있습니다");
  expect(text("destination")).toBe("작업자료 시험자료.pclm");
  expect(text("tab")).toBe("이 화면은 계약 목록 창 오른쪽 「지금 보는 화면」에서 가져옵니다. 창이 닫혀 있으면 계약 목록을 여세요.");
  expect(w.chrome.scripting.executeScript).toHaveBeenCalledTimes(reads); // 팝업을 열었다고 다시 읽지 않는다.

  // 앱이 꺼지면 연결 안 됨이고, 옛 hello 의 작업자료는 보이지 않는다. 개발 전환은 저장된 값으로 안다.
  w.ports[0].drop();
  w.local.erpDevelopment = true;
  expect(await w.ask({ type: "pclm-status" })).toMatchObject({ connected: false, destination: undefined, environment: "development", tab: { g2b: true } });
  active = { id: 3, windowId: 10, url: "https://other.example/" };
  expect((await w.ask({ type: "pclm-status" })).tab).toEqual({ g2b: false });
});

it("팝업의 엑셀 내보내기는 앱 없이 중계기가 읽은 표로 파일을 지어 내려받고, 나라장터가 아니면 막는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  // 앱이 없다 — 상시 포트도 hello 도 서지 않는다.
  w.connectNative.mockImplementation(() => { throw new Error("호스트 없음"); });
  await vi.advanceTimersByTimeAsync(0);
  w.native.mockRejectedValue(new Error("호스트 없음"));
  const result = { title: "시험 화면", sheets: [{ name: "필드", rows: [["번호"], ["001"]] },
    { name: "수집 정보", rows: [["항목", "내용"], ["범위", "로딩된 자료"]] }], warnings: [] };
  let active: any = { id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" };
  w.chrome.tabs.query.mockImplementation(async (filter?: any) => filter?.active ? [active] : []);
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : [{ result: { workbook: result } }]);
  // 페이지에서 온 내보내기 요청은 받지 않는다.
  expect(await w.ask({ type: "pclm-export" }, { id: "extension", tab: { id: 1 }, url: "https://www.g2b.go.kr/a" })).toBeUndefined();

  popupBody();
  const create = vi.fn((_blob: Blob) => "blob:export"), revoke = vi.fn();
  Object.defineProperty(URL, "createObjectURL", { configurable: true, value: create });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: revoke });
  const download = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
    expect(this.download).toMatch(/^시험 화면_.*\.xlsx$/);
    expect(this.href).toBe("blob:export");
  });
  try {
    const chrome = { runtime: { sendMessage: vi.fn((message: object) => w.ask(message)), openOptionsPage: vi.fn() } };
    await runUi(chrome, document);
    expect(text("connection")).toBe("계약 목록에 연결되지 않았습니다");
    const button = document.getElementById("export") as HTMLButtonElement;
    expect(button.disabled).toBe(false);
    button.click(); // 합성 클릭은 내려받지 않는다.
    expect(download).not.toHaveBeenCalled();
    await button.onclick!.call(button, { isTrusted: true } as MouseEvent);
    expect(download).toHaveBeenCalledTimes(1);
    expect(create.mock.calls[0][0]).toBeInstanceOf(Blob);
    expect(text("export-status")).toBe("1개 자료 시트의 다운로드를 요청했습니다. 현재 로딩된 범위만 포함합니다.");
    expect(w.chrome.scripting.executeScript).toHaveBeenCalledWith({ target: { tabId: 1 }, world: "MAIN", files: ["export.js"] });
    expect(w.native).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(30000); expect(revoke).toHaveBeenCalledWith("blob:export");

    // 그 사이 나라장터가 아닌 탭으로 옮겼으면 막는다.
    active = { id: 3, windowId: 10, url: "https://other.example/" };
    await button.onclick!.call(button, { isTrusted: true } as MouseEvent);
    expect(text("export-status")).toBe("나라장터 업무 화면을 열어 주세요. 다시 시도해 주세요.");
    expect(button.disabled).toBe(false);
    chrome.runtime.sendMessage.mockRejectedValueOnce(new Error("연결 끊김"));
    await button.onclick!.call(button, { isTrusted: true } as MouseEvent);
    expect(text("export-status")).toContain("연결 끊김");
    expect(download).toHaveBeenCalledTimes(1);
  } finally { delete (URL as any).createObjectURL; delete (URL as any).revokeObjectURL; }
});

// 자료 자리는 확장에서 보지도 옮기지도 않는다. 통로가 저장 구조를 고치는 통로를 겸하면 브라우저 쪽의
// 실수 하나가 사람의 자료 자리를 바꾼다 — 자리는 계약 목록 앱에서만 다룬다.
it("확장은 설정 창에서도 자료 자리를 묻거나 옮기지 않는다", async () => {
  const w = worker();
  const optionsPage = { id: "extension", url: "chrome-extension://extension/options.html" };
  for (const action of ["location", "moveLocation"])
    expect(await w.ask({ type: "pclm-location", action, datasetId: "test", folder: "D:\\" }, optionsPage)).toBeUndefined();
  expect(w.native).not.toHaveBeenCalled();

  document.body.innerHTML = optionsHtml;
  const chrome = { storage: { local: { get: async () => ({}), set: vi.fn(async (_: object) => {}) } }, runtime: { sendMessage: vi.fn() } };
  await new Function("chrome", "document", optionsSource)(chrome, document);
  expect(document.getElementById("location")).toBeNull();
  // 설정 창에는 연결 환경 하나만 남았다.
  expect(document.querySelectorAll("fieldset")).toHaveLength(1);
  expect(optionsHtml).not.toMatch(/panelMode|단축키|수집기/);
  (document.getElementById("development") as HTMLInputElement).click();
  await waitFor(() => expect(chrome.storage.local.set).toHaveBeenCalledWith({ erpDevelopment: true }));
  expect(text("saved")).toBe("개발 DB에 저장합니다.");
  expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
});
