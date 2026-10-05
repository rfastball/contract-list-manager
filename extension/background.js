const origin = "https://www.g2b.go.kr";
let busy = false;
const failure = (message, extra = {}) => Object.assign(new Error(message), extra);

async function rpc(method, params, selectedHost) {
  const host = selectedHost || ( (await chrome.storage.local.get("erpDevelopment")).erpDevelopment ? "kr.rfastball.pclm.erp.dev" : "kr.rfastball.pclm.erp");
  const requestId = crypto.randomUUID();
  let response;
  try {
    response = await chrome.runtime.sendNativeMessage(host, { protocolVersion: 2, requestId, method, params });
  } catch (error) {
    throw failure("저장 대상에 연결하지 못했습니다.", { connection: true, detail: error.message });
  }
  if (response?.protocolVersion !== 2 || response.requestId !== requestId || typeof response.ok !== "boolean")
    throw failure("저장 대상의 응답을 확인하지 못했습니다.", { response: true });
  if (!response.ok) {
    if (!response.error?.code || typeof response.error.message !== "string")
      throw failure("저장 대상의 응답을 확인하지 못했습니다.", { response: true });
    const connection = ["setup_required", "schema_mismatch", "wrong_environment"].includes(response.error.code);
    throw failure(connection ? "저장 대상 준비가 필요합니다. 연결 도움말을 확인해 주세요." : response.error.message,
      { code: response.error.code, connection, detail: response.error.message });
  }
  return response.result;
}

function browserName() {
  const brands = (navigator.userAgentData?.brands || []).map(b => b.brand);
  return brands.includes("Microsoft Edge") ? "Edge" : brands.includes("Google Chrome") ? "Chrome" : "기타";
}

// 앱을 새 판으로 바꾸면 앱이 확장 파일을 고쳐 두지만, 브라우저는 다시 불러오기 전까지 옛 판을 돈다.
// 호스트가 알려 준 디스크 판이 다르면 스스로 다시 불러온다. 같은 쌍으로 두 번은 하지 않는다 —
// 다른 폴더에서 로드한 확장이면 다시 불러와도 판이 바뀌지 않아 끝없이 되풀이된다.
// readScreen 은 미확인 저장(pendingCapture)을 해결한 뒤에만 불리므로, 다시 불러오며 session 이 지워져도 잃는 것이 없다.
async function reloadIfStale(hello) {
  const current = chrome.runtime.getManifest().version;
  if (typeof hello?.extensionVersion !== "string" || hello.extensionVersion === current) return;
  const attempt = `${current}→${hello.extensionVersion}`;
  if ((await chrome.storage.local.get("reloadAttempt")).reloadAttempt === attempt) return;
  await chrome.storage.local.set({ reloadAttempt: attempt });
  setTimeout(() => chrome.runtime.reload(), 300); // 응답이 나갈 틈을 준다.
  throw failure(`확장을 새 판(${hello.extensionVersion})으로 다시 불러옵니다. 잠시 뒤 화면을 다시 확인해 주세요.`);
}

async function readScreen(tabId) {
  const tab = await chrome.tabs.get(tabId);
  if (!tab.url || new URL(tab.url).origin !== origin)
    throw failure("나라장터 접수·공고·계약 상세 화면을 열어 주세요.");
  const selectedHost = (await chrome.storage.local.get("erpDevelopment")).erpDevelopment ? "kr.rfastball.pclm.erp.dev" : "kr.rfastball.pclm.erp";
  const hello = await rpc("hello", { extensionVersion: chrome.runtime.getManifest().version, browser: browserName() }, selectedHost);
  await reloadIfStale(hello);
  await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", files: ["capture.js"] });
  // 수집하는 화면(ADR-037)은 hello 의 매핑에서 screen 을 단 프로필이다. 옛 호스트는 싣지 않아 어느 화면도 받지 않는다.
  const [result] = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", args: [hello.mapping.profiles], func: profiles => {
    try { return { snapshot: pclmReadMapped(profiles) }; }
    catch (error) { return { error: error.message }; }
  } });
  if (!result?.result?.snapshot) throw failure(result?.result?.error || "화면을 읽지 못했습니다.");
  return { tabId, documentId: result.documentId, hello, host: selectedHost, snapshot: { ...result.result.snapshot, mappingRevision: hello.mappingRevision } };
}

function target(request) {
  return { captureId: request.captureId, entity: request.entity,
    entityType: request.entityType, fields: request.fields || {}, destination: request.destination,
    environment: request.environment, itemCount: request.itemCount };
}
async function clearPending() {
  await chrome.storage.session.remove(["pendingCapture", "pendingRetryAllowed"]);
}
async function complete(result, pending) {
  if (result?.status !== "stored" || result.entity !== pending.entity)
    throw failure("저장 결과를 확인하지 못했습니다.");
  await clearPending();
  return { ...target(pending), kind: "stored", message: result.changed === false
    ? "이미 같은 자료가 저장되어 있습니다." : "검토한 자료를 저장했습니다." };
}
async function recover(pending) {
  const result = await rpc("captureStatus", { datasetId: pending.datasetId, captureId: pending.captureId }, pending.host);
  if (result?.status === "stored") return complete(result, pending);
  if (result?.status !== "not_found") throw failure("저장 결과를 확인하지 못했습니다.");
  await chrome.storage.session.set({ pendingRetryAllowed: true });
  return { ...target(pending), kind: "retry", message: "저장 기록이 없습니다. 확인한 같은 요청을 다시 저장할 수 있습니다." };
}

// 메시지 발신자를 수집 범위로 바꾼다. 팝업은 활성 탭을, 페이지 수집기는 자기 탭의 최상위 문서만 가리킨다.
function scopeOf(sender) {
  const popup = sender.url === chrome.runtime.getURL("popup.html") && !sender.tab;
  if (sender.id !== chrome.runtime.id || (!popup &&
      (sender.frameId !== 0 || !sender.tab?.id || new URL(sender.url).origin !== origin)))
    throw failure("허용되지 않은 수집 요청입니다.");
  return popup ? { page: false } : { page: true, tabId: sender.tab.id, documentId: sender.documentId };
}
async function handle(action, sender, expectedCaptureId, preview, choices) {
  return run(action, scopeOf(sender), expectedCaptureId, preview, choices);
}

// 모든 수집 요청(메시지·단축키)이 거치는 한 길. 탭 origin·스냅샷 재검증·busy 잠금·미확인 저장 규율이 여기 있다.
async function run(action, scope, expectedCaptureId, preview, choices = {}) {
  const activeTab = async () => scope.tabId ?? (await chrome.tabs.query({ active: true, currentWindow: true }))[0]?.id;
  if (!["inspect", "save", "recover", "retry", "export"].includes(action)) throw failure("지원하지 않는 요청입니다.");
  if (busy) return { kind: "busy", message: "다른 수집 요청을 처리하고 있습니다. 잠시 후 다시 확인해 주세요." };
  busy = true;
  let pending;
  try {
    if (action === "export") {
      const tabId = await activeTab();
      if (!tabId || new URL((await chrome.tabs.get(tabId)).url).origin !== origin)
        throw failure("나라장터 업무 화면을 열어 주세요.");
      await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", files: ["export.js"] });
      const [result] = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", func: () => {
        try { return { workbook: pclmExportPage() }; }
        catch (error) { return { error: error.message }; }
      } });
      if (scope.page && result?.documentId !== scope.documentId) throw failure("화면이 바뀌었습니다. 다시 내보내세요.");
      if (!result?.result?.workbook) throw failure(result?.result?.error || "화면을 읽지 못했습니다.");
      return { kind: "exported", ...result.result.workbook };
    }
    const session = await chrome.storage.session.get(["pendingCapture", "pendingRetryAllowed"]);
    pending = session.pendingCapture;
    // 브라우저 전체에서 미확인 요청 한 건을 먼저 해결한다. 다른 탭도 자동 재전송하지 않는다.
    if (pending) {
      if (action !== "retry" || expectedCaptureId !== pending.captureId)
        return { ...await recover(pending), previous: true };
      if (!session.pendingRetryAllowed) return await recover(pending);
      await chrome.storage.session.set({ pendingRetryAllowed: false });
      try {
        const done = await complete(await rpc("capture", pending, pending.host), pending);
        mirrorEvent("recovered");
        return done;
      }
      catch (error) {
        if (error.code) { await clearPending(); pending = undefined; }
        throw error;
      }
    }
    if (action === "recover" || action === "retry")
      return { kind: "blocked", message: "미확인 저장 요청이 없습니다. 현재 화면을 다시 확인해 주세요." };
    const tabId = await activeTab();
    if (!tabId) throw failure("현재 탭을 확인하지 못했습니다.");
    const screen = await readScreen(tabId);
    if (scope.page && screen.documentId !== scope.documentId)
      throw failure("화면이 바뀌었습니다. 현재 화면을 다시 확인해 주세요.");
    if (action === "inspect") {
      const hello = screen.hello;
      const result = await rpc("inspect", { datasetId: hello.datasetId, snapshot: screen.snapshot }, screen.host);
      // 미리보기 증거는 UI에만 둔다. 워커가 종료되어도 저장 시 화면·C# 토큰을 다시 검증한다.
      const preview = { host: screen.host, tabId, documentId: screen.documentId, datasetId: hello.datasetId,
        baseToken: result.baseToken, fingerprint: JSON.stringify(screen.snapshot), entity: result.entity,
        entityType: result.entityType, fields: result.document.rows[0].values, itemCount: result.itemCount,
        destination: hello.destination, environment: hello.environment };
      return { ...target(preview), changes: result.changes, unmatched: result.unmatched, kind: result.unmatched.length ? "blocked" : "ready", fingerprint: JSON.stringify(screen.snapshot),
        preview, message: result.unmatched.length ? "기존 품목과의 대응은 메인 프로그램의 고급 도구에서 검토하세요." :
          `품목 ${result.itemCount}행 · 변경값을 검토한 뒤 저장하세요.` };
    }
    if (!preview || preview.host !== screen.host || preview.tabId !== tabId || typeof preview.datasetId !== "string" || typeof preview.baseToken !== "string" ||
        screen.documentId !== preview.documentId || JSON.stringify(screen.snapshot) !== preview.fingerprint)
      throw failure("확인한 화면이 바뀌었거나 확인 시간이 지났습니다. 다시 확인해 주세요.");
    const candidate = { host: screen.host, datasetId: preview.datasetId, baseToken: preview.baseToken,
      captureId: crypto.randomUUID(), snapshot: screen.snapshot, choices,
      entity: preview.entity, entityType: preview.entityType, fields: preview.fields, itemCount: preview.itemCount,
      destination: preview.destination, environment: preview.environment };
    await chrome.storage.session.set({ pendingCapture: candidate, pendingRetryAllowed: false });
    pending = candidate;
    try { return await complete(await rpc("capture", pending, pending.host), pending); }
    catch (error) {
      if (error.code) { await clearPending(); pending = undefined; }
      throw error;
    }
  } catch (error) {
    const code = failureCode(error, pending);
    if (code) mirrorEvent("error", code);
    return { ...(pending ? target(pending) : {}), kind: pending ? "pending" : "blocked",
      previous: Boolean(pending),
      message: pending ? "저장 결과가 미확인입니다. 다시 저장하기 전에 결과를 확인해 주세요." : error.message,
      connection: Boolean(error.connection), detail: error.detail || error.message };
  } finally { busy = false; }
}

// 자료 자리를 보거나 옮기는 길은 두지 않는다. 수집 통로가 저장 구조를 고치는 통로를 겸하면 브라우저 쪽의
// 실수 하나가 사람의 자료 자리를 바꾼다 — 자리는 계약 목록 앱에서만 다룬다. 저장 대상은 hello 의 destination 이 보인다.

const shortcutName = "save-current";
const badgeColor = { ok: "#1e8449", warn: "#a05a00" };
// 툴바 배지는 탭 단위로 잠깐 보인다. null 로 지우면 전역 배지(미확인 저장 !)로 돌아간다.
async function flash(tabId, ok) {
  await chrome.action.setBadgeBackgroundColor({ tabId, color: ok ? badgeColor.ok : badgeColor.warn });
  await chrome.action.setBadgeText({ tabId, text: ok ? "✓" : "!" });
  setTimeout(() => chrome.action.setBadgeText({ tabId, text: null }).catch(() => {}), 5000);
}
// 페이지 수집기를 꺼 두면 미확인 저장을 알릴 곳이 없다. 강제로 띄우던 패널 대신 전역 배지로 남긴다.
async function pendingBadge() {
  const [{ pendingCapture }, { panelMode }] = await Promise.all([
    chrome.storage.session.get("pendingCapture"), chrome.storage.local.get("panelMode")]);
  await chrome.action.setBadgeBackgroundColor({ color: badgeColor.warn });
  await chrome.action.setBadgeText({ text: pendingCapture && panelMode === "button" ? "!" : "" });
}
const settle = promise => promise.finally(() => pendingBadge().catch(() => {}));

// 있던 값을 빈 값으로 지우는 변경. 늦게 그려지는 탭을 빈 칸으로 읽은 것이 대부분이라 팝업도 단축키도
// 「기존값 유지」 를 기본으로 둔다(popup.js 의 wipe 와 같은 판정).
const wipes = change => change.conflict && !change.override && change.field !== "__row" &&
  (change.after ?? "") === "" && (change.before ?? "") !== "";
// 단축키 저장의 선택. 빈 값 지우기는 알아서 유지하고, 그 밖에 사용자가 고를 것이 하나라도 남았으면
// 저장하지 않는다(undefined).
function directChoices(view) {
  if (view?.kind !== "ready" || view.previous || view.unmatched?.length) return undefined;
  const conflicts = (view.changes || []).filter(change => change.conflict);
  if (!conflicts.every(wipes)) return undefined;
  return Object.fromEntries(conflicts.map(change => [change.id, "keep"]));
}
async function command(tab) {
  let page = false;
  try { page = Boolean(tab?.id && tab.url && new URL(tab.url).origin === origin); } catch { /* 주소 없음 */ }
  if (!page) { if (tab?.id) await flash(tab.id, false); return; }
  // 명령 이벤트는 페이지가 합성할 수 없다. 탭은 팝업과 같이 readScreen 이 origin 을 다시 확인하고 최상위 프레임만 읽는다.
  const scope = { page: false, tabId: tab.id };
  let view = await run("inspect", scope);
  const choices = directChoices(view);
  if (choices) view = await run("save", scope, view.captureId, view.preview, choices);
  else if (view.kind === "ready") view = { ...view, message: "검토가 필요합니다. 충돌한 변경값을 고른 뒤 저장하세요." };
  const outcome = view.kind === "stored" && !view.previous ? "stored" : view.kind === "blocked" && view.detail ? "error" : "review";
  const shown = await chrome.tabs.sendMessage(tab.id, { type: "pclm-command-result", view }, { frameId: 0 }).catch(() => false);
  if (shown === true) return;
  await flash(tab.id, outcome === "stored");
  if (outcome === "review") await chrome.action.openPopup().catch(() => {});
}

chrome.runtime.onMessage.addListener((message, sender, respond) => {
  const own = sender.id === chrome.runtime.id;
  if (message?.type === "pclm-shortcut" && own) {
    chrome.commands.getAll().then(all => respond(all.find(c => c.name === shortcutName)?.shortcut || ""), () => respond(""));
    return true;
  }
  if (message?.type === "pclm-options" && own) { chrome.runtime.openOptionsPage(); return; }
  if (message?.type !== "pclm-capture") return;
  settle(handle(message.action, sender, message.captureId, message.preview, message.choices)).then(respond, () => respond({ kind: "blocked", message: "허용되지 않은 수집 요청입니다." }));
  return true;
});
chrome.commands.onCommand.addListener((name, tab) => {
  if (name === shortcutName) settle(command(tab)).catch(() => {});
});
// 상시 연결(ADR-035). 포트가 열려 있는 동안 브라우저는 워커를 살려 두고 호스트는 포트만큼 산다 — 앱은 호스트가
// 남긴 프로세스별 파일로 확장이 지금 살아 있는지 안다. 인사만 하는 포트라 수집 RPC 는 여전히 rpc() 로 간다.
// 호스트는 제 exe 가 비켜지면(새 판을 놓으려고 옛 exe 의 이름을 바꿨다) 스스로 떠난다 — 그러면 물러서며 다시 이어
// 그 자리의 새 exe 에 붙는다.
const presenceDelays = [1000, 2000, 5000, 15000, 60000];
let presence = null, presenceOpening = false, presenceTimer = null;
let presenceRetry = 0, presenceSince = 0, presenceGeneration = 0;
function retryPresence() {
  if (presenceTimer) return;
  const delay = presenceDelays[Math.min(presenceRetry++, presenceDelays.length - 1)];
  presenceTimer = setTimeout(() => { presenceTimer = null; connectPresence().catch(() => retryPresence()); }, delay);
}
async function connectPresence() {
  if (presence || presenceOpening) return;
  clearTimeout(presenceTimer); presenceTimer = null;
  presenceOpening = true;
  const generation = presenceGeneration;
  let host;
  try { host = (await chrome.storage.local.get("erpDevelopment")).erpDevelopment ? "kr.rfastball.pclm.erp.dev" : "kr.rfastball.pclm.erp"; }
  catch { return retryPresence(); }
  finally { presenceOpening = false; }
  // 호스트를 고르는 사이 개발 전환이 바뀌었으면 고른 것을 버리고 다시 고른다.
  if (generation !== presenceGeneration) return connectPresence();
  if (presence) return;
  const port = chrome.runtime.connectNative(host);
  presence = port; presenceSince = Date.now();
  // 응답은 쓰지 않는다 — 포트가 살아 있다는 것만이 신호다. 앱이 밀어 보내는 명령(ADR-036)만 받는다.
  port.onMessage.addListener(message => { if (presence === port) mirrorCommand(message); });
  port.onDisconnect.addListener(() => {
    void chrome.runtime.lastError; // 읽지 않으면 브라우저가 확인하지 않은 오류로 남긴다.
    if (presence !== port) return; // 일부러 끊은 옛 포트.
    presence = null;
    stopMirror();
    if (Date.now() - presenceSince >= 60000) presenceRetry = 0;
    retryPresence();
  });
  port.postMessage({ protocolVersion: 2, requestId: crypto.randomUUID(), method: "presence",
    params: { extensionVersion: chrome.runtime.getManifest().version, browser: browserName() } });
  startMirror().catch(() => {});
}
function switchPresence() {
  presenceGeneration++;
  const port = presence;
  presence = null; presenceRetry = 0;
  stopMirror();
  clearTimeout(presenceTimer); presenceTimer = null;
  try { port?.disconnect(); } catch { /* 이미 끊겼다 */ }
  connectPresence().catch(() => retryPresence());
}
chrome.runtime.onStartup.addListener(() => { connectPresence().catch(() => retryPresence()); });
connectPresence().catch(() => retryPresence());

chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && changes.panelMode) { pendingBadge().catch(() => {}); refreshSettings().catch(() => {}); }
  if (area === "local" && changes.erpDevelopment) switchPresence();
});
// 확장을 새로 깔거나 올리거나 다시 불러오면, 그 전에 열린 나라장터 탭의 페이지 수집기는 끊긴다 —
// 탭을 새로 고치기 전까지 설정 전환도 단축키 결과도 닿지 않아 「항상 표시」 가 먹지 않는 것처럼 보였다.
// 열린 탭에 다시 넣는다. 새 수집기가 끊긴 옛 것을 물러나게 한다(popup.js).
chrome.runtime.onInstalled.addListener(() => {
  chrome.tabs.query({ url: origin + "/*" }).then(tabs => Promise.all(tabs.map(tab =>
    chrome.scripting.executeScript({ target: { tabId: tab.id }, files: ["capture.js", "export.js", "popup.js"] })
      .catch(() => {})))).catch(() => {});
});

// ── 지금 보는 화면(ADR-036) ─────────────────────────────────
// 상시 포트가 열려 있는 동안만 나라장터 탭을 알고, 읽고, 앱에 보고한다 — 포트가 없으면(앱 호스트 없음) 아무것도 하지
// 않는다. 호스트는 받은 보고를 홈의 파일에 적고 창이 그것을 읽는다. 창의 명령(그 탭을 앞으로·다시 읽기·엑셀로 내보내기)은
// 같은 포트로 돌아온다. 기존 수집기·팝업·단축키의 길(run)은 건드리지 않는다 — 읽기만 하고 저장하지 않는다.
// 읽는 때: 탭이 앞으로 올 때, 로드가 끝날 때, 창이 부를 때, 그리고 앞 탭은 가벼운 지문을 2초마다 보아 바뀌었을 때
// (WebSquare 는 주소 없이 화면을 바꾼다). 뒤의 탭은 처음 한 번과 부를 때만 읽는다.
const mirrorLoading = "화면을 불러오는 중입니다.";
const mirror = { tabs: new Map(), front: null, frontAt: 0, timer: null, poll: null, sent: "", hello: null, helloAt: 0,
  prints: new Map(), reading: new Set(), again: new Set(), retries: new Map(), settings: null };
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
  const hello = await rpc("hello", { extensionVersion: chrome.runtime.getManifest().version, browser: browserName() });
  mirror.hello = hello; mirror.helloAt = Date.now();
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
  mirror.prints.delete(tabId); mirror.retries.delete(tabId);
  if (!mirror.tabs.delete(tabId)) return;
  if (mirror.front === tabId) { mirror.front = null; mirror.frontAt = Date.now(); }
  report();
}

async function readTab(tabId) {
  if (!presence || !mirror.tabs.has(tabId)) return;
  if (mirror.reading.has(tabId)) { mirror.again.add(tabId); return; }
  mirror.reading.add(tabId);
  mirror.tabs.set(tabId, { ...mirror.tabs.get(tabId), state: "reading" });
  report();
  let next, retry = false;
  try {
    const hello = await mirrorHello();
    await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", files: ["capture.js", "export.js"] });
    const [result] = await chrome.scripting.executeScript({ target: { tabId }, world: "MAIN", args: [hello.mapping.profiles], func: profiles => {
      const screen = document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")?.textContent.trim() || "";
      // 메뉴 번호(ADR-037) — 창이 「수집 안 함」 곁에 보인다. 모르면 빈 글.
      let menu = ""; try { menu = pclmScreenCode(); } catch { /* 모른다 */ }
      // 화면의 이름표와 보이는 글(「화면 그대로」). 읽지 못해도 스냅샷은 그대로 낸다.
      const rows = (profile, snapshot) => { try { return pclmScreenRows(profile, snapshot); } catch { return []; } };
      try {
        const snapshot = pclmReadMapped(profiles);
        return { screen, menu, snapshot, screenRows: rows(profiles.find(p => p.id === snapshot.profile), snapshot) };
      }
      catch (error) { return { screen, menu, error: error.message, unsupported: Boolean(error.unsupported), screenRows: rows(null) }; }
    } });
    const read = result?.result || {};
    const menu = typeof read.menu === "string" && /^\d{5}$/.test(read.menu) ? read.menu : "";
    const screenRows = Array.isArray(read.screenRows) ? read.screenRows.slice(0, 400) : [];
    if (read.snapshot) {
      const snapshot = { ...read.snapshot, mappingRevision: hello.mappingRevision };
      next = byteLength(JSON.stringify(snapshot)) > (hello.maxRequestBytes || 768 * 1024) - 4096
        ? { state: "error", screen: read.screen, menu, message: "화면 자료가 커서 미리 볼 수 없습니다. 수집기에서 저장하세요." }
        : { state: "supported", screen: read.screen, menu, snapshot, screenRows };
    } else if (read.error === mirrorLoading && (mirror.retries.get(tabId) || 0) < 5) retry = true;
    else if (read.unsupported) next = { state: "unsupported", screen: read.screen, menu, screenRows };
    else next = { state: "error", screen: read.screen || "", menu, message: read.error || "화면을 읽지 못했습니다." };
  } catch (error) { next = { state: "error", screen: "", menu: "", message: error.message }; }
  finally { mirror.reading.delete(tabId); }
  const entry = mirror.tabs.get(tabId);
  if (!entry) return;
  if (retry) {
    // 화면이 아직 그려지는 중이다. 조금 뒤 다시 본다.
    mirror.retries.set(tabId, (mirror.retries.get(tabId) || 0) + 1);
    setTimeout(() => { readTab(tabId).catch(() => {}); }, 1000);
    return;
  }
  mirror.retries.delete(tabId);
  mirror.tabs.set(tabId, { tabId, windowId: entry.windowId, title: entry.title, readAt: localStamp(), ...next });
  report();
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

// 앞 탭의 가벼운 지문 — 머리 제목과 업무 칸의 값. 바뀌었으면 다시 읽는다. 처음 본 지문은 기준으로만 둔다.
async function watchFront() {
  const tabId = mirror.front;
  if (!presence || tabId === null || mirror.reading.has(tabId)) return;
  const [result] = await chrome.scripting.executeScript({ target: { tabId }, func: () =>
    (document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")?.textContent || "") + "␟" +
      [...document.querySelectorAll('[id^="mf_wfm_container"] input')].map(e => e.value).join("␟") });
  const print = result?.result;
  if (typeof print !== "string") return;
  const seen = mirror.prints.get(tabId);
  mirror.prints.set(tabId, print);
  if (seen !== undefined && seen !== print) await readTab(tabId);
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
  const params = () => ({ front: mirror.front, frontAt: mirror.frontAt, tabs, settings: mirror.settings });
  let text = JSON.stringify(params());
  for (const victim of tabs.filter(t => t.snapshot && t.tabId !== mirror.front)) {
    if (byteLength(text) <= limit) break;
    tabs = tabs.map(t => t === victim ? { tabId: t.tabId, windowId: t.windowId, title: t.title, readAt: t.readAt, screen: t.screen, menu: t.menu,
      state: "error", message: "열린 탭이 많아 이 화면은 앞으로 가져오면 다시 읽습니다." } : t);
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

// 확장 설정(「Chrome 확장 상태」 의 설정) — 수집기를 띄우는 방식, 현재 화면 바로 저장의 단축키, 나라장터 사이트 접근.
// 창이 보고 바꾼다(setPanelMode·openShortcuts). 바뀌면 다시 보고한다 — 단축키는 바뀌어도 알림이 없어 앞 탭을 볼 때마다 다시 본다.
async function readSettings() {
  const [{ panelMode }, commands, siteAccess] = await Promise.all([
    chrome.storage.local.get("panelMode").catch(() => ({})),
    chrome.commands.getAll().catch(() => []),
    chrome.permissions?.contains ? chrome.permissions.contains({ origins: [origin + "/*"] }).catch(() => false) : Promise.resolve(false),
  ]);
  return { panelMode: panelMode === "button" ? "button" : "always",
    shortcut: commands.find(c => c.name === shortcutName)?.shortcut || "", siteAccess: Boolean(siteAccess) };
}
async function refreshSettings() {
  if (!presence) return;
  const next = await readSettings();
  if (JSON.stringify(next) === JSON.stringify(mirror.settings)) return;
  mirror.settings = next;
  report();
}

// 수집 흐름의 사건 — 실패의 코드, 결과를 확인하지 못한 저장을 같은 요청으로 다시 보내 푼 것. 연결 기록에 남아 「Chrome 확장
// 상태」 의 오류와 재시도가 된다. 값·번호·건명 같은 자료는 싣지 않는다 — 코드만. 포트가 없으면 버린다.
function mirrorEvent(kind, code) {
  if (!presence) return;
  const params = kind === "error" ? { kind, code } : { kind };
  try { presence.postMessage({ protocolVersion: 2, requestId: crypto.randomUUID(), method: "event", params }); } catch { /* 끊겼다 */ }
}
// 실패를 코드로. 화면을 고르라는 안내(번호를 못 찾음·화면이 바뀜)는 오류가 아니라 세지 않는다.
function failureCode(error, pending) {
  if (pending) return "unconfirmed";
  if (error.connection) return error.code ? "setup" : "connection";
  if (error.code) return "rejected";
  if (error.response) return "response";
  return "";
}

async function startMirror() {
  mirror.sent = ""; // 새 호스트다 — 같은 것이라도 다시 보낸다.
  mirror.settings = await readSettings().catch(() => null);
  let tabs = [];
  try { tabs = await chrome.tabs.query({ url: origin + "/*" }); } catch { /* 모른다 */ }
  for (const tab of tabs) trackTab(tab);
  await refreshFront();
  // 처음 한 번만 읽는다. 다시 이은 것이면 읽어 둔 것을 그대로 보낸다.
  for (const [tabId, entry] of mirror.tabs) if (!entry.readAt && tabId !== mirror.front) readTab(tabId).catch(() => {});
  report();
  clearInterval(mirror.poll);
  mirror.poll = setInterval(() => { watchFront().catch(() => {}); refreshSettings().catch(() => {}); }, 2000);
}
function stopMirror() {
  clearInterval(mirror.poll); mirror.poll = null;
  clearTimeout(mirror.timer); mirror.timer = null;
}

// 앱이 보낸 명령. 탭 명령은 확장이 아는 나라장터 탭에만 닿는다. 설정 명령은 옵션 창과 같은 자리에 쓴다 — 수집기는
// 그 자리를 지켜보고 있어 기존 그대로 따른다.
function mirrorCommand(message) {
  if (message?.protocolVersion !== 2 || typeof message.command !== "string") return;
  if (message.command === "setPanelMode") {
    if (message.mode === "always" || message.mode === "button")
      chrome.storage.local.set({ panelMode: message.mode }).then(() => refreshSettings()).catch(() => {});
    return;
  }
  // 브라우저 안의 화면 — 명령줄로는 chrome://·edge:// 주소를 열 수 없어 창이 확장에 맡긴다. 새 창으로 연다: 브라우저는 앱 창
  // 뒤에 있어 스스로 앞으로 오지 못하고(focused 를 줘도), 앞으로 가져오는 것은 앱이 한다 — 앱은 새로 선 창으로 그것을 알아본다.
  // 탭으로 열면 마지막에 본 창(최소화돼 있을 수도 있다)에 섞여 앱이 가려낼 수 없다.
  if (message.command === "openShortcuts" || message.command === "openExtensions") {
    const page = message.command === "openShortcuts" ? "extensions/shortcuts" : "extensions/";
    chrome.windows.create({ url: `${browserName() === "Edge" ? "edge" : "chrome"}://${page}`, focused: true }).catch(() => {});
    return;
  }
  if (!Number.isInteger(message.tabId)) return;
  const entry = mirror.tabs.get(message.tabId);
  if (!entry) return;
  if (message.command === "read") { mirror.prints.delete(message.tabId); readTab(message.tabId).catch(() => {}); }
  else if (message.command === "focusTab")
    chrome.tabs.update(message.tabId, { active: true })
      .then(() => chrome.windows?.update(entry.windowId, { focused: true })).catch(() => {});
  else if (message.command === "export")
    chrome.tabs.sendMessage(message.tabId, { type: "pclm-export" }, { frameId: 0 }).catch(() => {});
}

chrome.tabs.onActivated?.addListener(() => { if (presence) refreshFront().catch(() => {}); });
chrome.tabs.onUpdated?.addListener((tabId, change, tab) => {
  if (!presence) return;
  if (!isG2b(tab?.url)) {
    if (mirror.tabs.has(tabId)) { forgetTab(tabId); if (tab?.active) refreshFront().catch(() => {}); }
    return;
  }
  trackTab(tab);
  if (change.status === "complete") { mirror.prints.delete(tabId); readTab(tabId).catch(() => {}); }
  if (tab.active && mirror.front !== tabId) refreshFront().catch(() => {});
});
chrome.tabs.onRemoved.addListener(tabId => forgetTab(tabId));
chrome.permissions?.onAdded?.addListener(() => { refreshSettings().catch(() => {}); });
chrome.permissions?.onRemoved?.addListener(() => { refreshSettings().catch(() => {}); });
chrome.windows?.onFocusChanged?.addListener(windowId => {
  if (presence && windowId !== chrome.windows.WINDOW_ID_NONE) refreshFront().catch(() => {});
});
