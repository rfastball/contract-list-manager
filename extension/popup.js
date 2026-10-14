(async () => {
  // 툴바 팝업 하나뿐이다 — 나라장터 화면 위에는 아무것도 그리지 않는다. 가져오기는 계약 목록 오른쪽의 「지금 보는 화면」
  // 이 하고(ADR-036), 여기는 연결이 살아 있는지와 이 탭을 그쪽에서 어떻게 보는지를 알리며, 앱 없이도 되는 엑셀
  // 내보내기 하나를 둔다.
  const byId = id => document.getElementById(id);

  // 현재 화면 엑셀로 내보내기. 중계기가 탭의 표를 읽어 주고, 파일은 여기서 지어 내려받는다. 앱이 부르는 내보내기는
  // 중계기가 그 탭 안에서 같은 이름 규칙으로 끝낸다(background.js downloadTab).
  async function exportPage() {
    const result = await chrome.runtime.sendMessage({ type: "pclm-export" });
    if (result?.kind !== "exported") throw new Error(result?.message || "화면을 읽지 못했습니다.");
    const bytes = pclmWorkbook(result.sheets);
    const url = URL.createObjectURL(new Blob([bytes], { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `${result.title.replace(/[<>:"/\\|?*\x00-\x1f]/g, "_").slice(0, 80)}_${new Date().toISOString().slice(0, 10)}.xlsx`;
    document.body.append(link); link.click(); link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 30000);
    return result;
  }

  // 작업자료는 hello 가 알려 준 경로다. 사람이 알아보는 것은 파일 이름이라 그것만 보인다.
  const fileName = path => path.split(/[\\/]/).filter(Boolean).pop() || path;
  function tabLine(tab) {
    if (!tab?.g2b) return "나라장터 화면이 아닙니다.";
    // 이어져 있다는 것은 호스트가 살아 있다는 뜻일 뿐이다 — 호스트는 브라우저가 띄우므로 창이 닫혀 있어도 이어진다.
    if (tab.state === "supported") return "이 화면은 계약 목록 창 오른쪽 「지금 보는 화면」에서 가져옵니다. 창이 닫혀 있으면 계약 목록을 여세요.";
    if (tab.state === "unsupported") return "이 화면은 수집하지 않습니다. 엑셀로는 내보낼 수 있습니다.";
    if (tab.state === "error") return tab.message || "화면을 읽지 못했습니다.";
    return "화면을 읽는 중입니다.";
  }
  function render(status) {
    const connected = Boolean(status?.connected), tab = status?.tab;
    byId("environment").textContent = status?.environment === "development" ? "개발용" : "";
    byId("connection").dataset.connected = String(connected);
    byId("connection").textContent = connected ? "계약 목록과 이어져 있습니다" : "계약 목록에 연결되지 않았습니다";
    const destination = connected && typeof status.destination === "string" && status.destination ? fileName(status.destination) : "";
    // 개발 전환이 켜져 있으면 개발용 호스트만 찾는다 — 업무용 계약 목록을 켜도 영영 이어지지 않으니 그 까닭을 먼저 말한다.
    byId("destination").textContent = connected ? destination && `작업자료 ${destination}`
      : status?.environment === "development"
        ? "확장 설정에서 「개발 DB 사용」 이 켜져 있어 개발용 계약 목록만 찾습니다. 업무용이면 설정에서 끄세요."
        : "계약 목록을 실행하면 자동으로 이어집니다.";
    byId("destination").hidden = !byId("destination").textContent;
    // 이 탭이 그쪽에서 어떻게 보이는지는 연결돼 있을 때만 뜻이 있다. 나라장터가 아니라는 것은 내보내기가 왜 꺼졌는지라 늘 보인다.
    byId("tab").hidden = !(tab && (!tab.g2b || connected));
    byId("tab").textContent = byId("tab").hidden ? "" : tabLine(tab);
    byId("tab").dataset.state = tab?.g2b ? tab.state || "reading" : "elsewhere";
    if (!exporting) byId("export").disabled = !tab?.g2b;
  }

  let exporting = false;
  byId("settings").onclick = event => { if (event.isTrusted) chrome.runtime.openOptionsPage(); };
  byId("export").onclick = async event => {
    if (!event.isTrusted || byId("export").disabled) return;
    const button = byId("export"), status = byId("export-status");
    exporting = true; button.disabled = true; button.textContent = "엑셀 파일 만드는 중…";
    status.textContent = "현재 화면의 필드와 표를 읽고 있습니다.";
    try {
      const result = await exportPage();
      status.textContent = `${result.sheets.length - 1}개 자료 시트의 다운로드를 요청했습니다. 현재 로딩된 범위만 포함합니다.` +
        (result.warnings.length ? ` 미수집 ${result.warnings.length}건은 파일의 ‘수집 정보’를 확인하세요.` : "");
    } catch (error) { status.textContent = `${error.message} 다시 시도해 주세요.`; }
    finally { exporting = false; button.disabled = false; button.textContent = "현재 화면 엑셀로 내보내기"; }
  };

  async function refresh() {
    let status = null;
    try { status = await chrome.runtime.sendMessage({ type: "pclm-status" }); } catch { /* 중계기가 답하지 않는다 — 연결 안 됨으로 그린다 */ }
    render(status);
  }
  await refresh();
  // 열려 있는 동안 2초마다 다시 읽는다 — 연 다음에 앱을 켜거나 탭을 옮겨도 팝업이 연 순간에 머물지 않는다.
  // 그리던 화면이 걷혔으면(다른 팝업이 같은 문서에 섰으면) 멈춘다.
  const root = byId("collector");
  const timer = setInterval(() => { if (byId("collector") !== root) clearInterval(timer); else void refresh(); }, 2000);
})();
