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
    throw failure("저장 대상의 응답을 확인하지 못했습니다.");
  if (!response.ok) {
    if (!response.error?.code || typeof response.error.message !== "string")
      throw failure("저장 대상의 응답을 확인하지 못했습니다.");
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
      try { return await complete(await rpc("capture", pending, pending.host), pending); }
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
    return { ...(pending ? target(pending) : {}), kind: pending ? "pending" : "blocked",
      previous: Boolean(pending),
      message: pending ? "저장 결과가 미확인입니다. 다시 저장하기 전에 결과를 확인해 주세요." : error.message,
      connection: Boolean(error.connection), detail: error.detail || error.message };
  } finally { busy = false; }
}

// 저장 위치는 설정 창에서만 보고 바꾼다. 페이지 수집기·팝업은 자리를 옮길 수 없다.
// 선택된 호스트를 따르고, 개발 DB 면 호스트가 거절한다. 옮기기는 예약뿐이다 — 실제 이동은 앱 다음 실행 맨 앞.
async function dataLocation(message, sender) {
  const page = String(sender.url || "").split(/[?#]/)[0];
  if (sender.id !== chrome.runtime.id || page !== chrome.runtime.getURL("options.html"))
    throw failure("설정 창에서만 저장 위치를 바꿀 수 있습니다.");
  if (message.action === "location") return rpc("dataLocation");
  if (message.action === "moveLocation") {
    if (typeof message.datasetId !== "string" || typeof message.folder !== "string" || !message.folder.trim())
      throw failure("옮길 폴더를 입력하세요.");
    return rpc("stageDataMove", { datasetId: message.datasetId, folder: message.folder.trim() });
  }
  throw failure("지원하지 않는 요청입니다.");
}

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

// 사용자가 고를 것이 하나라도 남았으면 단축키로 저장하지 않는다.
function directSave(view) {
  return view?.kind === "ready" && !view.previous && !view.unmatched?.length &&
    !(view.changes || []).some(change => change.conflict);
}
async function command(tab) {
  let page = false;
  try { page = Boolean(tab?.id && tab.url && new URL(tab.url).origin === origin); } catch { /* 주소 없음 */ }
  if (!page) { if (tab?.id) await flash(tab.id, false); return; }
  // 명령 이벤트는 페이지가 합성할 수 없다. 탭은 팝업과 같이 readScreen 이 origin 을 다시 확인하고 최상위 프레임만 읽는다.
  const scope = { page: false, tabId: tab.id };
  let view = await run("inspect", scope);
  if (directSave(view)) view = await run("save", scope, view.captureId, view.preview, {});
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
  if (message?.type === "pclm-location") {
    dataLocation(message, sender).then(result => respond({ ok: true, result }),
      error => respond({ ok: false, message: error.message, connection: Boolean(error.connection) }));
    return true;
  }
  if (message?.type !== "pclm-capture") return;
  settle(handle(message.action, sender, message.captureId, message.preview, message.choices)).then(respond, () => respond({ kind: "blocked", message: "허용되지 않은 수집 요청입니다." }));
  return true;
});
chrome.commands.onCommand.addListener((name, tab) => {
  if (name === shortcutName) settle(command(tab)).catch(() => {});
});
chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && changes.panelMode) pendingBadge().catch(() => {});
});
// 확장을 새로 깔거나 올리거나 다시 불러오면, 그 전에 열린 나라장터 탭의 페이지 수집기는 끊긴다 —
// 탭을 새로 고치기 전까지 설정 전환도 단축키 결과도 닿지 않아 「항상 표시」 가 먹지 않는 것처럼 보였다.
// 열린 탭에 다시 넣는다. 새 수집기가 끊긴 옛 것을 물러나게 한다(popup.js).
chrome.runtime.onInstalled.addListener(() => {
  chrome.tabs.query({ url: origin + "/*" }).then(tabs => Promise.all(tabs.map(tab =>
    chrome.scripting.executeScript({ target: { tabId: tab.id }, files: ["capture.js", "export.js", "popup.js"] })
      .catch(() => {})))).catch(() => {});
});
