// 계약 목록과 나라장터 화면 사이의 다리. 수집·저장은 앱의 「지금 보는 화면」 이 한다(ADR-036) — 여기는 상시 포트를
// 쥐고 나라장터 탭을 읽어 보고하고, 앱의 명령(다시 읽기·탭 앞으로·엑셀로 내보내기·확장 관리)을 따르며, 수집 화면에
// 문서 만들기 단추를 띄운다(ADR-046). 수동·조건부 자동 저장은 앱이 맡는다. 예외는 문서 만들기 패널(ADR-045)의 「계약목록에 저장」 하나 —
// 그것도 호스트의 document 메서드가 수집과 같은 저장(ErpCapture::save)을 탄다.
const origin = "https://www.g2b.go.kr";
const bundledRevision = "__PCLM_BUNDLE_REVISION__";
const failure = (message, extra = {}) => Object.assign(new Error(message), extra);
// 스위치는 없다 — 제 ID 가 개발용일 때만 개발 호스트에 잇는다(ADR-048). scripts/fake-g2b/server.mjs 의 DEV_ID 와 같아야 하고 시험이 대조한다.
const DEVELOPMENT_ID = "kjjcbnphdfamcohjlfipcnbodbblkffc";
const nativeHost = chrome.runtime.id === DEVELOPMENT_ID ? "kr.rfastball.pclm.erp.dev" : "kr.rfastball.pclm.erp";

async function rpc(method, params) {
  const requestId = crypto.randomUUID();
  let response;
  try {
    response = await chrome.runtime.sendNativeMessage(nativeHost, { protocolVersion: 2, requestId, method, params });
  } catch (error) {
    throw failure("계약 목록에 연결하지 못했습니다.", { connection: true, detail: error.message });
  }
  if (response?.protocolVersion !== 2 || response.requestId !== requestId || typeof response.ok !== "boolean")
    throw failure("계약 목록의 응답을 확인하지 못했습니다.", { response: true });
  if (!response.ok) {
    if (!response.error?.code || typeof response.error.message !== "string")
      throw failure("계약 목록의 응답을 확인하지 못했습니다.", { response: true });
    const connection = ["setup_required", "schema_mismatch", "wrong_environment"].includes(response.error.code);
    throw failure(connection ? "계약 목록 준비가 필요합니다. 계약 목록의 「Chrome 확장 상태」 를 확인해 주세요." : response.error.message,
      { code: response.error.code, connection, detail: response.error.message });
  }
  return response.result;
}

function browserName() {
  const brands = (navigator.userAgentData?.brands || []).map(b => b.brand);
  return brands.includes("Microsoft Edge") ? "Edge" : brands.includes("Google Chrome") ? "Chrome" : "기타";
}

const revisionShaped = value => typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const runningRevision = revisionShaped(bundledRevision) ? bundledRevision : null;
const update = { expected: undefined, state: "", checking: false, work: 0, reloading: false, requests: new Map() };
function updateState(state) {
  if (update.state === state || !presence) return;
  update.state = state;
  try { presence.postMessage({ protocolVersion: 2, requestId: crypto.randomUUID(), method: "extensionUpdateStatus", params: { state } }); } catch { /* 다시 연결하면 확인한다. */ }
}
function expectRevision(revision) {
  if (revision !== null && !revisionShaped(revision)) return;
  update.expected = revision;
  void applyExtensionUpdate();
}
function freshRevision() {
  return new Promise((resolve, reject) => {
    const requestId = crypto.randomUUID();
    const timer = setTimeout(() => { update.requests.delete(requestId); reject(failure("확장 파일 준비 상태를 확인하지 못했습니다.")); }, 3000);
    update.requests.set(requestId, { resolve, reject, timer });
    try { presence.postMessage({ protocolVersion: 2, requestId, method: "extensionUpdateCheck" }); }
    catch (error) { clearTimeout(timer); update.requests.delete(requestId); reject(error); }
  });
}
async function panelOpen() {
  // 입력 중인 값과 파일 쓰기는 패널 안에 있다. 빈 패널도 닫힐 때까지 기다린다.
  if (!chrome.runtime.getContexts) throw failure("열린 문서 패널을 확인하지 못했습니다.");
  return (await chrome.runtime.getContexts({ documentUrls: [chrome.runtime.getURL("panel/panel.html")] })).length > 0;
}
async function applyExtensionUpdate() {
  if (!presence || update.checking || update.reloading || update.expected === undefined) return;
  update.checking = true;
  let savedAttempt = false;
  try {
    if (!runningRevision) { updateState("failed"); return; }
    if (update.expected === null) { updateState("preparing"); return; }
    if (update.expected === runningRevision) {
      if (update.state !== "current") await chrome.storage.local.set({ reloadAttempt: null });
      updateState("current"); return;
    }
    if (await panelOpen()) { updateState("waiting-panel"); return; }
    if (update.work) { updateState("waiting-work"); return; }
    const expected = await freshRevision();
    // 추출 중에는 완료 표시가 없다. 오래된 알림으로 파일을 읽다 멈추지 않는다.
    if (expected === null || revisionShaped(expected)) update.expected = expected;
    else throw failure("확장 파일 준비 상태를 확인하지 못했습니다.");
    if (!presence || update.expected === null) { updateState("preparing"); return; }
    if (update.expected === runningRevision) { updateState("current"); return; }
    const attempt = `${runningRevision}→${update.expected}`;
    if ((await chrome.storage.local.get("reloadAttempt")).reloadAttempt === attempt) { updateState("failed"); return; }
    if (await panelOpen()) { updateState("waiting-panel"); return; }
    if (update.work) { updateState("waiting-work"); return; }
    await chrome.storage.local.set({ reloadAttempt: attempt });
    savedAttempt = true;
    // 위 비동기 저장 중 새 작업·패널·준비 취소가 들어올 수 있다.
    const opened = await panelOpen();
    if (!presence || update.expected === null || `${runningRevision}→${update.expected}` !== attempt || update.work || opened) {
      await chrome.storage.local.set({ reloadAttempt: null });
      if (update.expected === null) updateState("preparing");
      else if (update.work) updateState("waiting-work");
      else if (opened) updateState("waiting-panel");
      return;
    }
    updateState("reloading");
    update.reloading = true;
    chrome.runtime.reload();
  } catch {
    if (savedAttempt && !update.reloading) await chrome.storage.local.set({ reloadAttempt: null }).catch(() => {});
    updateState("failed");
  }
  finally { update.checking = false; }
}
async function extensionWork(task) {
  update.work++;
  try { return await task(); }
  finally { update.work--; void applyExtensionUpdate(); }
}

// 상시 연결(ADR-035). 포트가 열려 있는 동안 브라우저는 워커를 살려 두고 호스트는 포트만큼 산다 — 앱은 호스트가
// 남긴 프로세스별 파일로 확장이 지금 살아 있는지 안다. 인사만 하는 포트라 매핑을 받는 hello 는 여전히 rpc() 로 간다.
// 호스트는 제 exe 가 비켜지면(새 판을 놓으려고 옛 exe 의 이름을 바꿨다) 스스로 떠난다 — 그러면 물러서며 다시 이어
// 그 자리의 새 exe 에 붙는다.
const presenceDelays = [1000, 2000, 5000, 15000, 60000];
let presence = null, presenceTimer = null;
let presenceRetry = 0, presenceSince = 0;
function retryPresence() {
  if (presenceTimer) return;
  const delay = presenceDelays[Math.min(presenceRetry++, presenceDelays.length - 1)];
  presenceTimer = setTimeout(() => { presenceTimer = null; connectPresence().catch(() => retryPresence()); }, delay);
}
async function connectPresence() {
  if (presence) return;
  clearTimeout(presenceTimer); presenceTimer = null;
  const port = chrome.runtime.connectNative(nativeHost);
  presence = port; presenceSince = Date.now();
  const requestId = crypto.randomUUID();
  update.state = "";
  port.onMessage.addListener(message => {
    if (presence !== port) return;
    const request = update.requests.get(message?.requestId);
    if (request) {
      clearTimeout(request.timer); update.requests.delete(message.requestId);
      if (message.ok === true && Object.hasOwn(message.result || {}, "expectedRevision")) request.resolve(message.result.expectedRevision);
      else request.reject(failure("확장 파일 준비 상태를 확인하지 못했습니다."));
      return;
    }
    if (message?.requestId === requestId && message.ok === true && Object.hasOwn(message.result || {}, "expectedRevision")) expectRevision(message.result.expectedRevision);
    else mirrorCommand(message);
  });
  port.onDisconnect.addListener(() => {
    void chrome.runtime.lastError; // 읽지 않으면 브라우저가 확인하지 않은 오류로 남긴다.
    if (presence !== port) return; // 일부러 끊은 옛 포트.
    presence = null;
    update.expected = undefined;
    for (const request of update.requests.values()) { clearTimeout(request.timer); request.reject(failure("연결이 끊겼습니다.")); }
    update.requests.clear();
    stopMirror();
    notifyPanel();
    if (Date.now() - presenceSince >= 60000) presenceRetry = 0;
    retryPresence();
  });
  port.postMessage({ protocolVersion: 2, requestId, method: "presence",
    params: { extensionVersion: chrome.runtime.getManifest().version, browser: browserName(), ...(runningRevision ? { extensionRevision: runningRevision } : {}) } });
  startMirror().catch(() => {});
  notifyPanel();
}

// ── 지금 보는 화면(ADR-036) ─────────────────────────────────
// 상시 포트가 열려 있는 동안만 나라장터 탭을 알고, 읽고, 앱에 보고한다 — 포트가 없으면(앱 호스트 없음) 아무것도 하지
// 않는다. 호스트는 받은 보고를 홈의 파일에 적고 창이 그것을 읽는다. 창의 명령(그 탭을 앞으로·다시 읽기·엑셀로 내보내기)은
// 같은 포트로 돌아온다. 확장은 읽기만 하고 저장하지 않는다 — 저장은 창의 「가져오기」 다.
// 읽는 때: 탭이 앞으로 올 때, 로드가 끝날 때, 창이 부를 때, 그리고 앞 탭은 가벼운 지문을 2초마다 보아 바뀌었을 때
// (WebSquare 는 주소 없이 화면을 바꾼다). 뒤의 탭은 처음 한 번과 부를 때만 읽는다.
const mirrorLoading = "화면을 불러오는 중입니다.";
// seq 는 읽기마다 오르는 번호 — 탭 항목에 실어 보내고, 박동(beat)이 같은 번호로 그 읽기가 아직 그대로임을 알린다.
const mirror = { tabs: new Map(), front: null, frontAt: 0, timer: null, poll: null, sent: "", hello: null, helloAt: 0, helloTried: 0, seq: 0,
  prints: new Map(), reading: new Set(), again: new Set(), retries: new Map(), generations: new Map(), settings: null, me: { name: "" }, chips: new Set() };
// 문서 만들기 패널(아래)에 마지막으로 알린 읽은 결과 — 탭마다.
const panelSeen = new Map();
// 그 알린 결과의 번호 — 결과가 달라질 때마다 오른다. 패널이 check 로 받은 번호를 save 에 되싣는다(사람이 본 그 화면만 저장한다).
const panelRead = new Map();
let panelReads = 0;
const isG2b = url => typeof url === "string" && url.startsWith(origin + "/");
const two = n => String(n).padStart(2, "0");
function localStamp() {
  const d = new Date();
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}T${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`;
}
const byteLength = text => new TextEncoder().encode(text).length;

// 수집 규칙(매핑)은 작업자료에 있어 hello 로만 받는다. 읽을 때마다 호스트를 띄우지 않게 잠깐 쥐고 쓴다.
async function mirrorHello() {
  if (mirror.hello && Date.now() - mirror.helloAt < 60000) return mirror.hello;
  // 실패해도 시도한 때를 남긴다 — 앞 탭의 1분마다 다시 읽기(watchFront)가 실패하는 hello 를 2초마다 되풀이하지 않게.
  mirror.helloTried = Date.now();
  const hello = await rpc("hello", { extensionVersion: chrome.runtime.getManifest().version, browser: browserName() });
  mirror.hello = hello; mirror.helloAt = mirror.helloTried = Date.now();
  return hello;
}

function trackTab(tab) {
  if (!tab?.id || !isG2b(tab.url)) return false;
  const known = mirror.tabs.get(tab.id);
  mirror.tabs.set(tab.id, { ...(known || { state: "reading", readAt: "", screen: "", menu: "" }), tabId: tab.id, windowId: tab.windowId, title: tab.title || "" });
  if (!known || known.title !== (tab.title || "")) report();
  return !known;
}
function forgetTab(tabId) {
  mirror.generations.set(tabId, (mirror.generations.get(tabId) || 0) + 1);
  mirror.prints.delete(tabId); mirror.retries.delete(tabId); panelSeen.delete(tabId); panelRead.delete(tabId); mirror.chips.delete(tabId);
  const entry = mirror.tabs.get(tabId);
  if (!mirror.tabs.delete(tabId)) return;
  if (mirror.front === tabId) { mirror.front = null; mirror.frontAt = Date.now(); }
  report();
  notifyPanel(entry.windowId);
}

// 수집 화면에 띄운 「문서 만들기」 단추(ADR-046, chip.js). 심는 것은 chip.js 가 멱등이라 supported 로 읽을 때마다 넣고(페이지가
// 다시 서면 단추도 사라진다), 떼는 것은 심은 탭에만 한다. 실패(탭이 닫힘 · 페이지가 바뀌는 중)는 삼킨다.
function placeChip(tabId, on) {
  if (!on && !mirror.chips.delete(tabId)) return;
  if (on) mirror.chips.add(tabId);
  chrome.scripting.executeScript(on ? { target: { tabId }, files: ["chip.js"] }
    : { target: { tabId }, func: () => document.getElementById("pclm-chip")?.remove() }).catch(() => {});
}

async function readTab(tabId) {
  if (!presence || !mirror.tabs.has(tabId)) return;
  if (mirror.reading.has(tabId)) { mirror.again.add(tabId); return; }
  mirror.reading.add(tabId);
  const generation = mirror.generations.get(tabId) || 0;
  mirror.tabs.set(tabId, { ...mirror.tabs.get(tabId), state: "reading" });
  report();
  let next, retry = false;
  try {
    const hello = await mirrorHello();
    if (generation !== (mirror.generations.get(tabId) || 0)) throw failure("화면이 바뀌어 다시 읽어야 합니다.");
    const injected = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", files: ["capture.js", "export.js"] });
    const documentId = injected[0]?.documentId;
    const [result] = await chrome.scripting.executeScript({ target: { tabId, ...(documentId ? { documentIds: [documentId] } : {}) }, world: "MAIN", args: [hello.mapping.profiles], func: profiles => {
      const screen = document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")?.textContent.trim() || "";
      // 메뉴 번호(ADR-037) — 창이 「수집 안 함」 곁에 보인다. 모르면 빈 글.
      let menu = ""; try { menu = pclmScreenCode(); } catch { /* 모른다 */ }
      // 화면의 이름표와 보이는 글(「화면 그대로」). 읽지 못해도 스냅샷은 그대로 낸다.
      const rows = (profile, snapshot) => { try { return pclmScreenRows(profile, snapshot); } catch { return []; } };
      try {
        const observed = pclmObserveMapped(profiles);
        if (!observed.snapshot) return { screen, menu, ...observed, screenRows: rows(null) };
        return { screen, menu, ...observed, screenRows: rows(profiles.find(p => p.id === observed.snapshot.profile), observed.snapshot) };
      }
      catch (error) { return { screen, menu, error: error.message, unsupported: Boolean(error.unsupported), screenRows: rows(null) }; }
    } });
    const read = result?.result || {};
    if (documentId && result?.documentId !== documentId) throw failure("화면이 바뀌어 다시 읽어야 합니다.");
    if (typeof read.print === "string") mirror.prints.set(tabId, read.print);
    const menu = typeof read.menu === "string" && /^\d{5}$/.test(read.menu) ? read.menu : "";
    // 호스트는 칸 하나라도 1000자(UTF-16)를 넘으면 탭 보고 전체를 받지 않는다 — 넘는 글은 잘라 싣는다(짝 잃은 서로게이트는 남기지 않는다).
    const cut = v => { const s = String(v ?? ""); return s.length <= 1000 ? s : s.slice(0, /[\uD800-\uDBFF]/.test(s[999]) ? 999 : 1000); };
    const screenRows = Array.isArray(read.screenRows)
      ? read.screenRows.slice(0, 400).map(r => ({ group: cut(r?.group), label: cut(r?.label), text: cut(r?.text), source: cut(r?.source) })) : [];
    if (read.snapshot) {
      const snapshot = { ...read.snapshot, mappingRevision: hello.mappingRevision };
      next = byteLength(JSON.stringify(snapshot)) > (hello.maxRequestBytes || 768 * 1024) - 4096
        ? { state: "error", screen: read.screen, menu, message: "화면 자료가 커서 미리 볼 수 없습니다." }
        : { state: "supported", screen: read.screen, menu, snapshot, screenRows, readiness: read.readiness, datasetId: hello.datasetId };
    } else if (read.error === mirrorLoading && (mirror.retries.get(tabId) || 0) < 5) retry = true;
    else if (read.unsupported) next = { state: "unsupported", screen: read.screen, menu, screenRows };
    else next = { state: "error", screen: read.screen || "", menu, message: read.error || "화면을 읽지 못했습니다." };
  } catch (error) { next = { state: "error", screen: "", menu: "", message: error.message }; }
  finally { mirror.reading.delete(tabId); }
  if (generation !== (mirror.generations.get(tabId) || 0)) {
    if (mirror.again.delete(tabId)) await readTab(tabId);
    return;
  }
  const entry = mirror.tabs.get(tabId);
  if (!entry) return;
  if (retry) {
    // 화면이 아직 그려지는 중이다. 조금 뒤 다시 본다.
    mirror.retries.set(tabId, (mirror.retries.get(tabId) || 0) + 1);
    setTimeout(() => { readTab(tabId).catch(() => {}); }, 1000);
    return;
  }
  mirror.retries.delete(tabId);
  mirror.tabs.set(tabId, { tabId, windowId: entry.windowId, title: entry.title, readAt: localStamp(), seq: ++mirror.seq, ...next, ...(entry.export ? { export: entry.export } : {}) });
  report();
  placeChip(tabId, next.state === "supported");
  const seen = JSON.stringify([next.state, next.snapshot ?? null, next.message ?? ""]);
  if (panelSeen.get(tabId) !== seen) { panelSeen.set(tabId, seen); panelRead.set(tabId, ++panelReads); notifyPanel(entry.windowId); }
  if (mirror.again.delete(tabId)) await readTab(tabId);
}

// Chrome 에서 앞에 있는 나라장터 탭 — 마지막으로 초점을 가진 창의 활성 탭이 나라장터일 때. 사람이 앱 창으로 옮겨 가
// Chrome 이 초점을 잃어도 그 창의 활성 탭은 그대로 앞이다.
async function refreshFront() {
  let tab;
  try { [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true }); } catch { return; }
  const front = tab?.id && isG2b(tab.url) ? tab.id : null;
  if (front !== null) trackTab(tab);
  if (front === mirror.front) return;
  mirror.front = front; mirror.frontAt = Date.now();
  report();
  if (front !== null) await readTab(front);
}

// 앞 탭의 매핑 원값·표·탭 상태 지문. 자동 관찰은 클릭·스크롤·초점 이동 없이 읽기만 한다.
async function watchFront() {
  const tabId = mirror.front;
  if (!presence || tabId === null || mirror.reading.has(tabId)) return;
  const [result] = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", func: () => {
    const observed = pclmObserveMapped();
    return { print: observed.print, readiness: observed.readiness };
  } });
  const print = result?.result?.print;
  if (typeof print !== "string") return;
  const seen = mirror.prints.get(tabId);
  mirror.prints.set(tabId, print);
  const entry = mirror.tabs.get(tabId);
  // readiness 는 supported 에만 담는다 — 수집하지 않는 화면 · 오류는 견주면 늘 달라 2초마다 다시 읽게 된다.
  const supported = entry?.state === "supported";
  if (seen !== undefined && seen !== print || supported && entry.readiness?.stable !== result.result.readiness?.stable || Date.now() - mirror.helloTried >= 60000) await readTab(tabId);
  else if (supported && result.result.readiness) {
    // 화면이 그대로다 — 스냅샷을 다시 싣지 않고 그 읽기의 번호와 새 readiness 만 박동으로 보낸다.
    const { stable, observedAt, documentId } = result.result.readiness;
    try { presence.postMessage({ protocolVersion: 2, requestId: crypto.randomUUID(), method: "beat", params: { tabId, seq: entry.seq, readiness: { stable, observedAt, documentId } } }); }
    catch { /* 다음 박동이 다시 알린다. */ }
  }
}

// 보고는 바뀔 때만, 너무 잦지 않게 묶어 보낸다. 요청 상한을 넘으면 뒤 탭의 스냅샷부터 덜어 낸다 — 앞으로 오면 다시 읽는다.
function report() {
  if (!presence || mirror.timer) return;
  mirror.timer = setTimeout(() => { mirror.timer = null; sendReport(); }, 300);
}
function sendReport() {
  if (!presence) return;
  const limit = (mirror.hello?.maxRequestBytes || 768 * 1024) - 4096;
  let tabs = [...mirror.tabs.values()];
  const params = () => ({ front: mirror.front, frontAt: mirror.frontAt, tabs, settings: mirror.settings, me: mirror.me });
  let text = JSON.stringify(params());
  for (const victim of tabs.filter(t => t.snapshot && t.tabId !== mirror.front)) {
    if (byteLength(text) <= limit) break;
    tabs = tabs.map(t => t === victim ? { tabId: t.tabId, windowId: t.windowId, title: t.title, readAt: t.readAt, screen: t.screen, menu: t.menu,
      state: "error", message: "열린 탭이 많아 이 화면은 앞으로 가져오면 다시 읽습니다.", ...(t.export ? { export: t.export } : {}) } : t);
    text = JSON.stringify(params());
  }
  if (byteLength(text) > limit) tabs = tabs.map(t => t.snapshot || t.screenRows ? { ...t, snapshot: undefined, screenRows: undefined,
    state: "error", message: "화면 자료가 커서 미리 볼 수 없습니다." } : t);
  text = JSON.stringify(params());
  if (text === mirror.sent) return;
  mirror.sent = text;
  try { presence.postMessage({ protocolVersion: 2, requestId: crypto.randomUUID(), method: "tabs", params: params() }); }
  catch { mirror.sent = ""; }
}

// 확장 설정(「Chrome 확장 상태」 의 설정) — 나라장터 사이트 접근 하나. 사람이 브라우저에서 바꿔도 알림이 늦어
// 앞 탭을 볼 때마다 다시 본다. 바뀌면 다시 보고한다.
async function readSettings() {
  const siteAccess = chrome.permissions?.contains
    ? await chrome.permissions.contains({ origins: [origin + "/*"] }).catch(() => false) : false;
  return { siteAccess: Boolean(siteAccess) };
}
// 패널 설정의 「내 정보」 이름 — 앱의 자동 수집이 이 이름으로 앞 탭의 담당자를 가린다. 패널이 닫혀 있어도 싣도록 패널의
// IndexedDB(store.js 의 pclm-document / settings / me)를 직접 읽는다 — background 는 모듈이 아니라 store.js 를 쓰지 못한다.
// 판을 주지 않고 열어 저장소를 짓거나 올리지 않는다. 읽지 못하면 빈 이름(앱은 자동 수집하지 않는다).
function readMe() {
  return new Promise((resolve) => {
    const req = indexedDB.open("pclm-document");
    req.onerror = () => resolve("");
    req.onsuccess = () => {
      const db = req.result;
      try {
        const get = db.transaction("settings").objectStore("settings").get("me");
        get.onsuccess = () => { db.close(); resolve(get.result?.name); };
        get.onerror = () => { db.close(); resolve(""); };
      } catch { db.close(); resolve(""); }
    };
  }).then((name) => ({ name: typeof name === "string" ? name.trim() : "" }), () => ({ name: "" }));
}
async function refreshSettings() {
  if (!presence) return;
  const [next, me] = await Promise.all([readSettings(), readMe()]);
  if (JSON.stringify(next) === JSON.stringify(mirror.settings) && me.name === mirror.me.name) return;
  mirror.settings = next;
  mirror.me = me;
  report();
}

async function startMirror() {
  mirror.sent = ""; // 새 호스트다 — 같은 것이라도 다시 보낸다.
  [mirror.settings, mirror.me] = await Promise.all([readSettings().catch(() => null), readMe()]);
  let tabs = [];
  try { tabs = await chrome.tabs.query({ url: origin + "/*" }); } catch { /* 모른다 */ }
  for (const tab of tabs) trackTab(tab);
  await refreshFront();
  // 처음 한 번만 읽는다. 다시 이은 것이면 읽어 둔 것을 그대로 보낸다.
  for (const [tabId, entry] of mirror.tabs) if (!entry.readAt && tabId !== mirror.front) readTab(tabId).catch(() => {});
  report();
  clearInterval(mirror.poll);
  mirror.poll = setInterval(() => { watchFront().catch(() => {}); refreshSettings().catch(() => {}); void applyExtensionUpdate(); }, 2000);
}
function stopMirror() {
  // 포트가 없으면 패널이 「연결되지 않았습니다」 뿐이라 단추도 거둔다.
  for (const tabId of mirror.chips) placeChip(tabId, false);
  clearInterval(mirror.poll); mirror.poll = null;
  clearTimeout(mirror.timer); mirror.timer = null;
}

// 앱이 보낸 명령. 탭 명령은 확장이 아는 나라장터 탭에만 닿는다.
function mirrorCommand(message) {
  if (message?.protocolVersion !== 2 || typeof message.command !== "string") return;
  if (message.command === "extensionUpdate") { expectRevision(message.revision); return; }
  // 브라우저 안의 확장 관리 — 명령줄로는 chrome://·edge:// 주소를 열 수 없어 창이 확장에 맡긴다. 새 창으로 연다: 브라우저는
  // 앱 창 뒤에 있어 스스로 앞으로 오지 못하고(focused 를 줘도), 앞으로 가져오는 것은 앱이 한다 — 앱은 새로 선 창으로 그것을
  // 알아본다. 탭으로 열면 마지막에 본 창(최소화돼 있을 수도 있다)에 섞여 앱이 가려낼 수 없다.
  if (message.command === "openExtensions") {
    chrome.windows.create({ url: `${browserName() === "Edge" ? "edge" : "chrome"}://extensions/`, focused: true }).catch(() => {});
    return;
  }
  if (!Number.isInteger(message.tabId)) return;
  const entry = mirror.tabs.get(message.tabId);
  if (!entry) return;
  if (message.command === "read") { mirror.prints.delete(message.tabId); readTab(message.tabId).catch(() => {}); }
  else if (message.command === "focusTab")
    chrome.tabs.update(message.tabId, { active: true })
      .then(() => chrome.windows?.update(entry.windowId, { focused: true })).catch(() => {});
  else if (message.command === "export") {
    // 결과는 그 탭의 보고에 실어 앱에 돌려보낸다 — 앱은 요청 뒤에 온 결과로 성공·실패를 말한다.
    const tabId = message.tabId;
    extensionWork(() => downloadTab(tabId)).then(() => ({ ok: true, message: "" }),
      error => ({ ok: false, message: error?.message || "화면을 엑셀로 내보내지 못했습니다." }))
      .then(result => {
        const now = mirror.tabs.get(tabId);
        if (!now) return;
        mirror.tabs.set(tabId, { ...now, export: { at: Date.now(), ...result } });
        report();
      });
  }
}

// 현재 화면 엑셀로 내보내기를 그 탭 안에서 끝낸다 — 앱이 부른 것이라 확장에 띄울 화면이 없다. 페이지에 받을 쪽(콘텐츠
// 스크립트)을 두지 않으므로 export.js 를 넣고 같은 자리에서 파일을 지어 내려받기를 건다. 화면이 아직 읽는 중이거나 표가
// 없어 짓지 못하면 그 까닭을 돌려받아 던진다 — 부른 쪽이 탭 보고로 앱에 알린다. 파일 이름은 「화면 제목_날짜.xlsx」 다.
async function downloadTab(tabId) {
  await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", files: ["export.js"] });
  const [done] = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", func: () => {
    try {
      const result = pclmExportPage();
      const url = URL.createObjectURL(new Blob([pclmWorkbook(result.sheets)], { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" }));
      const link = document.createElement("a");
      link.href = url;
      // 탭 안에서 돈다 — background 의 localStamp 를 부르지 못해 그 자리에서 지역 날짜를 짓는다.
      const d = new Date(), two = n => String(n).padStart(2, "0");
      link.download = `${result.title.replace(/[<>:"/\\|?*\x00-\x1f]/g, "_").slice(0, 80)}_${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}.xlsx`;
      document.body.append(link); link.click(); link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 30000);
      return { ok: true };
    } catch (error) { return { error: error.message }; }
  } });
  if (!done?.result?.ok) throw failure(done?.result?.error || "화면을 읽지 못했습니다.");
}

// ── 문서 만들기 패널(ADR-045) ─────────────────────────────
// 패널은 제 창의 활성 탭 하나만 본다(고정 · 최근 건은 없다 — 문서는 언제나 눈앞의 화면과 같은 건이다). 탭을 읽는 것은
// 지금 보는 화면과 같은 readTab 이고, 호스트에 묻는 것도 같은 rpc() 다 — 두 번째 네이티브 길을 내지 않는다. 포트가 없으면
// (앱 호스트 없음) 읽지도 묻지도 않고 「연결되지 않았습니다」 를 돌려준다.
// 탭이 바뀌거나, 그 화면을 다시 읽어 결과가 달라졌거나, 포트가 서고 끊기면 패널에 알려 다시 묻게 한다(ADR-045 4항). 같은
// 결과를 다시 읽은 것은 알리지 않는다 — 패널의 check 가 다시 읽으므로, 알리면 끝없이 묻고 읽는다.
function notifyPanel(windowId = null) {
  // 열린 패널이 없으면 받는 이가 없다는 거절이 온다 — 버린다.
  try { chrome.runtime.sendMessage?.({ type: "pclm-document-changed", windowId })?.catch?.(() => {}); } catch { /* 받는 이 없음 */ }
}

// op: check(지금 화면이 저장된 것과 같은가 — 그 탭을 다시 읽어 묻는다. 답에 탭과 읽은 결과의 번호 read 를 싣는다)
// · save(읽어 둔 화면을 저장 — 사람이 본 그 화면이다: 패널이 되실은 tabId · read 가 지금 것과 다르면 저장하지 않고 다시 확인하게
// 한다) · read(check 가 돌려준 건의 면 뷰) · templates(서식 폴더 — action: list · read(name) · open, ADR-049. 작업자료와
// 상관없어 hello 를 거치지 않는다).
async function documentCall({ windowId, op, entity, entityType, action, name, tabId, read }) {
  if (!["check", "save", "read", "templates"].includes(op) || !Number.isInteger(windowId)) return { kind: "failed", message: "지원하지 않는 요청입니다." };
  if (!presence) return { kind: "offline" };
  if (op === "templates") return { kind: "answer", answer: await rpc("templates", { op: action, name }) };
  if (op === "read") {
    const hello = await mirrorHello();
    return { kind: "answer", answer: await rpc("document", { datasetId: hello.datasetId, op, entity, entityType }) };
  }
  let tab;
  try { [tab] = await chrome.tabs.query({ active: true, windowId }); } catch { /* 모른다 */ }
  if (!tab?.id || !isG2b(tab.url)) return { kind: "elsewhere" };
  trackTab(tab);
  // 진행 중인 읽기는 완료 알림으로 받는다 — 여기서 재예약하면 패널의 재확인과 끝없이 맞물린다.
  if (op === "check" && !mirror.reading.has(tab.id)) await readTab(tab.id);
  const entry = mirror.tabs.get(tab.id);
  if (!entry || entry.state === "reading") {
    // 다른 읽기가 아직 돈다. 그 결과가 앞과 같아도 패널은 알림을 받아야 「읽는 중」 에서 나온다.
    panelSeen.delete(tab.id);
    return { kind: "reading", tabId: tab.id };
  }
  if (entry.state === "unsupported") return { kind: "elsewhere" };
  if (entry.state !== "supported") return { kind: "error", tabId: tab.id, message: entry.message || "화면을 읽지 못했습니다." };
  // 패널이 본 뒤에 탭이 바뀌었거나 그 화면을 다시 읽어 달라졌다 — 패널이 다시 확인한다(state.js 의 reading).
  if (op === "save" && (tabId !== tab.id || read !== panelRead.get(tab.id))) return { kind: "reading", tabId: tab.id };
  return { kind: "answer", tabId: tab.id, read: panelRead.get(tab.id), answer: await rpc("document", { datasetId: entry.datasetId, op, snapshot: entry.snapshot }) };
}

// 툴바 단추는 팝업 없이 곧장 그 창의 패널을 연다(ADR-046).
chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: true }).catch(() => {});

// 묻는 것은 문서 만들기 패널뿐이다. 탭(페이지)에서는 띄운 단추의 「패널 열기」 하나만 받는다 — 확장 자신의 단추(chip.js)가
// 나라장터 탭에서 보낸 것만. sidePanel.open 은 사람 동작 안에서만 되므로 기다리지 않고 곧바로 연다.
// shortcut: 패널이 이미 열려 있어도 단추를 숨기지 않는다 — 열린 패널을 알 길이 생기면 그때 숨긴다.
chrome.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== chrome.runtime.id) return;
  if (sender.tab) {
    if (message?.type === "pclm-open-panel" && Number.isInteger(sender.tab.id) && isG2b(sender.tab.url))
      chrome.sidePanel.open({ windowId: sender.tab.windowId }).catch(() => {});
    return;
  }
  if (message?.type === "pclm-document") {
    extensionWork(() => documentCall(message)).then(respond, error => respond({ kind: "failed", message: error.message, connection: Boolean(error.connection) }));
    return true;
  }
});

chrome.tabs.onActivated?.addListener(info => {
  notifyPanel(Number.isInteger(info?.windowId) ? info.windowId : null);
  if (presence) refreshFront().catch(() => {});
});
chrome.tabs.onUpdated?.addListener((tabId, change, tab) => {
  if (!presence) return;
  if (!isG2b(tab?.url)) {
    if (mirror.tabs.has(tabId)) { forgetTab(tabId); if (tab?.active) refreshFront().catch(() => {}); }
    return;
  }
  trackTab(tab);
  if (change.status === "loading" || change.url) {
    mirror.generations.set(tabId, (mirror.generations.get(tabId) || 0) + 1);
    mirror.tabs.set(tabId, { ...mirror.tabs.get(tabId), state: "reading", readiness: undefined });
    report();
  }
  if (change.status === "complete") { mirror.prints.delete(tabId); readTab(tabId).catch(() => {}); }
  if (tab.active && mirror.front !== tabId) refreshFront().catch(() => {});
});
chrome.tabs.onRemoved.addListener(tabId => forgetTab(tabId));
chrome.permissions?.onAdded?.addListener(() => { refreshSettings().catch(() => {}); });
chrome.permissions?.onRemoved?.addListener(() => { refreshSettings().catch(() => {}); });
chrome.windows?.onFocusChanged?.addListener(windowId => {
  if (presence && windowId !== chrome.windows.WINDOW_ID_NONE) refreshFront().catch(() => {});
});

// 맨 끝에서 잇는다 — 포트가 열리면 곧장 위의 mirror · panelSeen 등을 쓰므로 그 선언이 다 선 뒤여야 한다.
chrome.runtime.onStartup.addListener(() => { connectPresence().catch(() => retryPresence()); });
chrome.runtime.onInstalled.addListener(() => { connectPresence().catch(() => retryPresence()); });
connectPresence().catch(() => retryPresence());
