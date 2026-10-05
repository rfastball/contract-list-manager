import { afterEach, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";
import mappingJson from "../../src/Pclm.Core/Erp/mapping.json?raw";
import captureSource from "../../extension/capture.js?raw";
import popupSource from "../../extension/popup.js?raw";
import popupHtml from "../../extension/popup.html?raw";
import popupCss from "../../extension/popup.css?raw";
import backgroundSource from "../../extension/background.js?raw";
import exportSource from "../../extension/export.js?raw";
import optionsSource from "../../extension/options.js?raw";
import optionsHtml from "../../extension/options.html?raw";

afterEach(() => {
  window.dispatchEvent(new Event("pagehide"));
  document.getElementById("pclm-collector")?.remove();
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
  // 중계·UI 테스트용 화면 대역. 실제 추출은 아래 수집 테스트가 소유한다.
  return () => {
    const reference = document.querySelector<HTMLInputElement>('input[title="입찰공고번호"]')!.value.split(" - ");
    return { noticeBase: reference[0], seq: reference[1], fields: { title: { value: "시험 공고" } } };
  };
}

function worker() {
  const read = screen();
  let snapshot = read(), documentId = "doc1";
  const stored: Record<string, any> = {}, local: Record<string, any> = {};
  const native = vi.fn(async (_host: string, request: any) => {
    let result: any;
    if (request.method === "hello") result = { datasetId: "test", mapping: { profiles: [] }, mappingRevision: "v1" };
    if (request.method === "inspect") result = { baseToken: "v1", entity: snapshot.noticeBase + "-" + snapshot.seq, entityType: "notice", itemCount: 0, changes: [], unmatched: [],
      document: { rows: [{ values: Object.fromEntries(Object.entries(snapshot.fields).map(([key, field]: [string, any]) => [key, field.value])) }] } };
    if (request.method === "capture") result = { status: "stored", entity: "R26BK00000001-001", changed: true };
    if (request.method === "captureStatus") result = { status: "not_found" };
    return { protocolVersion: 2, requestId: request.requestId, ok: true, result };
  });
  const original = native.getMockImplementation()!;
  const nextCapture = (action: (host: string, request: any) => Promise<any>) => {
    native.mockImplementation(async (host, request) => {
      if (request.method !== "capture") return original(host, request);
      native.mockImplementation(original); return action(host, request);
    });
  };
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
    tabs: { get: async () => ({ url: "https://www.g2b.go.kr/" }), query: vi.fn(async (_?: object): Promise<any[]> => [{ id: 1 }]),
      onRemoved: { addListener: vi.fn() }, onActivated: { addListener: vi.fn() }, onUpdated: { addListener: vi.fn() },
      update: vi.fn(async (..._: any[]) => ({})), create: vi.fn(async (..._: any[]) => ({})),
      sendMessage: vi.fn(async (..._: any[]): Promise<any> => { throw new Error("수신자 없음"); }) },
    windows: { WINDOW_ID_NONE: -1, update: vi.fn(async (..._: any[]) => ({})), create: vi.fn(async (..._: any[]) => ({})), onFocusChanged: { addListener: vi.fn() } },
    commands: { getAll: async () => [{ name: "save-current", shortcut: "Alt+Shift+S" }], onCommand: { addListener: vi.fn() } },
    permissions: { contains: vi.fn(async (_: object) => true), onAdded: { addListener: vi.fn() }, onRemoved: { addListener: vi.fn() } },
    action: { setBadgeText: vi.fn(async (_: object) => {}), setBadgeBackgroundColor: vi.fn(async (_: object) => {}), openPopup: vi.fn(async () => {}) },
    scripting: { executeScript: vi.fn(async (args: any): Promise<any[]> => args.files ? [] : [{ documentId, result: { snapshot } }]) },
    storage: { onChanged: { addListener: vi.fn() }, local: { get: vi.fn(async (): Promise<Record<string, unknown>> => ({ ...local })),
      set: vi.fn(async (value: object) => { Object.assign(local, value); }) }, session: {
      get: vi.fn(async () => ({ ...stored })),
      set: vi.fn(async (value: object) => { Object.assign(stored, value); }),
      remove: vi.fn(async (keys: string[]) => { keys.forEach(key => delete stored[key]); }),
    } },
  };
  const sender = { id: "extension", url: "https://www.g2b.go.kr/", frameId: 0, documentId: "doc1", tab: { id: 1 } };
  const start = () => new Function("chrome", backgroundSource)(chrome);
  start();
  let preview: any;
  const send = (action: string, from: any = sender, captureId?: string): Promise<any> =>
    new Promise(resolve => chrome.runtime.onMessage.addListener.mock.lastCall![0]({ type: "pclm-capture", action, captureId, preview }, from,
      (view: any) => { preview = view.preview; resolve(view); }));
  return { chrome, sender, native, ports, connectNative, stored, local, start, send, nextCapture,
    change: () => { snapshot = { ...snapshot, seq: "002" }; },
    navigate: () => { documentId = "doc2"; } };
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
  expect(w.chrome.tabs.sendMessage).toHaveBeenLastCalledWith(1, { type: "pclm-export" }, { frameId: 0 });
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

it("앱이 부른 내보내기는 그 탭의 수집기가 단추와 같은 길로 내려받고, 다른 탭이 보낸 것은 듣지 않는다", async () => {
  screen();
  const messages: any[] = [];
  const result = { kind: "exported", title: "시험 화면", sheets: [{ name: "필드", rows: [["번호"], ["001"]] }], warnings: [] };
  const chrome = { storage: { local: { get: async () => ({ panelMode: "button" }) }, onChanged: { addListener: vi.fn() } },
    runtime: { id: "extension", getURL: (file: string) => file, onMessage: { addListener: (fn: any) => messages.push(fn) },
      sendMessage: vi.fn(async (message: any): Promise<any> => message.action === "export" ? result : "") } };
  await runUi(chrome, document, { origin: "https://www.g2b.go.kr" }, async () => ({ ok: false }));
  Object.defineProperty(URL, "createObjectURL", { configurable: true, value: vi.fn(() => "blob:export") });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: vi.fn() });
  const download = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
    expect(this.download).toMatch(/^시험 화면_.*\.xlsx$/);
  });
  const forged = vi.fn();
  expect(messages[0]({ type: "pclm-export" }, { id: "extension", tab: { id: 2 } }, forged)).toBeUndefined();
  expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
  const respond = vi.fn();
  expect(messages[0]({ type: "pclm-export" }, { id: "extension" }, respond)).toBe(true);
  await waitFor(() => expect(respond).toHaveBeenCalledWith(true));
  expect(chrome.runtime.sendMessage).toHaveBeenCalledWith({ type: "pclm-capture", action: "export" });
  expect(download).toHaveBeenCalledTimes(1);
  delete (URL as any).createObjectURL; delete (URL as any).revokeObjectURL;
});

it("중계기는 발신자와 확인한 화면을 검증하고 동시 클릭·세션 저장 실패·충돌을 차단한다", async () => {
  const w = worker();
  for (const sender of [{ ...w.sender, id: "other" }, { ...w.sender, frameId: 1 },
    { ...w.sender, url: "https://evil.example/" }]) expect((await w.send("save", sender)).kind).toBe("blocked");
  expect(w.native).not.toHaveBeenCalled();
  expect((await w.send("inspect")).kind).toBe("ready");
  expect(w.stored.pendingCapture).toBeUndefined();
  w.change();
  expect((await w.send("save")).kind).toBe("blocked");
  expect(w.native.mock.calls.some(([, request]) => request.method === "capture")).toBe(false);
  await w.send("inspect");
  w.chrome.storage.session.set.mockRejectedValueOnce(new Error("세션 저장 실패"));
  expect((await w.send("save")).kind).toBe("blocked");
  expect(w.stored.pendingCapture).toBeUndefined();
  await w.send("inspect");
  w.nextCapture(async (_host, request) => ({ protocolVersion: 2, requestId: request.requestId,
    ok: false, error: { code: "conflict", message: "자료가 바뀌었습니다." } } as any));
  expect((await w.send("save")).message).toContain("바뀌었습니다");
  expect(w.stored.pendingCapture).toBeUndefined();
  await w.send("inspect"); w.navigate();
  expect((await w.send("save")).kind).toBe("blocked");
  w.start(); // 서비스 워커 재시작 후에는 미리 확인하지 않은 자료를 저장하지 않는다.
  expect((await w.send("save", { ...w.sender, documentId: "doc2" })).kind).toBe("blocked");

  const other = worker();
  await other.send("inspect");
  other.start(); // 정상 미리보기는 워커 재시작 뒤에도 화면·토큰 재검증을 거쳐 저장할 수 있다.
  let finish!: (value: any) => void;
  other.nextCapture((_host, request) => new Promise(resolve => { finish = result =>
    resolve({ protocolVersion: 2, requestId: request.requestId, ok: true, result }); }));
  const saving = other.send("save");
  await waitFor(() => expect(finish).toBeDefined());
  expect((await other.send("save")).kind).toBe("busy");
  finish({ status: "stored", entity: "R26BK00000001-001", changed: false });
  expect((await saving).message).toContain("이미 같은");
  expect(other.native.mock.calls.filter(([, request]) => request.method === "capture")).toHaveLength(1);
});

it("응답 단절 후 다른 탭·재시작에서도 결과부터 조회하고 같은 미확인 요청만 명시적으로 재전송한다", async () => {
  const w = worker();
  await w.send("inspect");
  w.nextCapture(async () => { throw new Error("연결 끊김"); });
  const unknown = await w.send("save");
  expect(unknown.kind).toBe("pending");
  const original = structuredClone(w.stored.pendingCapture);
  w.start(); w.change();
  w.chrome.storage.local.get.mockResolvedValue({ erpDevelopment: true });
  const second = { ...w.sender, tab: { id: 2 } };
  expect(await w.send("inspect", second)).toMatchObject({ kind: "retry", previous: true });
  expect(w.stored.pendingCapture).toEqual(original);
  expect(w.native.mock.lastCall![0]).toBe("kr.rfastball.pclm.erp");
  expect(w.native.mock.calls.filter(([, request]) => request.method === "capture")).toHaveLength(1);
  await w.send("retry", second, "stale-request");
  expect(w.native.mock.calls.filter(([, request]) => request.method === "capture")).toHaveLength(1);
  w.native.mockRejectedValueOnce(new Error("다시 끊김"));
  expect((await w.send("retry", second, original.captureId)).kind).toBe("pending");
  expect(w.native.mock.lastCall![1].params).toEqual(original);
  expect(w.stored.pendingRetryAllowed).toBe(false);
  w.native.mockImplementationOnce(async (_host, request) => ({ protocolVersion: 2, requestId: request.requestId,
    ok: true, result: { status: "unknown" } }));
  expect((await w.send("recover", second)).kind).toBe("pending");
  expect(w.stored.pendingRetryAllowed).toBe(false);
  w.native.mockImplementationOnce(async (_host, request) => ({ protocolVersion: 2, requestId: request.requestId,
    ok: true, result: { status: "stored", entity: original.snapshot.noticeBase + "-" + original.snapshot.seq } }));
  expect((await w.send("recover", second)).kind).toBe("stored");
  expect(w.stored.pendingCapture).toBeUndefined();
});

it("호스트가 알려 준 디스크 판이 다르면 확장이 스스로 다시 불러오고, 같은 쌍으로 두 번은 하지 않는다", async () => {
  vi.useFakeTimers({ toFake: ["setTimeout"] });
  const w = worker();
  const original = w.native.getMockImplementation()!;
  let disk: string | undefined = "0.4.2";
  w.native.mockImplementation(async (host, request) => {
    const response = await original(host, request);
    if (request.method === "hello") response.result.extensionVersion = disk;
    return response;
  });
  Object.defineProperty(navigator, "userAgentData", { configurable: true, value: { brands: [{ brand: "Chromium" }, { brand: "Microsoft Edge" }] } });
  try {
    const first = await w.send("inspect");
    expect(first).toMatchObject({ kind: "blocked", message: "확장을 새 판(0.4.2)으로 다시 불러옵니다. 잠시 뒤 화면을 다시 확인해 주세요." });
    expect(w.native.mock.calls.find(([, request]) => request.method === "hello")![1].params).toEqual({ extensionVersion: "0.4.1", browser: "Edge" });
    expect(w.native.mock.calls.some(([, request]) => request.method === "inspect")).toBe(false);
    expect(w.chrome.runtime.reload).not.toHaveBeenCalled(); // 응답이 먼저 나간다.
    await vi.advanceTimersByTimeAsync(1000);
    expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);
    expect(w.local.reloadAttempt).toBe("0.4.1→0.4.2");

    // 다시 불러와도 판이 그대로면(다른 폴더에서 로드한 확장) 되풀이하지 않고 평소대로 간다.
    expect((await w.send("inspect")).kind).toBe("ready");
    await vi.advanceTimersByTimeAsync(1000);
    expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);

    // 같은 판이거나 옛 호스트라 판을 모르면 다시 불러오지 않는다.
    for (const version of ["0.4.1", undefined]) {
      disk = version;
      expect((await w.send("inspect")).kind).toBe("ready");
    }
    await vi.advanceTimersByTimeAsync(1000);
    expect(w.chrome.runtime.reload).toHaveBeenCalledTimes(1);
  } finally { delete (navigator as any).userAgentData; }
});

const runUi = new Function("chrome", "document", "location", "fetch", exportSource + "; return " + popupSource);
it("팝업은 자동 확인 뒤 한 번의 저장으로 대상을 유지하고 오류 종류에 맞는 복구 행동을 표시한다", async () => {
  const w = worker();
  const chrome = { storage: w.chrome.storage, commands: w.chrome.commands, runtime: { sendMessage: vi.fn(({ action, captureId }) => w.send(action,
    { id: "extension", url: w.chrome.runtime.getURL("popup.html") }, captureId)), openOptionsPage: vi.fn() } };
  document.body.innerHTML = popupHtml.match(/<body>([\s\S]*)<\/body>/)![1];
  await runUi(chrome, document, { origin: "chrome-extension://extension" });
  const button = () => document.getElementById("action") as HTMLButtonElement;
  const click = () => button().onclick!.call(button(), { isTrusted: true } as MouseEvent);
  expect(button().textContent).toBe("검토한 자료 저장");
  expect(document.getElementById("title")!.textContent).toBe("시험 공고");
  expect(document.getElementById("shortcut")!.textContent).toContain("Alt+Shift+S");
  expect(document.getElementById("development")).toBeNull(); // 연결 환경은 설정 창으로 옮겼다.
  // 툴바 팝업은 간결한 모양이다 — 내보내기 안내는 누르기 전엔 비어 있고, 폭은 뷰포트에 눌리지 않는다.
  expect(document.getElementById("collector")!.dataset.surface).toBe("popup");
  expect(document.getElementById("export-status")!.textContent).toBe("");
  expect(popupCss).not.toMatch(/body\s*\{[^}]*max-width/);
  const settings = document.getElementById("settings")!;
  settings.click(); expect(chrome.runtime.openOptionsPage).not.toHaveBeenCalled();
  settings.onclick!.call(settings, { isTrusted: true } as MouseEvent);
  expect(chrome.runtime.openOptionsPage).toHaveBeenCalledTimes(1);
  expect(w.native.mock.calls.some(([, request]) => request.method === "capture")).toBe(false);
  button().click(); // ERP가 만든 합성 클릭은 저장 동작이 아니다.
  expect(chrome.runtime.sendMessage).toHaveBeenCalledTimes(1);
  w.nextCapture(async () => { throw new Error("연결 끊김"); });
  click();
  await waitFor(() => expect(button().textContent).toBe("저장 결과 확인"));
  expect(document.getElementById("help")!.hidden).toBe(false);
  expect(document.getElementById("title")!.textContent).toBe("시험 공고");
  click();
  await waitFor(() => expect(button().textContent).toBe("같은 요청 다시 저장"));
  click();
  await waitFor(() => expect(document.getElementById("status")!.textContent).toContain("저장했습니다"));
  expect(document.getElementById("title")!.textContent).toBe("시험 공고");
  expect(document.getElementById("help")!.hidden).toBe(true);
});

it("페이지 수집기는 접기와 상세를 제공하고 SPA 전환·값 대입 뒤 이전 저장 상태를 무효화한다", async () => {
  vi.useFakeTimers();
  const read = screen();
  let changed = false;
  const chrome = { storage: { local: { get: async () => ({}) }, onChanged: { addListener: vi.fn() } },
    runtime: { id: "extension", getURL: (file: string) => file, onMessage: { addListener: vi.fn() }, sendMessage: vi.fn(async (message: any): Promise<any> => {
    if (message.type !== "pclm-capture") return "";
    let snapshot; try { snapshot = read(); } catch (e) { return { kind: "blocked", message: (e as Error).message }; }
    return { kind: "ready", message: "저장 대상 확인됨", entity: snapshot.noticeBase + "-" + snapshot.seq,
      fields: { title: changed ? "다음 공고" : "시험 공고" }, fingerprint: JSON.stringify(snapshot) };
  }) } };
  const fetchFile = async (file: string) => ({ ok: true, text: async () => file === "popup.html" ? popupHtml : popupCss });
  await runUi(chrome, document, { origin: "https://www.g2b.go.kr" }, fetchFile);
  const panel = document.getElementById("pclm-collector")!;
  const root = panel.shadowRoot!;
  expect(panel.hidden).toBe(false);
  expect(root.getElementById("title")!.textContent).toBe("시험 공고");
  expect(root.getElementById("collector")!.dataset.surface).toBeUndefined(); // 페이지 수집기는 다 보인다.
  root.getElementById("collapse")!.click();
  expect(root.getElementById("content")!.hidden).toBe(true);
  expect(root.getElementById("collapse")!.getAttribute("aria-expanded")).toBe("false");
  expect(root.getElementById("compact-status")!.textContent).toBe("저장 가능");
  root.getElementById("collapse")!.click();
  document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")!.textContent = "계약 상세";
  await vi.advanceTimersByTimeAsync(1000);
  expect(panel.hidden).toBe(false); // 계약 화면도 수집 대상이다.
  window.dispatchEvent(new Event("pagehide"));
  document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")!.textContent = "입찰공고상세[내자]";
  changed = true;
  document.querySelector<HTMLInputElement>('input[title="입찰공고번호"]')!.value = "R26BK00000002 - 001";
  window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true }));
  await vi.advanceTimersByTimeAsync(1000);
  expect(panel.hidden).toBe(false);
  expect(root.getElementById("entity")!.textContent).toContain("R26BK00000002-001");
  const calls = chrome.runtime.sendMessage.mock.calls.length;
  await vi.advanceTimersByTimeAsync(3000);
  expect(chrome.runtime.sendMessage).toHaveBeenCalledTimes(calls); // 같은 화면에는 연결 요청을 반복하지 않는다.
  document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")!.remove();
  await vi.advanceTimersByTimeAsync(1000);
  expect(root.getElementById("target")!.hidden).toBe(true);
  expect(panel.hidden).toBe(false);
  expect(root.getElementById("status")!.textContent).toContain("엑셀로 내보내세요");
});

it("「툴바 버튼으로만」이면 페이지 수집기도 관찰도 두지 않고, 설정을 바꾸면 열린 탭에 바로 반영한다", async () => {
  vi.useFakeTimers();
  screen();
  const changed: any[] = [], messages: any[] = [];
  const chrome = { storage: { local: { get: async () => ({ panelMode: "button" }) }, onChanged: { addListener: (fn: any) => changed.push(fn) } },
    runtime: { id: "extension", getURL: (file: string) => file, onMessage: { addListener: (fn: any) => messages.push(fn) },
      sendMessage: vi.fn(async (message: any): Promise<any> => message.type === "pclm-capture"
        ? { kind: "ready", message: "저장 대상 확인됨", entity: "R26BK00000001-001", fields: { title: "시험 공고" } } : "") } };
  const fetchFile = async (file: string) => ({ ok: true, text: async () => file === "popup.html" ? popupHtml : popupCss });
  await runUi(chrome, document, { origin: "https://www.g2b.go.kr" }, fetchFile);
  expect(document.getElementById("pclm-collector")).toBeNull();
  await vi.advanceTimersByTimeAsync(3000);
  expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
  const respond = vi.fn();
  const result = (view: object, sender: object = { id: "extension" }, reply = respond) =>
    messages[0]({ type: "pclm-command-result", view }, sender, reply);
  result({ kind: "stored", message: "검토한 자료를 저장했습니다.", entityType: "notice", entity: "R26BK00000001-001", fields: { title: "시험 공고" } });
  expect(respond).toHaveBeenLastCalledWith(false); // 패널이 없으면 중계기가 배지로 알린다.
  // 배지만으로는 저장됐는지 알기 어렵다. 패널이 없어도 화면에 크게 알린다.
  const toast = () => document.getElementById("pclm-toast")?.shadowRoot?.querySelector<HTMLElement>(".toast");
  expect(toast()!.className).toBe("toast ok");
  expect(toast()!.textContent).toContain("계약 목록에 저장했습니다");
  expect(toast()!.textContent).toContain("공고 · R26BK00000001-001 · 시험 공고");
  result({ kind: "ready", message: "검토가 필요합니다. 충돌한 변경값을 고른 뒤 저장하세요." });
  expect(document.querySelectorAll("#pclm-toast")).toHaveLength(1); // 새 결과가 옛 알림을 갈아 끼운다.
  expect(toast()!.className).toBe("toast review");
  expect(toast()!.textContent).toContain("충돌한 변경값");
  await vi.advanceTimersByTimeAsync(8000);
  expect(document.getElementById("pclm-toast")).toBeNull();

  changed[0]({ panelMode: { newValue: "always" } }, "local");
  await vi.waitFor(() => expect(document.getElementById("pclm-collector")?.shadowRoot?.getElementById("title")?.textContent).toBe("시험 공고"));
  const root = document.getElementById("pclm-collector")!.shadowRoot!;
  expect(root.getElementById("shortcut")!.textContent).toBe("단축키 없음 — 설정에서 지정");
  const forged = vi.fn();
  result({ kind: "blocked", message: "위조" }, { id: "extension", tab: { id: 2 } }, forged);
  expect(forged).not.toHaveBeenCalled();
  result({ kind: "stored", message: "검토한 자료를 저장했습니다.", entity: "R26BK00000001-001" });
  expect(respond).toHaveBeenLastCalledWith(true);
  expect(root.getElementById("status")!.textContent).toContain("저장했습니다");

  changed[0]({ panelMode: { newValue: "button" } }, "local");
  await vi.waitFor(() => expect(document.getElementById("pclm-collector")).toBeNull());
  const calls = chrome.runtime.sendMessage.mock.calls.length;
  document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")!.textContent = "계약 상세";
  await vi.advanceTimersByTimeAsync(3000);
  expect(chrome.runtime.sendMessage).toHaveBeenCalledTimes(calls); // 1초 관찰도 함께 멈췄다.
});

it("충돌은 「수집값 적용」 을 미리 고르되 빈 값으로 지우기는 유지를 고르고, 사람 값 복원과 행 삭제만 사람이 고르게 둔다", async () => {
  const conflicts = [
    { id: "c1", conflict: true, table: "notice", field: "title", before: "옛 제목", after: "새 제목" },
    { id: "c2", conflict: true, table: "notice", field: "__override", override: "공고명", before: "고친 값", after: "새 제목" },
    { id: "c3", conflict: true, table: "notice_item", line: 2, field: "__row", before: "지어낸 품목", after: null },
    { id: "c4", conflict: false, table: "notice", field: "posted_at", before: null, after: "2026/09/24" },
    // 늦게 그려진 탭을 빈 칸으로 읽었다. 한 번 누름에 멀쩡한 값을 지우지 않는다.
    { id: "c5", conflict: true, table: "notice", field: "award_method", before: "적격심사제", after: null },
    { id: "c6", conflict: true, table: "notice", field: "notice_agency", before: "시험기관", after: "" },
  ];
  const sendMessage = vi.fn(async (message: any): Promise<any> => message.action === "inspect"
    ? { kind: "ready", message: "검토가 필요합니다.", entity: "R26BK00000001-001", fields: { title: "시험 공고" }, changes: conflicts }
    : { kind: "stored", message: "검토한 자료를 저장했습니다." });
  const chrome = { storage: { local: { get: async () => ({}), set: vi.fn(async () => {}) } },
    commands: { getAll: async () => [] }, runtime: { sendMessage, openOptionsPage: vi.fn() } };
  document.body.innerHTML = popupHtml.match(/<body>([\s\S]*)<\/body>/)![1];
  await runUi(chrome, document, { origin: "chrome-extension://extension" });
  const selects = [...document.querySelectorAll<HTMLSelectElement>("#changes select")];
  expect(selects.map(s => s.value)).toEqual(["apply", "", "", "keep", "keep"]);
  // 덮개는 사람이 고친 값과 새로 읽은 값을 나란히 보인다.
  expect(document.getElementById("changes")!.textContent).toContain("notice 공고명: 정정값 고친 값 · 새 수집값 새 제목");
  const button = document.getElementById("action")!;
  button.onclick!.call(button, { isTrusted: true } as MouseEvent);
  await waitFor(() => expect(sendMessage).toHaveBeenLastCalledWith(expect.objectContaining({ action: "save", choices: { c1: "apply", c5: "keep", c6: "keep" } })));
});

it("페이지 수집기의 「숨기기」 는 툴바 버튼으로만 열기로 바꾸고, 툴바 팝업은 표시 방식을 오간다", async () => {
  vi.useFakeTimers();
  screen();
  const local: Record<string, any> = { panelMode: "always" }, changed: any[] = [];
  const storage = { local: { get: async () => ({ ...local }), set: vi.fn(async (value: object) => {
    const changes = Object.fromEntries(Object.entries(value).map(([key, newValue]) => [key, { newValue, oldValue: local[key] }]));
    Object.assign(local, value); changed.forEach(fn => fn(changes, "local"));
  }) }, onChanged: { addListener: (fn: any) => changed.push(fn) } };
  const chrome = { storage, runtime: { id: "extension", getURL: (file: string) => file, onMessage: { addListener: vi.fn() },
    sendMessage: vi.fn(async (message: any): Promise<any> => message.type === "pclm-capture"
      ? { kind: "ready", message: "저장 대상 확인됨", entity: "R26BK00000001-001", fields: { title: "시험 공고" } } : "") } };
  const fetchFile = async (file: string) => ({ ok: true, text: async () => file === "popup.html" ? popupHtml : popupCss });
  await runUi(chrome, document, { origin: "https://www.g2b.go.kr" }, fetchFile);
  const pin = document.getElementById("pclm-collector")!.shadowRoot!.getElementById("pin")!;
  expect(pin.hidden).toBe(false);
  expect(pin.textContent).toBe("숨기기");
  expect(pin.title).toContain("툴바의 확장 아이콘에서 다시 띄울 수 있습니다");
  pin.click(); // 페이지 스크립트가 만든 합성 클릭은 수집기를 치우지 못한다.
  expect(storage.local.set).not.toHaveBeenCalled();
  pin.onclick!.call(pin, { isTrusted: true } as MouseEvent);
  expect(local.panelMode).toBe("button");
  await vi.waitFor(() => expect(document.getElementById("pclm-collector")).toBeNull());
  vi.useRealTimers();
  changed.length = 0; // 그 탭은 닫았다 — 아래 전환이 수집기를 다시 세우지 않게 한다.

  // 툴바 팝업: 지금 값의 반대로 가는 버튼이다.
  document.body.innerHTML = popupHtml.match(/<body>([\s\S]*)<\/body>/)![1];
  const popup = { storage, commands: { getAll: async () => [] }, runtime: { openOptionsPage: vi.fn(),
    sendMessage: vi.fn(async () => ({ kind: "blocked", message: "현재 화면 확인 필요" })) } };
  await runUi(popup, document, { origin: "chrome-extension://extension" });
  const toggle = document.getElementById("pin")!;
  expect(toggle.textContent).toBe("화면에 항상 띄우기");
  await toggle.onclick!.call(toggle, { isTrusted: true } as MouseEvent);
  expect(local.panelMode).toBe("always");
  expect(toggle.textContent).toBe("화면에서 숨기기");
  expect(document.getElementById("status")!.textContent).toContain("띄웁니다");
  await toggle.onclick!.call(toggle, { isTrusted: true } as MouseEvent);
  expect(local.panelMode).toBe("button");
  expect(toggle.textContent).toBe("화면에 항상 띄우기");
});

it("확장을 다시 불러오면 열린 나라장터 탭에 수집기를 다시 넣고, 새 수집기가 끊긴 옛 것을 물러나게 한다", async () => {
  const w = worker();
  w.chrome.runtime.onInstalled.addListener.mock.lastCall![0]({ reason: "update" });
  await waitFor(() => expect(w.chrome.scripting.executeScript).toHaveBeenCalledWith(
    { target: { tabId: 1 }, files: ["capture.js", "export.js", "popup.js"] }));
  expect(w.chrome.tabs.query).toHaveBeenCalledWith({ url: "https://www.g2b.go.kr/*" });

  vi.useFakeTimers();
  screen();
  const instance = () => {
    const changed: any[] = [], messages: any[] = [];
    const chrome = { storage: { local: { get: async () => ({ panelMode: "always" }) }, onChanged: { addListener: (fn: any) => changed.push(fn) } },
      runtime: { id: "extension", getURL: (file: string) => file, onMessage: { addListener: (fn: any) => messages.push(fn) },
        sendMessage: vi.fn(async (message: any): Promise<any> => message.type === "pclm-capture"
          ? { kind: "ready", message: "저장 대상 확인됨", entity: "R26BK00000001-001", fields: { title: "시험 공고" } } : "") } };
    const fetchFile = async (file: string) => ({ ok: true, text: async () => file === "popup.html" ? popupHtml : popupCss });
    return { changed, messages, run: () => runUi(chrome, document, { origin: "https://www.g2b.go.kr" }, fetchFile) };
  };
  const old = instance(), fresh = instance();
  await old.run();
  const stale = document.getElementById("pclm-collector");
  expect(stale).not.toBeNull();
  await fresh.run();
  expect(document.querySelectorAll("#pclm-collector")).toHaveLength(1);
  expect(document.getElementById("pclm-collector")).not.toBe(stale);

  // 물러난 쪽은 설정 전환도 단축키 결과도 받지 않는다.
  old.changed[0]({ panelMode: { newValue: "button" } }, "local");
  const reply = vi.fn();
  old.messages[0]({ type: "pclm-command-result", view: { kind: "stored" } }, { id: "extension" }, reply);
  await vi.advanceTimersByTimeAsync(0);
  expect(document.getElementById("pclm-collector")).not.toBeNull();
  expect(reply).not.toHaveBeenCalled();
  fresh.changed[0]({ panelMode: { newValue: "button" } }, "local");
  await vi.waitFor(() => expect(document.getElementById("pclm-collector")).toBeNull());
});

it("단축키는 고를 것이 없을 때만 저장하고, 결과를 페이지 수집기나 탭 배지로 알린다", async () => {
  const w = worker();
  const g2b = { id: 1, url: "https://www.g2b.go.kr/" };
  const press = (tab: object = g2b) => w.chrome.commands.onCommand.addListener.mock.lastCall![0]("save-current", tab);
  const count = (method: string) => w.native.mock.calls.filter(([, request]) => request.method === method).length;
  const tabBadges = () => w.chrome.action.setBadgeText.mock.calls.map(([o]: any) => o).filter(o => o.tabId === 1 && o.text !== null);
  press();
  await waitFor(() => expect(tabBadges().at(-1)?.text).toBe("✓"));
  expect(count("capture")).toBe(1);
  expect(w.chrome.action.openPopup).not.toHaveBeenCalled();
  expect(w.chrome.tabs.sendMessage.mock.lastCall![2]).toEqual({ frameId: 0 });

  const original = w.native.getMockImplementation()!;
  w.native.mockImplementation(async (host, request) => {
    const response = await original(host, request);
    if (request.method === "inspect") response.result.changes = [{ id: "c1", table: "공고", field: "title", before: "갑", after: "을", conflict: true }];
    return response;
  });
  press();
  await waitFor(() => expect(w.chrome.action.openPopup).toHaveBeenCalledTimes(1));
  expect(tabBadges().at(-1)?.text).toBe("!");
  expect(count("capture")).toBe(1); // 충돌은 사람이 고른다.

  // 빈 값으로 지우는 변경만 남았으면 팝업의 기본과 같이 유지를 골라 저장한다 — 지우지 않는다.
  w.native.mockImplementation(async (host, request) => {
    const response = await original(host, request);
    if (request.method === "inspect") response.result.changes = [{ id: "w1", table: "공고", field: "award_method", before: "적격심사제", after: null, conflict: true }];
    return response;
  });
  press();
  await waitFor(() => expect(count("capture")).toBe(2));
  expect(w.native.mock.calls.filter(([, request]) => request.method === "capture").at(-1)![1].params.choices).toEqual({ w1: "keep" });
  w.native.mockImplementation(async (host, request) => {
    const response = await original(host, request);
    if (request.method === "inspect") response.result.changes = [{ id: "c1", table: "공고", field: "title", before: "갑", after: "을", conflict: true }];
    return response;
  });

  w.chrome.tabs.sendMessage.mockResolvedValueOnce(true); // 페이지 수집기가 떠 있으면 거기에 그린다.
  const badges = tabBadges().length;
  press();
  await waitFor(() => expect(w.chrome.tabs.sendMessage).toHaveBeenCalledTimes(4));
  expect(w.chrome.tabs.sendMessage.mock.lastCall![1].view).toMatchObject({ kind: "ready", message: expect.stringContaining("검토가 필요합니다") });
  await new Promise(resolve => setTimeout(resolve, 0));
  expect(tabBadges()).toHaveLength(badges);
  expect(count("capture")).toBe(2);

  const inspected = count("inspect");
  press({ id: 1, url: "https://other.example/" });
  await waitFor(() => expect(tabBadges()).toHaveLength(badges + 1));
  expect(tabBadges().at(-1)?.text).toBe("!");
  expect(count("inspect")).toBe(inspected);

  // 버튼 모드에서 미확인 저장이 생기면 강제로 띄우던 패널 대신 전역 배지로 남기고, 단축키도 저장하지 않는다.
  w.native.mockImplementation(original);
  w.chrome.storage.local.get.mockResolvedValue({ panelMode: "button" });
  await w.send("inspect");
  w.nextCapture(async () => { throw new Error("연결 끊김"); });
  expect((await w.send("save")).kind).toBe("pending");
  expect(w.chrome.action.setBadgeText).toHaveBeenLastCalledWith({ text: "!" });
  press();
  await waitFor(() => expect(w.chrome.action.openPopup).toHaveBeenCalledTimes(2));
  expect(count("capture")).toBe(3);
  expect(w.stored.pendingCapture).toBeDefined();
});


// 자료 자리는 확장에서 보지도 옮기지도 않는다. 수집 통로가 저장 구조를 고치는 통로를 겸하면 브라우저 쪽의
// 실수 하나가 사람의 자료 자리를 바꾼다 — 자리는 계약 목록 앱에서만 다룬다.
it("확장은 설정 창에서도 자료 자리를 묻거나 옮기지 않는다", async () => {
  const w = worker();
  const listener = w.chrome.runtime.onMessage.addListener.mock.lastCall![0];
  const optionsPage = { id: "extension", url: "chrome-extension://extension/options.html" };
  const replied = vi.fn();
  for (const action of ["location", "moveLocation"])
    expect(listener({ type: "pclm-location", action, datasetId: "test", folder: "D:\\" }, optionsPage, replied)).toBeUndefined();
  expect(w.native).not.toHaveBeenCalled();
  expect(replied).not.toHaveBeenCalled();

  document.body.innerHTML = optionsHtml;
  const chrome = { storage: { local: { get: async () => ({}), set: vi.fn(async () => {}) }, onChanged: { addListener: vi.fn() } },
    commands: { getAll: async () => [] }, tabs: { create: vi.fn() }, runtime: { sendMessage: vi.fn() } };
  await new Function("chrome", "document", optionsSource)(chrome, document);
  expect(document.getElementById("location")).toBeNull();
  (document.getElementById("development") as HTMLInputElement).click();
  await waitFor(() => expect(chrome.storage.local.set).toHaveBeenCalledWith({ erpDevelopment: true }));
  expect(chrome.runtime.sendMessage).not.toHaveBeenCalled();
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

it("확장은 hello 매핑의 프로필만을 수집과 지금 보는 화면 읽기에 넘기고, 탭 보고에 메뉴 번호를 싣는다", async () => {
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

  // 수집기의 확인도 같은 목록으로 읽고, 화면 번호가 실린 스냅샷을 호스트에 보낸다.
  passed.length = 0;
  const view = w.send("inspect");
  await vi.advanceTimersByTimeAsync(0);
  expect((await view).kind).toBe("ready");
  expect(passed).toEqual([[profiles]]);
  expect(w.native.mock.calls.find(([, r]) => r.method === "inspect")![1].params.snapshot).toMatchObject({ screen: "01579" });
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

it("엑셀 파일은 유효한 ZIP과 XML로 생성하고 번호·수식 원문·중복 시트 이름을 보존한다", async () => {
  const nodeVerifier = "node:zlib";
  const { crc32 } = await import(/* @vite-ignore */ nodeVerifier);
  const workbook = new Function(exportSource + "; return pclmWorkbook;")();
  const bytes: Uint8Array = workbook([
    { name: "업무/자료", rows: [["번호", "내용"], ["00123", '=HYPERLINK("https://example.invalid")'], ["한글 & <원문>", "_x0041_"]] },
    { name: "업무:자료", rows: [["번호"], ["002"]] },
  ]);
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

it("내보내기는 DB 연결과 미확인 저장에 의존하지 않으며 사용자 클릭으로 XLSX를 내려받는다", async () => {
  const w = worker();
  w.stored.pendingCapture = { captureId: "미확인 요청" };
  const result = { title: "시험 화면", sheets: [{ name: "필드", rows: [["번호"], ["001"]] },
    { name: "수집 정보", rows: [["항목", "내용"], ["범위", "로딩된 자료"]] }], warnings: [] };
  w.chrome.scripting.executeScript.mockImplementation(async args => args.files ? [] : [{ documentId: "doc1", result: { workbook: result } }] as any);
  expect(await w.send("export")).toMatchObject({ kind: "exported", ...result });
  expect(w.native).not.toHaveBeenCalled();
  expect(w.stored.pendingCapture).toEqual({ captureId: "미확인 요청" });
  expect((await w.send("export", { ...w.sender, documentId: "old" })).kind).toBe("blocked");
  expect((await w.send("export", { ...w.sender, url: "https://other.example" })).kind).toBe("blocked");
  w.chrome.tabs.get = async () => ({ url: "https://other.example/" });
  expect((await w.send("export", { id: "extension", url: w.chrome.runtime.getURL("popup.html") })).kind).toBe("blocked");

  document.body.innerHTML = popupHtml.match(/<body>([\s\S]*)<\/body>/)![1];
  const create = vi.fn((_blob: Blob) => "blob:export"), revoke = vi.fn();
  Object.defineProperty(URL, "createObjectURL", { configurable: true, value: create });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: revoke });
  const download = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
    expect(this.download).toMatch(/^시험 화면_.*\.xlsx$/);
    expect(this.href).toBe("blob:export");
  });
  const chrome = { storage: w.chrome.storage, commands: w.chrome.commands, runtime: { sendMessage: vi.fn(async ({ action }) => action === "export"
    ? { kind: "exported", ...result } : { kind: "blocked", message: "앱 연결 없음", connection: true }) } };
  await runUi(chrome, document, { origin: "chrome-extension://extension" });
  const button = document.getElementById("export") as HTMLButtonElement;
  button.click(); expect(download).not.toHaveBeenCalled();
  vi.useFakeTimers();
  await button.onclick!.call(button, { isTrusted: true } as MouseEvent);
  expect(download).toHaveBeenCalledTimes(1);
  expect(create.mock.calls[0][0]).toBeInstanceOf(Blob);
  expect(document.getElementById("export-status")!.textContent).toContain("다운로드를 요청했습니다");
  expect(document.getElementById("status")!.textContent).toBe("앱 연결 없음");
  await vi.advanceTimersByTimeAsync(30000); expect(revoke).toHaveBeenCalledWith("blob:export");
  chrome.runtime.sendMessage.mockRejectedValueOnce(new Error("연결 끊김"));
  await button.onclick!.call(button, { isTrusted: true } as MouseEvent);
  expect(document.getElementById("export-status")!.textContent).toContain("연결 끊김");
  expect(button.disabled).toBe(false);
  delete (URL as any).createObjectURL; delete (URL as any).revokeObjectURL;
});

// ── 지금 보는 화면 3b: 화면 이름표 · 확장 설정 · 오류 사건 ──────────────────

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

it("확장 설정을 탭 보고에 싣고, 창의 설정 명령을 옵션 창과 같은 자리에 쓰며 단축키 화면을 연다", async () => {
  vi.useFakeTimers();
  const w = worker();
  w.local.panelMode = "button";
  w.chrome.tabs.query.mockImplementation(async (filter?: any) => filter?.active ? [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }] : [{ id: 1, windowId: 10, url: "https://www.g2b.go.kr/a" }]);
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : !args.args ? [{ result: "지문" }]
    : [{ result: { screen: "계약 상세", snapshot: { profile: "g2b-contract-v1", data: { pointInfo: {}, tables: {} } },
      screenRows: [{ group: "계약 기본정보", label: "계약명", text: "시험 계약", source: "ctrtNm" }] } }]);
  const reports = () => w.ports.flatMap(p => p.posted).filter(m => m.method === "tabs");
  await vi.advanceTimersByTimeAsync(300);
  const last = reports().at(-1)!;
  expect(last.params.settings).toEqual({ panelMode: "button", shortcut: "Alt+Shift+S", siteAccess: true });
  expect(w.chrome.permissions.contains).toHaveBeenCalledWith({ origins: ["https://www.g2b.go.kr/*"] });
  expect(last.params.tabs[0].screenRows).toEqual([{ group: "계약 기본정보", label: "계약명", text: "시험 계약", source: "ctrtNm" }]);

  const command = (message: object) => w.ports[0].onMessage.addListener.mock.calls[0][0](message);
  command({ protocolVersion: 2, command: "setPanelMode", mode: "always" });
  await vi.advanceTimersByTimeAsync(300);
  expect(w.chrome.storage.local.set).toHaveBeenCalledWith({ panelMode: "always" });
  expect(reports().at(-1)!.params.settings.panelMode).toBe("always");
  command({ protocolVersion: 2, command: "setPanelMode", mode: "sometimes" });
  expect(w.chrome.storage.local.set).toHaveBeenCalledTimes(1);
  // 새 창으로 연다 — 앱이 새로 선 창을 알아보고 앞으로 가져온다. 탭으로 열면 뒤에 있는 창에 섞여 가려낼 수 없다.
  command({ protocolVersion: 2, command: "openShortcuts" });
  expect(w.chrome.windows.create).toHaveBeenCalledWith({ url: "chrome://extensions/shortcuts", focused: true });
  // 확장 관리 — 명령줄로는 chrome:// 를 열 수 없어 창이 확장에 맡긴다.
  command({ protocolVersion: 2, command: "openExtensions" });
  expect(w.chrome.windows.create).toHaveBeenCalledWith({ url: "chrome://extensions/", focused: true });
  expect(w.chrome.tabs.create).not.toHaveBeenCalled();

  // 사이트 접근이 바뀌면(다음 확인에서) 다시 보고한다.
  w.chrome.permissions.contains.mockResolvedValue(false);
  await vi.advanceTimersByTimeAsync(2600);
  expect(reports().at(-1)!.params.settings.siteAccess).toBe(false);
});

it("수집 흐름의 실패와 다시 보내 풀림을 코드만 실어 포트로 알린다", async () => {
  const w = worker();
  await waitFor(() => expect(w.ports).toHaveLength(1));
  const events = () => w.ports.flatMap(p => p.posted).filter(m => m.method === "event").map(m => m.params);
  await w.send("inspect");
  w.nextCapture(async () => { throw new Error("연결 끊김"); });
  expect((await w.send("save")).kind).toBe("pending");
  expect(events()).toEqual([{ kind: "error", code: "unconfirmed" }]);
  expect((await w.send("inspect")).kind).toBe("retry"); // 결과 조회 — 오류가 아니다.
  expect((await w.send("retry", w.sender, w.stored.pendingCapture.captureId)).kind).toBe("stored");
  expect(events()).toEqual([{ kind: "error", code: "unconfirmed" }, { kind: "recovered" }]);
  // 저장 대상의 거절은 rejected — 화면 안내(번호 없음)는 세지 않는다.
  await w.send("inspect");
  w.nextCapture(async (_host, request) => ({ protocolVersion: 2, requestId: request.requestId,
    ok: false, error: { code: "conflict", message: "시험 계약 건명이 바뀌었습니다." } } as any));
  await w.send("save");
  w.chrome.scripting.executeScript.mockImplementation(async (args: any) => args.files ? [] : [{ documentId: "doc1", result: { error: "화면에서 번호를 읽지 못했습니다. 화면이 다 열린 뒤 다시 읽어 주세요." } }]);
  await w.send("inspect");
  expect(events().slice(2)).toEqual([{ kind: "error", code: "rejected" }]);
  // 자료는 싣지 않는다.
  expect(JSON.stringify(events())).not.toMatch(/R26|시험|건명/);
});
