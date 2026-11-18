import { afterEach, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";
import mappingJson from "../../crates/pclm-core/src/erp/mapping.json?raw";
import captureSource from "../../extension/capture.js?raw";
import backgroundSource from "../../extension/background.js?raw";
import chipSource from "../../extension/chip.js?raw";
import exportSource from "../../extension/export.js?raw";
import manifestJson from "../../extension/manifest.json?raw";
import fakeG2bServer from "../../scripts/fake-g2b/server.mjs?raw";

afterEach(() => {
  document.body.replaceChildren();
  delete (Element.prototype as Partial<Element>).checkVisibility;
  vi.restoreAllMocks(); vi.useRealTimers(); vi.unstubAllGlobals();
});

/** 패널의 IndexedDB(store.js pclm-document) 대역 — background 는 settings 저장소의 me 만 읽는다. */
function panelStore(me: () => unknown) {
  const later = (req: any, result: () => unknown) => { queueMicrotask(() => { req.result = result(); req.onsuccess?.(); }); return req; };
  return { open: () => later({}, () => ({ close() {}, transaction: () => ({ objectStore: () => ({ get: () => later({}, me) }) }) })) };
}

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

const panelSender = { id: "extension", url: "chrome-extension://extension/panel/panel.html" };

const revisionA = "a".repeat(64), revisionB = "b".repeat(64), revisionC = "c".repeat(64);
function worker(id = "extension", revision = revisionA, local: Record<string, any> = {}) {
  let disk: string | null = revision;
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
      postMessage: (message: any) => {
        port.posted.push(message);
        if (message.method === "extensionUpdateCheck") queueMicrotask(() => port.onMessage.addListener.mock.lastCall![0]({
          protocolVersion: 2, requestId: message.requestId, ok: true, result: { expectedRevision: disk },
        }));
      },
      onMessage: { addListener: vi.fn() }, onDisconnect: { addListener: (fn: () => void) => listeners.push(fn) },
      disconnect: vi.fn(() => { port.closed = true; }),
      drop: () => { port.closed = true; listeners.forEach(fn => fn()); } };
    ports.push(port);
    return port;
  });
  const chrome = {
    runtime: { id, getURL: (file: string) => "chrome-extension://extension/" + file,
      sendNativeMessage: native, connectNative, lastError: undefined, onStartup: { addListener: vi.fn() },
      onMessage: { addListener: vi.fn() }, onInstalled: { addListener: vi.fn() },
      getManifest: () => ({ version: "0.4.1" }), reload: vi.fn(), getContexts: vi.fn(async (_?: object): Promise<any[]> => []) },
    tabs: { query: vi.fn(async (_?: object): Promise<any[]> => [{ id: 1 }]),
      onRemoved: { addListener: vi.fn() }, onActivated: { addListener: vi.fn() }, onUpdated: { addListener: vi.fn() },
      update: vi.fn(async (..._: any[]) => ({})), create: vi.fn(async (..._: any[]) => ({})),
      sendMessage: vi.fn(async (..._: any[]): Promise<any> => { throw new Error("수신자 없음"); }) },
    windows: { WINDOW_ID_NONE: -1, update: vi.fn(async (..._: any[]) => ({})), create: vi.fn(async (..._: any[]) => ({})), onFocusChanged: { addListener: vi.fn() } },
    // 옛 판이 쓰던 자리. 지금 판은 닿지 않아야 한다.
    commands: { getAll: vi.fn(async () => []), onCommand: { addListener: vi.fn() } },
    action: { setBadgeText: vi.fn(async (_: object) => {}), setBadgeBackgroundColor: vi.fn(async (_: object) => {}), openPopup: vi.fn(async () => {}) },
    permissions: { contains: vi.fn(async (_: object) => true), onAdded: { addListener: vi.fn() }, onRemoved: { addListener: vi.fn() } },
    sidePanel: { setPanelBehavior: vi.fn(async (_: object) => {}), open: vi.fn(async (_: object) => {}) },
    scripting: { executeScript: vi.fn(async (args: any): Promise<any[]> => args.files ? [] : [{ result: { error: "화면을 읽지 못했습니다." } }]) },
    storage: { onChanged: { addListener: vi.fn() }, local: { get: vi.fn(async (_?: unknown): Promise<Record<string, unknown>> => ({ ...local })),
      set: vi.fn(async (value: object) => { Object.assign(local, value); }) } },
  };
  new Function("chrome", backgroundSource.replace("__PCLM_BUNDLE_REVISION__", revision))(chrome);
  // 패널이 중계기에 묻는 길 — 받지 않으면(true 를 돌려주지 않으면) 답이 없다.
  const ask = (message: object, sender: object = panelSender): Promise<any> => new Promise(resolve => {
    if (chrome.runtime.onMessage.addListener.mock.lastCall![0](message, sender, resolve) !== true) resolve(undefined);
  });
  const command = (message: object) => ports[0].onMessage.addListener.mock.calls[0][0](message);
  const handshake = (expected: string | null) => {
    disk = expected;
    command({ protocolVersion: 2, requestId: ports[0].posted[0].requestId, ok: true, result: { expectedRevision: expected } });
  };
  const expectRevision = (expected: string | null) => { disk = expected; command({ protocolVersion: 2, command: "extensionUpdate", revision: expected }); };
  return { chrome, native, ports, connectNative, local, ask, command, handshake, expectRevision, setDisk: (expected: string | null) => { disk = expected; } };
}

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
  // 수집 시각은 지역 시각이다 — 한국 시각 아침 8시 반이 UTC 로는 전날이다.
  vi.useFakeTimers(); vi.setSystemTime(new Date(2026, 10, 14, 8, 30, 5));
  const result = read(document, fakeWindow, location);
  expect(result.sheets.map((s: any) => s.name)).toEqual(["화면 필드", "품목", "schedule", "수집 정보"]);
  expect(result.sheets[3].rows).toContainEqual(["수집 시각", "2026-11-14T08:30:05"]);
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

it("주민등록번호 칸은 이름표가 「주민번호」 이거나 id 가 jumin · ihidnum · rsdntNo 꼴이어도 내보내지 않는다", () => {
  screen();
  document.body.innerHTML = `<input id="ceoNm" title="대표자" value="홍길동"><input id="no1" title="주민번호" value="000000-0000001">
    <input id="txtJuminNo" title="번호" value="000000-0000002"><input id="ihidnum" title="식별" value="000000-0000003">
    <input id="rsdntRgstNo" title="등록" value="000000-0000004">`;
  const fakeWindow: any = { $p: { getComponentById: () => null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", exportSource + "; return pclmExportPage();");
  const result = read(document, fakeWindow, { origin: "https://www.g2b.go.kr", pathname: "/work" });
  expect(result.sheets[0].rows).toContainEqual(["대표자", "홍길동", "ceoNm", ""]);
  expect(JSON.stringify(result)).not.toMatch(/000000-000000\d/);
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

it("상시 연결은 포트 하나로 인사하고, 끊기면 물러서며 다시 잇는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  await vi.advanceTimersByTimeAsync(0);
  // 워커가 뜰 때와 브라우저 시작 알림이 겹쳐도 포트는 하나다.
  w.chrome.runtime.onStartup.addListener.mock.lastCall![0]();
  w.chrome.runtime.onInstalled.addListener.mock.lastCall![0]({ reason: "update" });
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
  // 압축 해제 확장의 reload 는 installed(update) 로도 깨운다. 물러서던 포트도 즉시 다시 잇는다.
  w.ports[4].drop();
  w.chrome.runtime.onInstalled.addListener.mock.lastCall![0]({ reason: "update" });
  await vi.advanceTimersByTimeAsync(0);
  expect(w.ports).toHaveLength(6);
  expect(w.ports[5].posted[0].method).toBe("presence");
  await vi.advanceTimersByTimeAsync(1000);
  expect(w.ports).toHaveLength(6);
});

it("확장은 제 ID 가 개발용일 때만 개발 호스트에 잇는다 — 상시 포트도 RPC 도", async () => {
  const development = /const DEVELOPMENT_ID = "([a-p]{32})"/.exec(backgroundSource)![1];
  for (const [id, host] of [[development, "kr.rfastball.pclm.erp.dev"], ["extension", "kr.rfastball.pclm.erp"]]) {
    const w = worker(id);
    await waitFor(() => expect(w.ports.map(p => p.host)).toEqual([host]));
    await w.ask({ type: "pclm-document", windowId: 10, op: "read" }, { ...panelSender, id });
    expect(w.native.mock.calls.map(([called]) => called)).toContain(host);
    expect(w.native.mock.calls.every(([called]) => called === host)).toBe(true);
  }
});

it("확장의 개발용 ID 는 가짜 나라장터가 복사본에 심는 ID 와 같다", () => {
  const development = /const DEVELOPMENT_ID = "([a-p]{32})"/.exec(backgroundSource)?.[1];
  const fake = /const DEV_ID = "([a-p]{32})"/.exec(fakeG2bServer)?.[1];
  expect(development).toBeTruthy();
  expect(development).toBe(fake);
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
    if (!args.args) return [{ result: { print: prints[tabId] } }]; // 매핑 원값 지문
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

it("자동 관찰은 DOM 표시가 같은 원값·표의 후속 로딩을 알아채고 화면을 조작하지 않는다", () => {
  vi.useFakeTimers(); vi.setSystemTime(10000); screen();
  const profile = JSON.parse(mappingJson).profiles.find((p: any) => p.id === "g2b-contract-v1");
  const grid = profile.tables[0].source;
  document.body.innerHTML = `<div id="mf_wfm_container"><span id="mf_wfm_container_ctrtNoOrd"></span><span id="mf_wfm_container_ctrtNm">변하지 않은 표시</span><div id="${grid}"></div><button type="button" role="tab" aria-selected="true">합성 탭</button></div>`;
  const values: Record<string, string> = { ctrtNoOrd: "R26TA99999999-00", ctrtNm: "합성 원값" };
  let rows: any[] = [];
  const fakeWindow: any = { com: { gfnGetMenuNo: () => "01579" }, $p: { getComponentById: (id: string) => id === grid
    ? { getDataList: () => ({ getAllJSON: () => rows }) }
    : { getRef: () => "data:dma_pointInfo." + id.replace("mf_wfm_container_", ""), getValue: () => values[id.replace("mf_wfm_container_", "")] } } };
  fakeWindow.top = fakeWindow;
  const observe = new Function("document", "window", "location", "profiles", captureSource + "; return pclmObserveMapped(profiles);");
  const run = () => observe(document, fakeWindow, { origin: "https://www.g2b.go.kr", href: "https://www.g2b.go.kr/synthetic" }, [profile]);
  const click = vi.spyOn(HTMLElement.prototype, "click"), focus = vi.spyOn(HTMLElement.prototype, "focus");
  const first = run(); expect(first.readiness.stable).toBe(false);
  vi.setSystemTime(11000); expect(run().readiness.stable).toBe(true);
  values.ctrtNm = "늦은 원값";
  expect(run()).toMatchObject({ snapshot: { data: { pointInfo: { ctrtNm: "늦은 원값" } } }, readiness: { stable: false } });
  vi.setSystemTime(12000); expect(run().readiness.stable).toBe(true);
  rows = [{ ctrtNo: "R26TA99999999", ctrtChgOrd: "00", ctrtItemSqno: 1, ctrtQty: 3 }];
  expect(run().readiness.stable).toBe(false);
  vi.setSystemTime(13000); expect(run().readiness.stable).toBe(true);
  document.querySelector("[role=tab]")!.setAttribute("aria-selected", "false");
  expect(run().readiness.stable).toBe(false);
  expect(run().readiness.documentId).toBe(first.readiness.documentId);
  expect(click).not.toHaveBeenCalled(); expect(focus).not.toHaveBeenCalled();
});

it("다시 읽는 동안 문서가 바뀌면 이전 응답을 버리고 새 문서에 고정한 읽기만 보고한다", async () => {
  vi.useFakeTimers();
  const w = worker();
  const tab = { id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" };
  w.chrome.tabs.query.mockResolvedValue([tab]);
  let generation = 1, release: (value: any) => void = () => {};
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => {
    if (args.files) return [{ documentId: "doc" + generation }];
    if (!args.args) return [];
    if (generation === 1) return await new Promise(resolve => { release = resolve; });
    return [{ documentId: "doc2", result: { screen: "새 문서", menu: "01579", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: { ctrtNm: "새 값" }, tables: {} } } } }];
  });
  await vi.advanceTimersByTimeAsync(0);
  generation = 2;
  const updated = w.chrome.tabs.onUpdated.addListener.mock.lastCall![0];
  updated(1, { status: "loading" }, tab); updated(1, { status: "complete" }, tab);
  release([{ documentId: "doc1", result: { screen: "옛 문서", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: { ctrtNm: "옛 값" }, tables: {} } } } }]);
  await vi.advanceTimersByTimeAsync(300);
  const reported = w.ports[0].posted.filter(m => m.method === "tabs").at(-1)!.params.tabs[0];
  expect(reported.screen).toBe("새 문서"); expect(reported.snapshot.data.pointInfo.ctrtNm).toBe("새 값");
  expect(w.chrome.scripting.executeScript.mock.calls.filter(([args]: any) => args.args).map(([args]: any) => args.target.documentIds)).toEqual([["doc1"], ["doc2"]]);
  expect(w.chrome.tabs.update).not.toHaveBeenCalled(); expect(w.chrome.windows.update).not.toHaveBeenCalled();
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

it("와일드카드가 여러 표에 맞으면 빈 표를 버리고, 고른 행이 같으면 앞의 표 하나로 담고, 다르면 담지 않는다", () => {
  screen();
  // 「미리보기」 창이 같은 첨부 표를 한 벌 더 싣는다. 전에는 표가 여럿이면 조용히 건너뛰어 첨부를 빠뜨렸다.
  const file = (sqno: number, name: string, status = "R") => ({ atchFileSqno: sqno, atchFileDscr: "공고서", orgnlAtchFileNm: name, rowStatus: status });
  const grids: Record<string, unknown[]> = {
    mf_wfm_container_tabEmpty_grdFile: [],
    mf_wfm_container_grdFile: [file(1, "시험 공고서.hwp")],
    // 고르지 않는 칸이 달라도 같은 표다.
    mf_wfm_container_preview_grdFile: [file(1, "시험 공고서.hwp", "U")],
  };
  const values: Record<string, string> = { bidPbancNo: "R26BK00000001", bidPbancOrd: "000" };
  document.body.innerHTML = `<div id="mf_wfm_container">${Object.keys(grids).map(id => `<div id="${id}"></div>`).join("")}${
    Object.keys(values).map(k => `<span id="mf_wfm_container_${k}"></span>`).join("")}</div>`;
  const fakeWindow: any = { com: { gfnGetMenuNo: () => "01179" }, $p: { getComponentById: (id: string) => {
    if (id in grids) return { getDataList: () => ({ getAllJSON: () => grids[id] }) };
    const key = id.replace("mf_wfm_container_", "");
    return key in values ? { getRef: () => "data:dma_pointInfo." + key, getValue: () => values[key] } : null;
  } } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", "profiles", captureSource + "; return pclmReadMapped(profiles);");
  const run = () => read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, JSON.parse(mappingJson).profiles);
  const files = (result: any) => Object.keys(result.data.tables).filter(k => k.endsWith("_grdFile"));

  const same = run();
  expect(files(same)).toEqual(["mf_wfm_container_grdFile"]);
  expect(same.data.tables.mf_wfm_container_grdFile).toEqual([{ atchFileSqno: 1, atchFileDscr: "공고서", orgnlAtchFileNm: "시험 공고서.hwp" }]);
  // 서로 다르면 어느 것이 대표인지 모른다 — 그 표만 담지 않고 나머지는 읽는다.
  grids.mf_wfm_container_preview_grdFile = [file(1, "다른 첨부.pdf")];
  const different = run();
  expect(files(different)).toEqual([]);
  expect(different).toMatchObject({ profile: "g2b-notice-a-v1", data: { pointInfo: values } });
  // 모두 비었으면 앞의 빈 표 하나다.
  grids.mf_wfm_container_grdFile = []; grids.mf_wfm_container_preview_grdFile = [];
  expect(run().data.tables).toEqual({ mf_wfm_container_tabEmpty_grdFile: [] });
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
  // 일반용역 공고 상세는 같은 칸을 읽는 다른 프로필이다 — 화면 하나에 프로필 하나.
  menu = "01181";
  expect(run()).toMatchObject({ profile: "g2b-notice-a-service-v1", screen: "01181", data: { pointInfo: { bidPbancNo: "R26BK00000001", bidPbancOrd: "000" } } });
  menu = "01179";
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

it("입찰공고작성[물품]의 게시일시는 업무 영역 밖에 붙은 「미리보기」 창에서 읽고, 닫힌 창의 칸은 읽지 않는다", () => {
  // 실제 화면에서 확인한 짜임 — 창은 #mf_wfm_container 밖에 서고 ID 머리만 그 영역의 것이다.
  Object.defineProperty(Element.prototype, "checkVisibility", { configurable: true, value(this: Element) { return !this.closest("[data-hidden]"); } });
  const notice = JSON.parse(mappingJson).profiles.find((p: any) => p.id === "g2b-notice-a-v1");
  const preview = "mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_";
  document.body.innerHTML = `<h2 id="mf_wfm_cntsHeader_spnHeaderTitle">입찰공고작성[물품]</h2>
    <div id="mf_wfm_container"><table><tbody>
      <tr><th>입찰공고번호</th><td><input id="mf_wfm_container_ibxBidPbancNo" value="R26BK00000001"></td></tr>
      <tr><td><input id="mf_wfm_container_ibxBidPbancOrd" value="000"></td></tr></tbody></table></div>
    <div id="mf_wfm_container_PNPE027_01"><table><tbody>
      <tr><th>게시일시</th><td><span id="${preview}spnPbancPstgDt">2026/09/01 10:30:00</span></td></tr></tbody></table></div>
    <div id="mf_wfm_container_PNPE027_00" data-hidden><span id="mf_wfm_container_PNPE027_00_spnPbancPstgDt"></span></div>`;
  const bound = (key: string, value: string) => ({ getRef: () => "data:dma_pointInfo." + key, getValue: () => value });
  const parts: Record<string, any> = {
    mf_wfm_container_ibxBidPbancNo: bound("bidPbancNo", "R26BK00000001"),
    mf_wfm_container_ibxBidPbancOrd: bound("bidPbancOrd", "000"),
    [preview + "spnPbancPstgDt"]: bound("pbancPstgDt", "2026/09/01 10:30:00"),
    // 닫힌 채 남은 창 — 다른 공고의 값이다. 열린 창이 없을 때도 이것을 집으면 안 된다.
    mf_wfm_container_PNPE027_00_spnPbancPstgDt: bound("pbancPstgDt", "2026/01/01 09:00:00"),
  };
  const fakeWindow: any = { com: { gfnGetMenuNo: () => "01179" }, $p: { getComponentById: (id: string) => parts[id] ?? null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", "profiles", captureSource + exportSource +
    "; const snapshot = pclmReadMapped(profiles); return { snapshot, rows: pclmScreenRows(profiles[0], snapshot) };");
  const run = () => read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, [notice]);

  const open = run();
  expect(open.snapshot.data.pointInfo).toMatchObject({ bidPbancNo: "R26BK00000001", bidPbancOrd: "000", pbancPstgDt: "2026/09/01 10:30:00" });
  // 「화면 그대로」 도 같은 칸을 짚는다.
  expect(open.rows).toContainEqual(expect.objectContaining({ label: "게시일시", text: "2026/09/01 10:30:00", source: "pbancPstgDt" }));

  document.getElementById("mf_wfm_container_PNPE027_01")!.setAttribute("data-hidden", "");
  const closed = run();
  expect(closed.snapshot.data.pointInfo).not.toHaveProperty("pbancPstgDt"); // 걷지 않은 칸 — 앱은 담긴 값을 그대로 둔다.
  expect(closed.rows.map((r: any) => r.source)).not.toContain("pbancPstgDt");
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
  document.body.innerHTML = `<h2 id="mf_wfm_cntsHeader_spnHeaderTitle">물품·일반용역 계약서</h2><div id="mf_wfm_container">
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

it("확장은 콘텐츠 스크립트·단축키·저장 메시지를 두지 않는다 — 페이지에 심는 것은 문서 만들기 단추 하나다", () => {
  const manifest = JSON.parse(manifestJson);
  for (const key of ["content_scripts", "web_accessible_resources", "commands"]) expect(manifest).not.toHaveProperty(key);
  expect(manifest.permissions).toEqual(["scripting", "nativeMessaging", "storage", "sidePanel", "downloads"]);
  expect(manifest.host_permissions).toEqual(["https://www.g2b.go.kr/*"]);
  expect(manifest.action).toEqual({ default_title: "계약 목록" });
  const w = worker();
  expect(w.chrome.commands.onCommand.addListener).not.toHaveBeenCalled();
  const listener = w.chrome.runtime.onMessage.addListener.mock.lastCall![0];
  const reply = vi.fn();
  for (const type of ["pclm-capture", "pclm-shortcut", "pclm-options", "pclm-command-result"])
    expect(listener({ type, action: "save" }, panelSender, reply)).toBeUndefined();
  expect(reply).not.toHaveBeenCalled();
  expect(backgroundSource).not.toMatch(/pendingCapture|setBadgeText|panelMode|chrome\.commands|method: "event"/);
});

it("탭 보고의 설정은 사이트 접근 하나이고, 창의 명령 중 확장 관리만 열며 옛 설정 명령은 듣지 않는다", async () => {
  vi.useFakeTimers();
  let me: unknown = { dept: "회계과", name: " 홍길동 ", phone: "000-0000-0000" };
  vi.stubGlobal("indexedDB", panelStore(() => me));
  const w = worker();
  w.chrome.tabs.query.mockImplementation(async () => [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }]);
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : !args.args ? [{ result: "지문" }]
    : [{ result: { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } },
      screenRows: [{ group: "계약 기본정보", label: "계약명", text: "시험 계약", source: "ctrtNm" }] } }]);
  const reports = () => w.ports.flatMap(p => p.posted).filter(m => m.method === "tabs");
  await vi.advanceTimersByTimeAsync(300);
  const last = reports().at(-1)!;
  expect(last.params.settings).toEqual({ siteAccess: true });
  // 패널이 열려 있지 않아도 내 정보의 이름(앞뒤 공백을 뗀 것) 하나만 싣는다 — 앱의 자동 수집이 이것으로 담당자를 가린다.
  expect(last.params.me).toEqual({ name: "홍길동" });
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
  // 패널에서 이름을 고치면(다음 확인에서) 다시 보고하고, 지우면 빈 이름이다.
  me = { name: "김수요" };
  await vi.advanceTimersByTimeAsync(2600);
  expect(reports().at(-1)!.params.me).toEqual({ name: "김수요" });
  me = undefined;
  await vi.advanceTimersByTimeAsync(2600);
  expect(reports().at(-1)!.params.me).toEqual({ name: "" });
});

it("화면 줄의 글이 호스트 상한(1000자)을 넘으면 잘라 실어 탭 보고 전체가 거절되지 않게 한다 — 서로게이트 짝을 가르지 않는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  w.chrome.tabs.query.mockImplementation(async () => [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }]);
  const long = "가".repeat(5000), pair = "a".repeat(999) + "😀b";
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : !args.args ? [{ result: "지문" }]
    : [{ result: { screen: "합성 화면", menu: "", error: "수집하지 않는 화면", unsupported: true,
      screenRows: [{ group: "", label: "긴 칸", text: long, source: "" }, { group: pair, label: "짝", text: "합성 값", source: "" }] } }]);
  await vi.advanceTimersByTimeAsync(300);
  const rows = w.ports.flatMap(p => p.posted).filter(m => m.method === "tabs").at(-1)!.params.tabs[0].screenRows;
  expect(rows).toEqual([{ group: "", label: "긴 칸", text: long.slice(0, 1000), source: "" }, { group: "a".repeat(999), label: "짝", text: "합성 값", source: "" }]);
  const lone = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;
  for (const row of rows) for (const v of Object.values(row) as string[]) { expect(v.length).toBeLessThanOrEqual(1000); expect(v).not.toMatch(lone); }
});

it("지문이 그대로인 앞 탭은 수집하지 않는 화면이어도 · 수집 규칙을 받지 못해도 2초마다 다시 읽지 않는다 — 60초에 한 번꼴이다", async () => {
  vi.useFakeTimers();
  const g2b = { id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" };
  // 수집하지 않는 화면: 지문은 그대로이고, 스냅샷이 없어 readiness.stable 은 늘 false 다.
  const w = worker();
  w.chrome.tabs.query.mockImplementation(async () => [g2b]);
  const captures = vi.fn(() => [{ result: { screen: "합성 화면", menu: "", error: "수집하지 않는 화면", unsupported: true, screenRows: [] } }]);
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : args.args ? captures()
    : [{ result: { print: "지문", readiness: { stable: false } } }]);
  await vi.advanceTimersByTimeAsync(300);
  expect(captures).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(10_000);
  expect(captures).toHaveBeenCalledTimes(1);

  // 앱 준비가 안 돼 hello 가 늘 실패한다: 다시 읽을 때마다 호스트를 띄우므로 2초마다 읽으면 안 된다.
  const failing = worker();
  failing.chrome.tabs.query.mockImplementation(async () => [g2b]);
  failing.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : [{ result: { print: "지문", readiness: { stable: false } } }]);
  failing.native.mockImplementation(async (_host: string, request: any): Promise<any> => ({ protocolVersion: 2, requestId: request.requestId, ok: false,
    error: { code: "setup_required", message: "준비 필요" } }));
  const hellos = () => failing.native.mock.calls.filter(([, r]) => r.method === "hello").length;
  await vi.advanceTimersByTimeAsync(300);
  expect(hellos()).toBe(1);
  await vi.advanceTimersByTimeAsync(10_000);
  expect(hellos()).toBe(1);
  await vi.advanceTimersByTimeAsync(52_000);
  expect(hellos()).toBe(2);
});

it("지문이 그대로인 앞 탭은 내용 보고(tabs)를 다시 보내지 않고 그 읽기의 번호(seq)와 새 readiness 만 박동(beat)으로 보낸다", async () => {
  vi.useFakeTimers();
  const w = worker();
  w.chrome.tabs.query.mockImplementation(async () => [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }]);
  const readiness = () => ({ stable: true, observedAt: Date.now(), documentId: "합성-문서" });
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : !args.args
    ? [{ result: { print: "지문", readiness: readiness() } }]
    : [{ result: { screen: "합성 화면", menu: "01579", print: "지문", readiness: readiness(),
      snapshot: { profile: "g2b-contract-v1", data: { pointInfo: { ctrtNm: "합성 값" }, tables: {} } } } }]);
  const posted = () => w.ports.flatMap(p => p.posted);
  const reports = () => posted().filter(m => m.method === "tabs");
  const beats = () => posted().filter(m => m.method === "beat");
  await vi.advanceTimersByTimeAsync(300);
  const first = reports().at(-1)!.params.tabs[0];
  expect(first.state).toBe("supported");
  expect(Number.isInteger(first.seq)).toBe(true);
  const sent = reports().length;

  await vi.advanceTimersByTimeAsync(4000);
  expect(reports()).toHaveLength(sent);
  expect(beats().length).toBeGreaterThanOrEqual(2);
  const [a, b] = beats().slice(-2);
  // 스냅샷 · 지문은 싣지 않는다 — 탭 번호 · 읽기 번호 · readiness 셋뿐이다.
  expect(b).toEqual({ protocolVersion: 2, requestId: expect.any(String), method: "beat",
    params: { tabId: 1, seq: first.seq, readiness: { stable: true, observedAt: expect.any(Number), documentId: "합성-문서" } } });
  expect(b.params.readiness.observedAt).toBeGreaterThan(a.params.readiness.observedAt);

  // 다시 읽으면 번호가 오르고, 그 뒤의 박동은 새 번호를 싣는다.
  w.command({ protocolVersion: 2, command: "read", tabId: 1 });
  await vi.advanceTimersByTimeAsync(300);
  const second = reports().at(-1)!.params.tabs[0];
  expect(second.seq).toBeGreaterThan(first.seq);
  await vi.advanceTimersByTimeAsync(2000);
  expect(beats().at(-1)!.params.seq).toBe(second.seq);
});

it("나라장터 탭 없이도 묶음 지문으로 갱신하며 같은 쌍은 한 번만 시도하고 새 실행 지문으로 확인한다", async () => {
  vi.useFakeTimers();
  const w = worker();
  w.chrome.tabs.query.mockResolvedValue([]);
  w.handshake(revisionB); // getManifest 의 버전 번호는 그대로다.
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);
  expect(w.local.reloadAttempt).toBe(`${revisionA}→${revisionB}`);
  expect(w.ports[0].posted[0].params.extensionRevision).toBe(revisionA);
  expect(w.native).not.toHaveBeenCalled(); // DB 를 여는 hello 는 필요 없다.

  const failed = worker("extension", revisionA, w.local); // 잘못된 폴더를 다시 불러온 경우
  failed.handshake(revisionB);
  await vi.advanceTimersByTimeAsync(0);
  expect(failed.chrome.runtime.reload).not.toHaveBeenCalled();
  expect(failed.ports[0].posted).toContainEqual(expect.objectContaining({ method: "extensionUpdateStatus", params: { state: "failed" } }));
  const next = worker("extension", revisionB, w.local);
  next.handshake(revisionB);
  await vi.advanceTimersByTimeAsync(0);
  expect(next.chrome.runtime.reload).not.toHaveBeenCalled();
  expect(next.local.reloadAttempt).toBeNull();
  expect(next.ports[0].posted).toContainEqual(expect.objectContaining({ method: "extensionUpdateStatus", params: { state: "current" } }));
});

it("열린 패널은 초안 보호를 위해 기다리고, 추출 중 완료 표시가 사라지면 취소한 뒤 최신 묶음만 적용한다", async () => {
  vi.useFakeTimers();
  const w = worker();
  let open = true;
  w.chrome.tabs.query.mockResolvedValue([]);
  w.chrome.runtime.getContexts.mockImplementation(async () => open ? [{ contextType: "SIDE_PANEL" }] : []);
  w.handshake(revisionB);
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.runtime.reload).not.toHaveBeenCalled();
  expect(w.ports[0].posted).toContainEqual(expect.objectContaining({ method: "extensionUpdateStatus", params: { state: "waiting-panel" } }));
  w.setDisk(null); // 호스트의 3초 알림 전에도 적용 직전 다시 확인한다.
  open = false;
  await vi.advanceTimersByTimeAsync(2000);
  expect(w.chrome.runtime.reload).not.toHaveBeenCalled();
  expect(w.local.reloadAttempt).toBeUndefined();
  expect(w.ports[0].posted).toContainEqual(expect.objectContaining({ method: "extensionUpdateStatus", params: { state: "preparing" } }));
  w.expectRevision(revisionC);
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);
  expect(w.local.reloadAttempt).toBe(`${revisionA}→${revisionC}`);
});

it("진행 중인 문서 요청은 마친 뒤에 갱신한다", async () => {
  vi.useFakeTimers();
  const w = worker();
  w.chrome.tabs.query.mockResolvedValue([]);
  let finish!: () => void;
  w.native.mockImplementation(async (_host, request) => {
    await new Promise<void>(resolve => { finish = resolve; });
    return { protocolVersion: 2, requestId: request.requestId, ok: true, result: {} };
  });
  const work = w.ask({ type: "pclm-document", windowId: 10, op: "templates", action: "list" });
  w.handshake(revisionB);
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.runtime.reload).not.toHaveBeenCalled();
  expect(w.ports[0].posted).toContainEqual(expect.objectContaining({ method: "extensionUpdateStatus", params: { state: "waiting-work" } }));
  finish(); await work;
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);
});

it.each(["작업 시작", "준비 취소", "패널 조회 실패"])("마지막 비동기 안전 확인 중 %s이면 시도 기록을 걷고 다시 확인한다", async change => {
  vi.useFakeTimers();
  const w = worker();
  w.chrome.tabs.query.mockResolvedValue([]);
  let calls = 0, finish!: () => void;
  let work: Promise<unknown> | undefined;
  w.native.mockImplementation(async (_host, request) => {
    await new Promise<void>(resolve => { finish = resolve; });
    return { protocolVersion: 2, requestId: request.requestId, ok: true, result: {} };
  });
  w.chrome.runtime.getContexts.mockImplementation(async () => {
    if (++calls === 3) {
      if (change === "작업 시작") work = w.ask({ type: "pclm-document", windowId: 10, op: "templates", action: "list" });
      else if (change === "준비 취소") w.expectRevision(null);
      else throw new Error("일시적인 조회 실패");
    }
    return [];
  });
  w.handshake(revisionB);
  await vi.advanceTimersByTimeAsync(0);
  expect(w.chrome.runtime.reload).not.toHaveBeenCalled();
  expect(w.local.reloadAttempt).toBeNull();
  if (work) { finish(); await work; }
  if (change === "준비 취소") w.expectRevision(revisionC);
  await vi.advanceTimersByTimeAsync(2000);
  expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);
  expect(w.local.reloadAttempt).toBe(`${revisionA}→${change === "준비 취소" ? revisionC : revisionB}`);
});

it("앱이 부른 내보내기는 콘텐츠 스크립트 없이 그 탭에 export.js 를 넣고, 「화면 제목_날짜.xlsx」 로 그 탭에서 내려받는다", async () => {
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
    expect(this.download).toBe("시험_화면_2026-11-14.xlsx"); // 지역 날짜 — UTC 로는 전날이다.
    expect(this.href).toBe("blob:export");
  });
  try {
    vi.setSystemTime(new Date(2026, 10, 14, 8, 30));
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

// 자료 자리는 확장에서 보지도 옮기지도 않는다. 통로가 저장 구조를 고치는 통로를 겸하면 브라우저 쪽의
// 실수 하나가 사람의 자료 자리를 바꾼다 — 자리는 계약 목록 앱에서만 다룬다.
it("확장은 패널에서도 자료 자리를 묻거나 옮기지 않는다", async () => {
  const w = worker();
  for (const action of ["location", "moveLocation"])
    expect(await w.ask({ type: "pclm-location", action, datasetId: "test", folder: "D:\\" })).toBeUndefined();
  expect(w.native).not.toHaveBeenCalled();
});

// ── 문서 만들기 패널(ADR-045 · ADR-046) ──────────────────────────────

it("툴바 단추는 곧장 패널을 열고, 엔진(wasm)은 CSP 가 허락하며, 팝업은 없다", () => {
  const manifest = JSON.parse(manifestJson);
  expect(manifest.side_panel).toEqual({ default_path: "panel/panel.html" });
  expect(manifest.content_security_policy).toEqual({ extension_pages: "script-src 'self' 'wasm-unsafe-eval'; object-src 'self'" });
  expect(manifest.minimum_chrome_version).toBe("116");
  expect(manifest).not.toHaveProperty("content_scripts");
  expect(manifest.action).not.toHaveProperty("default_popup");
  const w = worker();
  expect(w.chrome.sidePanel.setPanelBehavior).toHaveBeenCalledWith({ openPanelOnActionClick: true });
  // 팝업만 묻던 것은 이제 답하지 않는다.
  for (const type of ["pclm-status", "pclm-export"]) expect(w.chrome.runtime.onMessage.addListener.mock.lastCall![0]({ type }, panelSender, vi.fn())).toBeUndefined();
});

it("수집 화면으로 읽은 탭에 문서 만들기 단추를 심고, 수집 화면이 아니게 되거나 포트가 끊기면 뗀다", async () => {
  vi.useFakeTimers();
  const w = worker();
  const tab = { id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" };
  w.chrome.tabs.query.mockImplementation(async () => [tab]);
  let page: any = { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } } };
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files || args.func?.length === 0 && !args.world ? []
    : !args.args ? [{ result: "지문" }] : [{ result: page }]);
  const chips = () => w.chrome.scripting.executeScript.mock.calls.map(([args]: any) => args)
    .filter(a => a.files?.includes("chip.js") || (a.func && !a.world)).map(a => a.files ? "심기" : "떼기");
  await vi.advanceTimersByTimeAsync(300);
  // 맨 위 프레임 · 페이지와 갈린 세계(chrome.runtime 이 있어야 단추가 말을 보낸다).
  expect(w.chrome.scripting.executeScript).toHaveBeenCalledWith({ target: { tabId: 1 }, files: ["chip.js"] });
  expect(chips()).toEqual(["심기"]);
  page = { screen: "개찰 결과", unsupported: true, error: "수집하지 않는 화면입니다." };
  w.command({ protocolVersion: 2, command: "read", tabId: 1 });
  await vi.advanceTimersByTimeAsync(300);
  w.command({ protocolVersion: 2, command: "read", tabId: 1 });
  await vi.advanceTimersByTimeAsync(300);
  expect(chips()).toEqual(["심기", "떼기"]); // 심지 않은 탭에서는 다시 떼지 않는다.
  page = { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } } };
  w.command({ protocolVersion: 2, command: "read", tabId: 1 });
  await vi.advanceTimersByTimeAsync(300);
  w.ports[0].drop();
  expect(chips()).toEqual(["심기", "떼기", "심기", "떼기"]);

  // 뗄 때 그 탭에서 도는 함수.
  const remove = w.chrome.scripting.executeScript.mock.calls.map(([args]: any) => args).filter(a => a.func && !a.world).at(-1);
  document.body.innerHTML = `<div id="pclm-chip"></div>`;
  remove.func();
  expect(document.getElementById("pclm-chip")).toBeNull();
});

it("단추는 Shadow DOM 의 진짜 button 이고, 두 번 심어도 하나이며, 사람이 누를 때만 패널을 열어 달라고 한다", () => {
  let root: ShadowRoot | undefined;
  const attach = Element.prototype.attachShadow;
  vi.spyOn(Element.prototype, "attachShadow").mockImplementation(function (this: Element, init: ShadowRootInit) { return root = attach.call(this, init); });
  const sendMessage = vi.fn(async (_: object) => {});
  const inject = () => new Function("chrome", "document", chipSource)({ runtime: { sendMessage } }, document);
  // jsdom 의 isTrusted 는 고칠 수 없어, 단추에 단 손잡이를 받아 사람이 누른 것처럼 부른다.
  const handlers: EventListener[] = [];
  const listen = EventTarget.prototype.addEventListener;
  vi.spyOn(EventTarget.prototype, "addEventListener").mockImplementation(function (this: EventTarget, type: string, handler: any, options?: any) {
    if (type === "click") handlers.push(handler);
    return listen.call(this, type, handler, options);
  });
  inject(); inject();
  expect(handlers).toHaveLength(1);
  expect(document.querySelectorAll("#pclm-chip")).toHaveLength(1);
  const host = document.getElementById("pclm-chip")!;
  expect(host.style.getPropertyValue("position")).toBe("fixed");
  expect(host.style.getPropertyPriority("position")).toBe("important");
  const button = root!.querySelector("button")!;
  expect(button.textContent).toBe("문서 만들기");
  expect(root!.querySelector("style")!.textContent).toMatch(/@media print\s*\{\s*button\s*\{\s*display:\s*none/);
  expect(root!.querySelector("style")!.textContent).toMatch(/:focus-visible/);
  button.click(); // 합성 클릭(isTrusted 가 거짓)은 보내지 않는다.
  expect(sendMessage).not.toHaveBeenCalled();
  handlers[0]({ isTrusted: true } as Event);
  expect(sendMessage).toHaveBeenCalledWith({ type: "pclm-open-panel" });
});

it("탭에서 오는 말은 띄운 단추의 「패널 열기」 하나만, 확장 자신의 나라장터 탭에서 온 것만 받아 기다리지 않고 그 창의 패널을 연다", () => {
  const w = worker();
  const listener = w.chrome.runtime.onMessage.addListener.mock.lastCall![0];
  const g2b = { id: "extension", tab: { id: 5, windowId: 10, url: "https://www.g2b.go.kr/a" }, url: "https://www.g2b.go.kr/a" };
  // 사람 동작이 살아 있는 동안 불러야 열린다 — 리스너가 돌아오기 전에 이미 불렀다.
  expect(listener({ type: "pclm-open-panel" }, g2b, vi.fn())).toBeUndefined();
  expect(w.chrome.sidePanel.open).toHaveBeenCalledWith({ windowId: 10 });
  expect(w.chrome.sidePanel.open).toHaveBeenCalledTimes(1);
  // 다른 사이트 · 다른 확장 · 탭 번호가 없는 것은 열지 않는다.
  listener({ type: "pclm-open-panel" }, { ...g2b, tab: { ...g2b.tab, url: "https://other.example/" } }, vi.fn());
  listener({ type: "pclm-open-panel" }, { ...g2b, id: "other" }, vi.fn());
  listener({ type: "pclm-open-panel" }, { ...g2b, tab: { windowId: 10, url: g2b.tab.url } }, vi.fn());
  expect(w.chrome.sidePanel.open).toHaveBeenCalledTimes(1);
  // 그 밖의 말은 탭에서 오면 버린다 — 패널의 물음도.
  const reply = vi.fn();
  for (const type of ["pclm-document", "pclm-status", "pclm-export", "pclm-capture"])
    expect(listener({ type, windowId: 10, op: "check" }, g2b, reply)).toBeUndefined();
  expect(reply).not.toHaveBeenCalled();
  // 패널(탭이 아닌 곳)이 보낸 「패널 열기」 도 받지 않는다.
  listener({ type: "pclm-open-panel" }, panelSender, vi.fn());
  expect(w.chrome.sidePanel.open).toHaveBeenCalledTimes(1);
});

it("패널의 물음은 그 창의 활성 탭을 지금 보는 화면과 같은 길로 읽어 document 로 묻고, 같은 것을 다시 읽으면 알리지 않는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  const original = w.native.getMockImplementation()!;
  const asked: any[] = [];
  w.native.mockImplementation(async (host, request) => {
    if (request.method !== "document") return original(host, request);
    asked.push(request.params);
    const result = request.params.op === "read" ? { entityType: "notice", entity: "R26BK00000001-001", values: { 건명: "시험 공고" },
      rows: 1, older: false, caseKeys: ["notice:R26BK00000001"] }
      : { state: "differs", entity: "R26BK00000001-001", entityType: "notice", isNew: true, changedCount: 4, ...(request.params.op === "save" ? { saved: true } : {}) };
    return { protocolVersion: 2, requestId: request.requestId, ok: true, result };
  });
  const sent: any[] = [];
  (w.chrome.runtime as any).sendMessage = vi.fn(async (message: any) => { sent.push(message); });
  let active: any = { id: 5, windowId: 10, url: "https://www.g2b.go.kr/a", active: true };
  w.chrome.tabs.query.mockImplementation(async (filter?: any) => filter?.active ? [active] : [active]);
  const snapshot = { profile: "g2b-notice-v1", data: { pointInfo: { 공고: "1" }, tables: {} }, scope: "live" };
  const screen = (result: object) => w.chrome.scripting.executeScript.mockImplementation(async (args: any) =>
    args.files ? [] : !args.args ? [{ result: { print: "지문" } }] : [{ result }]);
  screen({ screen: "입찰공고 상세", snapshot });
  await vi.advanceTimersByTimeAsync(300);
  const panel = { id: "extension", url: "chrome-extension://extension/panel/panel.html" };

  // 패널이 아닌 곳(페이지)에서 온 물음은 받지 않는다.
  expect(await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, { id: "extension", tab: { id: 5 }, url: "https://www.g2b.go.kr/a" })).toBeUndefined();

  const check = await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel);
  expect(check).toEqual({ kind: "answer", tabId: 5, read: 1, answer: { state: "differs", entity: "R26BK00000001-001", entityType: "notice", isNew: true, changedCount: 4 } });
  expect(asked.at(-1)).toEqual({ datasetId: "test", op: "check", snapshot: { ...snapshot, mappingRevision: "v1" } });
  expect(w.chrome.tabs.query).toHaveBeenCalledWith({ active: true, windowId: 10 }); // 패널이 선 창의 활성 탭이다.
  // check 는 그 탭을 다시 읽는다 — 그러나 같은 결과면 패널에 알리지 않는다(알리면 패널이 다시 묻고 또 읽는다).
  const notified = sent.length;
  await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel);
  expect(sent.length).toBe(notified);

  // 저장은 읽어 둔 그 화면을 보낸다(사람이 본 것). 읽기는 check 가 돌려준 건으로 면 뷰를 묻는다.
  const reads = w.chrome.scripting.executeScript.mock.calls.length;
  expect((await w.ask({ type: "pclm-document", windowId: 10, op: "save", tabId: 5, read: check.read }, panel)).answer.saved).toBe(true);
  expect(w.chrome.scripting.executeScript.mock.calls.length).toBe(reads);
  expect(asked.at(-1)).toMatchObject({ op: "save", snapshot: { profile: "g2b-notice-v1" } });
  const read = await w.ask({ type: "pclm-document", windowId: 10, op: "read", entity: "R26BK00000001-001", entityType: "notice" }, panel);
  expect(read.answer.values).toEqual({ 건명: "시험 공고" });
  expect(asked.at(-1)).toEqual({ datasetId: "test", op: "read", entity: "R26BK00000001-001", entityType: "notice" });
  expect(await w.ask({ type: "pclm-document", windowId: 10, op: "delete" }, panel)).toMatchObject({ kind: "failed" });

  // 탭이 바뀌면 그 창의 패널에 알린다. 화면을 다시 읽어 달라졌을 때도.
  w.chrome.tabs.onActivated.addListener.mock.calls[0][0]({ tabId: 5, windowId: 10 });
  expect(sent.at(-1)).toEqual({ type: "pclm-document-changed", windowId: 10 });
  screen({ screen: "입찰공고 상세", snapshot: { ...snapshot, data: { pointInfo: { 공고: "2" }, tables: {} } } });
  sent.length = 0;
  await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel);
  expect(sent).toEqual([{ type: "pclm-document-changed", windowId: 10 }]);

  // 나라장터가 아니거나 수집하지 않는 화면이면 비운다.
  active = { id: 6, windowId: 10, url: "https://other.example/", active: true };
  expect(await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel)).toEqual({ kind: "elsewhere" });
  active = { id: 5, windowId: 10, url: "https://www.g2b.go.kr/a", active: true };
  screen({ screen: "개찰 결과", unsupported: true, error: "수집하지 않는 화면입니다." });
  expect(await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel)).toEqual({ kind: "elsewhere" });

  // 호스트가 거절하면 그 글을, 연결이 없으면 「연결 안 됨」 을 돌려준다.
  screen({ screen: "입찰공고 상세", snapshot });
  w.native.mockImplementation(async (host, request) => request.method !== "document" ? original(host, request)
    : { protocolVersion: 2, requestId: request.requestId, ok: false, error: { code: "setup_required", message: "준비" } } as any);
  expect(await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel)).toEqual({ kind: "failed", connection: true,
    message: "계약 목록 준비가 필요합니다. 계약 목록의 「Chrome 확장 상태」 를 확인해 주세요." });
  sent.length = 0;
  w.ports[0].drop();
  expect(sent).toEqual([{ type: "pclm-document-changed", windowId: null }]); // 끊기면 열린 패널이 모두 다시 묻는다.
  expect(await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel)).toEqual({ kind: "offline" });
});

it("패널을 연 채 느린 화면 읽기가 겹쳐도 같은 결과의 완료 알림은 다시 읽기를 끝없이 예약하지 않는다", async () => {
  vi.useFakeTimers();
  const w = worker();
  const tab = { id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" };
  w.chrome.tabs.query.mockResolvedValue([tab]);
  const snapshot = { profile: "g2b-notice-v1", data: { pointInfo: { 공고: "1" }, tables: {} }, scope: "live" };
  const captures = vi.fn(async () => {
    await new Promise(resolve => setTimeout(resolve, 400)); // 패널의 150ms 재확인보다 오래 걸린다.
    return [{ result: { screen: "입찰공고 상세", snapshot } }];
  });
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) =>
    args.files ? [] : args.args ? captures() : [{ result: { print: "지문" } }]);
  const original = w.native.getMockImplementation()!;
  w.native.mockImplementation(async (host, request) => request.method !== "document" ? original(host, request)
    : { protocolVersion: 2, requestId: request.requestId, ok: true, result: { state: "same" } });
  const replies: any[] = [];
  const check = async () => { replies.push(await w.ask({ type: "pclm-document", windowId: 10, op: "check" })); };
  (w.chrome.runtime as any).sendMessage = vi.fn(async (message: any) => {
    if (message.type === "pclm-document-changed") setTimeout(() => { void check(); }, 150);
  });
  await vi.advanceTimersByTimeAsync(0); // 앞 탭의 첫 읽기가 이미 진행 중이다.
  await check();
  expect(replies).toEqual([{ kind: "reading", tabId: 1 }]);
  await vi.advanceTimersByTimeAsync(1600);
  expect(replies.at(-1)).toMatchObject({ kind: "answer", answer: { state: "same" } });
  expect(captures).toHaveBeenCalledTimes(2); // 첫 읽기 완료 뒤의 재확인 한 번이면 끝난다.
});

it("패널이 본 뒤에 탭이 바뀌었거나 그 화면을 다시 읽어 달라졌으면 저장하지 않고 다시 확인하게 한다 — 저장은 사람이 본 그 화면이다", async () => {
  vi.useFakeTimers();
  const w = worker();
  const original = w.native.getMockImplementation()!;
  const saved: any[] = [];
  w.native.mockImplementation(async (host, request) => {
    if (request.method !== "document") return original(host, request);
    if (request.params.op === "save") saved.push(request.params.snapshot);
    return { protocolVersion: 2, requestId: request.requestId, ok: true,
      result: { state: "differs", entity: "R26BK00000001-001", entityType: "notice", isNew: true, changedCount: 1, saved: request.params.op === "save" } };
  });
  (w.chrome.runtime as any).sendMessage = vi.fn(async () => {});
  let active: any = { id: 5, windowId: 10, url: "https://www.g2b.go.kr/a", active: true };
  w.chrome.tabs.query.mockImplementation(async () => [active]);
  const snapshot = (n: string) => ({ profile: "g2b-notice-v1", data: { pointInfo: { 공고: n }, tables: {} }, scope: "live" });
  let shown = snapshot("A");
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) =>
    args.files ? [] : !args.args ? [{ result: { print: "지문" } }] : [{ result: { screen: "입찰공고 상세", snapshot: shown } }]);
  await vi.advanceTimersByTimeAsync(300);
  const panel = { id: "extension", url: "chrome-extension://extension/panel/panel.html" };
  const check = await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel);

  // 패널이 탭 5 의 다른 점을 보이는 사이 사람이 탭 7 로 옮겼다 — 탭 7 의 화면을 저장하지 않는다.
  shown = snapshot("B");
  active = { id: 7, windowId: 10, url: "https://www.g2b.go.kr/b", active: true };
  w.chrome.tabs.onActivated.addListener.mock.calls[0][0]({ tabId: 7, windowId: 10 });
  await vi.advanceTimersByTimeAsync(0);
  expect(await w.ask({ type: "pclm-document", windowId: 10, op: "save", tabId: check.tabId, read: check.read }, panel))
    .toEqual({ kind: "reading", tabId: 7 });
  expect(saved).toEqual([]);

  // 탭 5 로 돌아왔지만 그사이 화면이 다시 읽혀 달라졌다 — 그 새 화면도 저장하지 않는다.
  active = { id: 5, windowId: 10, url: "https://www.g2b.go.kr/a", active: true };
  w.chrome.tabs.onUpdated.addListener.mock.calls[0][0](5, { status: "complete" }, active);
  await vi.advanceTimersByTimeAsync(0);
  expect(await w.ask({ type: "pclm-document", windowId: 10, op: "save", tabId: check.tabId, read: check.read }, panel))
    .toEqual({ kind: "reading", tabId: 5 });
  expect(saved).toEqual([]);

  // 다시 확인한 것은 저장한다 — 그 화면 그대로.
  const again = await w.ask({ type: "pclm-document", windowId: 10, op: "check" }, panel);
  expect(again.read).not.toBe(check.read);
  expect((await w.ask({ type: "pclm-document", windowId: 10, op: "save", tabId: again.tabId, read: again.read }, panel)).answer.saved).toBe(true);
  expect(saved).toEqual([{ ...snapshot("B"), mappingRevision: "v1" }]);
});
