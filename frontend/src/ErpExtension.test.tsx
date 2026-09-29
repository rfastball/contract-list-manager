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
  const chrome = {
    runtime: { id: "extension", getURL: (file: string) => "chrome-extension://extension/" + file,
      sendNativeMessage: native, onMessage: { addListener: vi.fn() }, onInstalled: { addListener: vi.fn() }, openOptionsPage: vi.fn(),
      getManifest: () => ({ version: "0.4.1" }), reload: vi.fn() },
    tabs: { get: async () => ({ url: "https://www.g2b.go.kr/" }), query: vi.fn(async (_?: object) => [{ id: 1 }]),
      onRemoved: { addListener: vi.fn() }, sendMessage: vi.fn(async (..._: any[]): Promise<any> => { throw new Error("수신자 없음"); }) },
    commands: { getAll: async () => [{ name: "save-current", shortcut: "Alt+Shift+S" }], onCommand: { addListener: vi.fn() } },
    action: { setBadgeText: vi.fn(async (_: object) => {}), setBadgeBackgroundColor: vi.fn(async (_: object) => {}), openPopup: vi.fn(async () => {}) },
    scripting: { executeScript: vi.fn(async (args: any) => args.files ? [] : [{ documentId, result: { snapshot } }]) },
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
  return { chrome, sender, native, stored, local, start, send, nextCapture,
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
  expect(() => mapped(document, fakeWindow, location)).toThrow("번호를 찾지 못했습니다");
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
  expect(root.getElementById("status")!.textContent).toContain("엑셀로 내보낼 수 있습니다");
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

  w.chrome.tabs.sendMessage.mockResolvedValueOnce(true); // 페이지 수집기가 떠 있으면 거기에 그린다.
  const badges = tabBadges().length;
  press();
  await waitFor(() => expect(w.chrome.tabs.sendMessage).toHaveBeenCalledTimes(3));
  expect(w.chrome.tabs.sendMessage.mock.lastCall![1].view).toMatchObject({ kind: "ready", message: expect.stringContaining("검토가 필요합니다") });
  await new Promise(resolve => setTimeout(resolve, 0));
  expect(tabBadges()).toHaveLength(badges);
  expect(count("capture")).toBe(1);

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
  expect(count("capture")).toBe(2);
  expect(w.stored.pendingCapture).toBeDefined();
});


it("저장 위치는 설정 창에서만 묻고, 대상 경로를 먼저 보인 뒤 두 번째 누름에 예약하며, 거절 사유를 그대로 보인다", async () => {
  const w = worker();
  const reply = (request: any, body: object): any => ({ protocolVersion: 2, requestId: request.requestId, ...body });
  let place: any = { datasetId: "test", path: "C:\\자료\\계약목록_자료\\pclm.db", folder: "C:\\자료\\계약목록_자료",
    folderName: "계약목록_자료", pending: null };
  w.native.mockImplementation(async (_host: string, request: any) => {
    if (request.method === "dataLocation") return reply(request, { ok: true, result: place });
    if (request.method !== "stageDataMove") throw new Error("예상하지 않은 요청");
    if (request.params.folder === "D:\\쓰는 중") return reply(request, { ok: false,
      error: { code: "invalid_request", message: "고른 자리에 이미 계약 목록 자료가 있습니다." } });
    const to = request.params.folder.replace(/\\+$/, "") + "\\계약목록_자료";
    place = { ...place, pending: { from: place.folder, to } };
    return reply(request, { ok: true, result: { folder: to } });
  });
  const listener = w.chrome.runtime.onMessage.addListener.mock.lastCall![0];
  const optionsPage = { id: "extension", url: "chrome-extension://extension/options.html" };
  const ask = (message: object, from: object = optionsPage): Promise<any> =>
    new Promise(resolve => listener({ type: "pclm-location", ...message }, from, resolve));
  // 페이지 수집기·팝업·다른 확장은 자리를 볼 수도 옮길 수도 없다.
  for (const from of [w.sender, { id: "extension", url: "chrome-extension://extension/popup.html" }, { ...optionsPage, id: "other" }])
    expect(await ask({ action: "moveLocation", datasetId: "test", folder: "D:\\" }, from)).toMatchObject({ ok: false });
  expect(w.native).not.toHaveBeenCalled();

  const open = () => {
    document.body.innerHTML = optionsHtml;
    const chrome = { storage: { local: { get: async () => ({}), set: async () => {} } },
      commands: { getAll: async () => [] }, tabs: { create: vi.fn() }, runtime: { sendMessage: ask } };
    new Function("chrome", "document", optionsSource)(chrome, document);
  };
  const byId = (id: string) => document.getElementById(id) as HTMLInputElement;
  const stages = () => w.native.mock.calls.filter(([, request]) => request.method === "stageDataMove");
  const typeAndPress = (folder: string) => {
    byId("locationFolder").value = folder; byId("locationFolder").dispatchEvent(new Event("input"));
    byId("locationMove").click();
  };
  open();
  await waitFor(() => expect(byId("locationPath").value).toBe(place.path));
  expect(byId("locationPath").readOnly).toBe(true);
  expect(byId("locationPending").hidden).toBe(true);
  expect(byId("locationFolderName").textContent).toBe("계약목록_자료");
  expect(w.native.mock.lastCall![0]).toBe("kr.rfastball.pclm.erp");

  // 첫 누름은 대상만 보인다. 입력을 바꾸면 처음부터 다시 확인한다.
  typeAndPress("E:\\다른 곳");
  typeAndPress("D:\\업무\\");
  expect(byId("locationTarget").textContent).toContain("D:\\업무\\계약목록_자료");
  expect(stages()).toHaveLength(0);
  byId("locationMove").click();
  await waitFor(() => expect(byId("locationPending").hidden).toBe(false));
  expect(stages()).toHaveLength(1);
  expect(stages()[0][1].params).toEqual({ datasetId: "test", folder: "D:\\업무\\" });
  expect(byId("locationPending").textContent).toBe("다음 앱 실행 때 C:\\자료\\계약목록_자료 → D:\\업무\\계약목록_자료 로 옮깁니다.");
  expect(byId("locationTarget").textContent).toContain("예약했습니다");

  typeAndPress("D:\\쓰는 중"); byId("locationMove").click();
  await waitFor(() => expect(byId("locationTarget").textContent).toContain("이미 계약 목록 자료가 있습니다"));
  expect(stages()).toHaveLength(2);

  // 개발 DB 면 호스트가 거절한 까닭을, 호스트가 없으면 연결 안내만 보인다.
  w.native.mockImplementation(async (_host: string, request: any) => reply(request, { ok: false,
    error: { code: "invalid_request", message: "개발 DB 는 위치를 옮길 수 없습니다." } }));
  open();
  await waitFor(() => expect(byId("locationStatus").textContent).toBe("개발 DB 는 위치를 옮길 수 없습니다."));
  expect(byId("locationBody").hidden).toBe(true);
  w.native.mockRejectedValue(new Error("호스트 없음"));
  open();
  await waitFor(() => expect(byId("locationStatus").textContent).toContain("연결하지 못했습니다"));
  expect(byId("locationBody").hidden).toBe(true);
});

it("활성 매핑은 표의 전체 행과 복합키를 읽고 허용하지 않은 값을 보내지 않는다", () => {
  screen();
  const p = JSON.parse(mappingJson).profiles.find((p: any) => p.id === "g2b-request-v1");
  const table = p.tables.find((t: any) => t.required);
  document.body.innerHTML = `<div id="mf_wfm_container"><input id="ctrtDmndBizNm" value="시험 접수" readonly><div id="${table.source}"></div></div>`;
  const rows = ["001", "002"].map(key => ({ ctrtDmndRcptNo: "RC001", ctrtDmndRcptOrd: "000", ctrtDmndRcptItemSqno: key,
    ndfsPrcmDmndNo: "REPEATED", ctrtDmndQty: "0", giveActno: "보내면 안 되는 값" }));
  const fakeWindow: any = { $p: { getComponentById: (id: string) => id === table.source ? { getDataList: () => ({ getAllJSON: () => rows }) } : null } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", "profiles", captureSource + "; return pclmReadMapped(profiles);");
  const result = read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, [p]);
  expect(result.data.tables[table.source]).toHaveLength(2);
  expect(result.data.tables[table.source][1].ctrtDmndRcptItemSqno).toBe("002");
  expect(result.data.tables[table.source][0].ctrtDmndQty).toBe("0");
  expect(JSON.stringify(result)).not.toContain("보내면 안 되는 값");
});

it("화면의 문서는 번호로만 가리고, 번호가 여럿 보이면 가장 아랫단 문서를 고르며, 표가 없어도 받는다", () => {
  screen();
  const profiles = JSON.parse(mappingJson).profiles;
  // 품목표 하나 없는 화면. 계약 화면은 공고·접수 번호를 참조로 함께 싣는다.
  const values: Record<string, string> = { ctrtNoOrd: "R26TA00000001-00", bidPbancNo: "R26BK00000001", ctrtDmndRcptNo: "R26DC00000001" };
  const render = () => {
    document.body.innerHTML = `<div id="mf_wfm_container">${Object.keys(values).map(k => `<span id="mf_wfm_container_${k}"></span>`).join("")}</div>`;
  };
  const fakeWindow: any = { $p: { getComponentById: (id: string) => {
    const key = id.replace("mf_wfm_container_", "");
    return key in values ? { getRef: () => "data:dma_pointInfo." + key, getValue: () => values[key] } : null;
  } } };
  fakeWindow.top = fakeWindow;
  const read = new Function("document", "window", "location", "profiles", captureSource + "; return pclmReadMapped(profiles);");
  const run = () => read(document, fakeWindow, { origin: "https://www.g2b.go.kr" }, profiles);
  render();
  expect(run()).toMatchObject({ profile: "g2b-contract-v1", data: { tables: {} } });
  delete values.ctrtNoOrd; values.bidPbancOrd = "000"; render();
  expect(run().profile).toBe("g2b-notice-a-v1");
  delete values.bidPbancOrd; render(); // 공고 차수가 없으면 공고 번호는 참조일 뿐이다.
  expect(run).toThrow("번호를 찾지 못했습니다");
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
  const fakeWindow: any = { $p: { getComponentById: (id: string) => parts[id] ?? null } };
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
