(async () => {
  const inPage = location.origin === "https://www.g2b.go.kr";
  const labels = { title: "공고명", notice_kind: "공고종류", posted_at: "게시일시",
    award_method: "낙찰방법", contract_method: "계약방법", notice_agency: "공고기관" };

  // 수집기 한 벌. 팝업은 문서 자체에, 페이지 수집기는 shadow DOM 안에 그린다.
  async function collector(root, panel) {
    const byId = id => root.getElementById(id);
    let state = "blocked", busy = false, stopped = false, fingerprint, observed, captureId, preview, choices = {}, timer;
    function render(view) {
      state = view.kind;
      captureId = view.captureId;
      preview = view.preview;
      choices = {};
      byId("changes").replaceChildren();
      for (const change of view.changes || []) {
        if (!change.conflict) continue;
        const label = document.createElement("label");
        label.textContent = `${change.table} ${change.line || ""} ${change.override || change.field}: ${change.before ?? "빈값"} → ${change.after ?? "빈값"}`;
        const select = document.createElement("select");
        for (const [value, text] of [["", "선택 필요"], ["keep", "기존값 유지"], ["apply", change.override ? "수집 원값으로 복원" : change.field === "__row" ? "행 삭제" : "수집값 적용"]]) {
          const option = document.createElement("option"); option.value = value; option.textContent = text; select.append(option);
        }
        select.onchange = () => { choices[change.id] = select.value; };
        label.append(select); byId("changes").append(label);
      }
      if (view.destination) byId("destination").textContent = `저장 대상: ${view.destination}`;
      else if (view.connection) byId("destination").textContent = "DB 저장 대상 연결 안 됨";
      byId("environment").textContent = view.environment === "development" ? "개발용" : "";
      byId("scope").textContent = `품목 ${view.itemCount || 0}행 · 매핑 범위 확인됨`;
      byId("collector").dataset.state = state;
      byId("compact-status").textContent = ({ ready: "저장 가능", stored: "저장 확인됨", pending: "결과 미확인",
        retry: "재시도 가능", blocked: "확인 필요", busy: "처리 중" })[state];
      byId("status").textContent = view.message;
      byId("help").hidden = !view.connection;
      byId("error").hidden = !view.detail;
      byId("error-text").textContent = view.detail || "";
      byId("previous").hidden = !view.previous;
      if (view.entity) {
        byId("target").hidden = false;
        byId("entity").textContent = `${({ request: "접수", notice: "공고", contract: "계약" })[view.entityType] || "자료"} · ${view.entity}`;
        byId("title").textContent = view.fields?.title || "";
        byId("preview").replaceChildren();
        for (const [key, value] of Object.entries(view.fields || {})) {
          if (key === "title") continue; // 제목은 대상 요약에서 항상 보인다.
          const dt = document.createElement("dt"), dd = document.createElement("dd");
          dt.textContent = labels[key] || key; dd.textContent = value; byId("preview").append(dt, dd);
        }
      } else if (state === "blocked") byId("target").hidden = true;
      byId("action").textContent = ({ ready: "검토한 자료 저장", stored: "현재 화면 다시 확인",
        pending: "저장 결과 확인", retry: "같은 요청 다시 저장", blocked: "현재 화면 확인", busy: "다시 확인" })[state];
      byId("inspect").hidden = state !== "ready";
      if (view.fingerprint) fingerprint = view.fingerprint;
      if (panel && ["pending", "retry"].includes(state)) panel.hidden = false;
    }
    async function request(action) {
      if (busy) return;
      busy = true;
      const focused = root.activeElement;
      byId("action").disabled = byId("inspect").disabled = true;
      byId("collector").setAttribute("aria-busy", "true");
      byId("action").textContent = action === "save" || action === "retry" ? "저장 중…" : "확인 중…";
      byId("compact-status").textContent = byId("action").textContent;
      byId("status").textContent = action === "save" || action === "retry" ? "검토한 자료를 저장하고 있습니다." : "화면과 저장 결과를 확인하고 있습니다.";
      try {
        const view = await chrome.runtime.sendMessage({ type: "pclm-capture", action, captureId, preview, choices });
        if (!stopped) render(view);
      } catch (error) {
        if (!stopped) render({ kind: "pending", message: "요청 결과를 확인하지 못했습니다. 결과를 다시 확인해 주세요.", detail: error.message });
      } finally {
        busy = false;
        byId("action").disabled = byId("inspect").disabled = false;
        byId("collector").removeAttribute("aria-busy");
        if (focused === byId("inspect") && focused.hidden) byId("action").focus();
      }
    }
    byId("export").onclick = async event => {
      if (!event.isTrusted || byId("export").disabled) return;
      const button = byId("export"), status = byId("export-status");
      button.disabled = true; button.textContent = "엑셀 파일 만드는 중…";
      status.textContent = "현재 화면의 필드와 표를 읽고 있습니다.";
      try {
        const result = await chrome.runtime.sendMessage({ type: "pclm-capture", action: "export" });
        if (result.kind !== "exported") throw new Error(result.message || "화면을 읽지 못했습니다.");
        const bytes = pclmWorkbook(result.sheets);
        const url = URL.createObjectURL(new Blob([bytes], { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" }));
        const link = document.createElement("a");
        link.href = url;
        link.download = `${result.title.replace(/[<>:"/\\|?*\x00-\x1f]/g, "_").slice(0, 80)}_${new Date().toISOString().slice(0, 10)}.xlsx`;
        document.body.append(link); link.click(); link.remove();
        setTimeout(() => URL.revokeObjectURL(url), 30000);
        status.textContent = `${result.sheets.length - 1}개 자료 시트의 다운로드를 요청했습니다. 현재 로딩된 범위만 포함합니다.` +
          (result.warnings.length ? ` 미수집 ${result.warnings.length}건은 파일의 ‘수집 정보’를 확인하세요.` : "");
      } catch (error) { status.textContent = `${error.message} 다시 시도해 주세요.`; }
      finally { button.disabled = false; button.textContent = "현재 화면 엑셀로 내보내기"; }
    };
    byId("action").onclick = event => {
      // ERP 페이지 스크립트가 합성한 클릭으로 개인 DB 저장을 실행하지 않는다.
      if (event.isTrusted) request(({ ready: "save", pending: "recover", retry: "retry" })[state] || "inspect");
    };
    byId("inspect").onclick = () => request("inspect");
    byId("settings").onclick = event => {
      if (!event.isTrusted) return;
      // 콘텐츠 스크립트는 설정 창을 직접 열 수 없어 중계기에 맡긴다.
      if (inPage) chrome.runtime.sendMessage({ type: "pclm-options" }); else chrome.runtime.openOptionsPage();
    };
    // 콘텐츠 스크립트에는 chrome.commands 가 없다. 페이지 수집기는 중계기에게 묻는다.
    (inPage ? chrome.runtime.sendMessage({ type: "pclm-shortcut" })
      : chrome.commands.getAll().then(all => all.find(c => c.name === "save-current")?.shortcut)).then(shortcut => {
      byId("shortcut").textContent = shortcut ? `단축키 ${shortcut} — 현재 화면 바로 저장` : "단축키 없음 — 설정에서 지정";
    }, () => { byId("shortcut").textContent = "단축키 없음 — 설정에서 지정"; });
    if (!inPage) { await request("inspect"); return; }

    byId("collapse").hidden = false;
    byId("collapse").onclick = () => {
      const collapsed = !byId("content").hidden;
      byId("content").hidden = collapsed;
      byId("compact-status").hidden = !collapsed;
      byId("collapse").textContent = collapsed ? "펼치기" : "접기";
      byId("collapse").setAttribute("aria-expanded", String(!collapsed));
    };
    const check = async () => {
      if (stopped || busy || document.hidden || ["pending", "retry"].includes(state)) return;
      const supported = Boolean(document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle"));
      panel.hidden = false;
      if (!supported) {
        observed = fingerprint = undefined;
        render({ kind: "blocked", message: "현재 화면은 엑셀로 내보낼 수 있습니다. DB 저장은 업무 상세 화면에서 확인하세요." });
        return;
      }
      // 페이지 원문은 MAIN world 수집기에서 읽는다. 상태가 바뀔 때에만 사용자가 검토를 이어간다.
      const current = location.href + "|" + document.getElementById("mf_wfm_cntsHeader_spnHeaderTitle")?.textContent + "|" + [...document.querySelectorAll('[id^="mf_wfm_container"] input, [id^="mf_wfm_container"] select')].map(e => `${e.value}:${e.readOnly}`).join("|");
      if (current !== observed) { observed = current; await request("inspect"); }
    };
    // ponytail: 1초마다 허용된 필드만 관찰. 대형 화면에서 비용이 커지면 ERP의 화면 전환 이벤트로 교체한다.
    const hide = () => clearInterval(timer);
    const show = event => {
      if (event.persisted) { clearInterval(timer); timer = setInterval(check, 1000); check(); }
    };
    timer = setInterval(check, 1000);
    window.addEventListener("pagehide", hide);
    window.addEventListener("pageshow", show);
    await check();
    return {
      // 단축키 결과. 패널이 떠 있으면 배지 대신 여기에 그린다.
      show(view) { render(view); panel.hidden = false; },
      // 연결 환경이 바뀌면 같은 화면이라도 다시 확인한다.
      refresh() { observed = undefined; },
      stop() {
        stopped = true; clearInterval(timer);
        window.removeEventListener("pagehide", hide);
        window.removeEventListener("pageshow", show);
        panel.remove();
      },
    };
  }

  if (!inPage) { await collector(document); return; }
  // 확장을 다시 불러오면 열린 탭의 옛 수집기는 chrome API 가 끊긴 채 남아 설정 전환도 단축키 결과도
  // 받지 못한다. 중계기가 새로 넣은 쪽이 옛 것을 물러나게 한다 — DOM 이벤트는 끊기지 않아 옛 쪽도
  // 듣는다. 둘이 한꺼번에 들어와도 늦은 쪽 하나만 남는다. 이 신호를 모르는 옛 판의 패널은 직접 걷는다.
  window.dispatchEvent(new CustomEvent("pclm-takeover"));
  document.getElementById("pclm-collector")?.remove();
  let mounted, queue = Promise.resolve(), retired = false;
  window.addEventListener("pclm-takeover", () => {
    retired = true; mounted?.stop(); mounted = undefined;
    document.getElementById("pclm-toast")?.remove();
  }, { once: true });
  async function mount() {
    const [html, css] = await Promise.all(["popup.html", "popup.css"].map(async file => {
      const response = await fetch(chrome.runtime.getURL(file));
      if (!response.ok) throw new Error("수집기 화면을 불러오지 못했습니다.");
      return response.text();
    }));
    const panel = document.createElement("div");
    panel.id = "pclm-collector";
    panel.style.cssText = "all:initial;position:fixed;right:16px;bottom:16px;width:min(360px,calc(100vw - 32px));max-height:calc(100dvh - 32px);overflow:auto;z-index:2147483647;border-radius:9px;box-shadow:0 6px 24px rgba(0,0,0,.24);";
    panel.hidden = true;
    const root = panel.attachShadow({ mode: "open" });
    const style = document.createElement("style"); style.textContent = css;
    root.append(style, new DOMParser().parseFromString(html, "text/html").getElementById("collector"));
    document.documentElement.append(panel);
    mounted = await collector(root, panel);
    if (retired) { mounted.stop(); mounted = undefined; }
  }
  // 설정 창의 표시 방식을 차례로 반영한다. 「툴바 버튼으로만」이면 패널도 1초 관찰도 두지 않는다.
  const apply = mode => queue = queue.then(() => {
    if (retired) return;
    if (mode === "button") { mounted?.stop(); mounted = undefined; }
    else if (!mounted) return mount();
  }).catch(() => {});
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area !== "local" || retired) return;
    if (changes.panelMode) apply(changes.panelMode.newValue);
    if (changes.erpDevelopment) mounted?.refresh();
  });
  // 단축키 결과 알림. 툴바 배지의 작은 ✓ 나 패널 안 상태 글자만으로는 저장됐는지 알기 어렵다 —
  // 표시 방식과 관계없이 화면 위쪽 가운데에 크게 띄운다. 페이지 스타일이 닿지 않게 shadow DOM 에 그린다.
  let toastTimer;
  function toast(view) {
    const outcome = view?.kind === "stored" && !view.previous ? "ok" : view?.kind === "blocked" && view.detail ? "error" : "review";
    document.getElementById("pclm-toast")?.remove();
    clearTimeout(toastTimer);
    const host = document.createElement("div");
    host.id = "pclm-toast";
    host.style.cssText = "all:initial;position:fixed;top:24px;left:50%;transform:translateX(-50%);z-index:2147483647;";
    const root = host.attachShadow({ mode: "open" });
    const style = document.createElement("style");
    style.textContent = `
      .toast { --ok:#1e8449; --review:#a05a00; --error:#b3261e; --card:#fff; --ink:#1c2126; --muted:#5c626b;
        display:flex; gap:12px; align-items:flex-start; min-width:320px; max-width:min(560px, calc(100vw - 32px));
        padding:14px 18px; border-radius:9px; border-left:6px solid var(--tone); background:var(--card); color:var(--ink);
        box-shadow:0 8px 28px rgba(0,0,0,.28); cursor:pointer; overflow-wrap:anywhere;
        font:14px/1.5 "Pretendard GOV Variable","Malgun Gothic",system-ui,sans-serif; }
      .ok { --tone:var(--ok); } .review { --tone:var(--review); } .error { --tone:var(--error); }
      .mark { flex:none; width:28px; height:28px; border-radius:50%; display:grid; place-items:center;
        background:var(--tone); color:#fff; font-weight:700; font-size:16px; }
      strong { display:block; font-size:16px; font-weight:600; color:var(--tone); }
      span.sub { display:block; color:var(--muted); font-size:13px; margin-top:2px; }
      @media (prefers-color-scheme: dark) { .toast { --ok:#43b877; --review:#e0a850; --error:#f2867b; --card:#1c2127; --ink:#e7eaee; --muted:#aab1bb; } .mark { color:#0f1418; } }
      @media (prefers-reduced-motion: no-preference) { .toast { animation:in .16s ease-out; } @keyframes in { from { opacity:0; transform:translateY(-8px); } } }`;
    const box = document.createElement("div");
    box.className = `toast ${outcome}`;
    box.setAttribute("role", outcome === "error" ? "alert" : "status");
    const mark = document.createElement("span");
    mark.className = "mark"; mark.textContent = ({ ok: "✓", review: "!", error: "×" })[outcome];
    const text = document.createElement("div");
    const head = document.createElement("strong");
    head.textContent = ({ ok: "계약 목록에 저장했습니다", review: "저장하지 않았습니다 — 검토가 필요합니다", error: "저장하지 못했습니다" })[outcome];
    const kind = ({ request: "접수", notice: "공고", contract: "계약" })[view?.entityType];
    const target = [kind, view?.entity, view?.fields?.title].filter(Boolean).join(" · ");
    text.append(head);
    for (const line of outcome === "ok" ? [target] : [view?.message, target]) {
      if (!line) continue;
      const sub = document.createElement("span"); sub.className = "sub"; sub.textContent = line; text.append(sub);
    }
    box.append(mark, text);
    root.append(style, box);
    const close = () => { clearTimeout(toastTimer); host.remove(); };
    box.onclick = close;
    document.documentElement.append(host);
    toastTimer = setTimeout(close, outcome === "ok" ? 4000 : 8000);
  }
  chrome.runtime.onMessage.addListener((message, sender, respond) => {
    // 중계기(서비스 워커)만 보낸다. 다른 탭의 콘텐츠 스크립트는 탭으로 메시지를 보낼 수 없다.
    if (retired || message?.type !== "pclm-command-result" || sender.id !== chrome.runtime.id || sender.tab) return;
    toast(message.view);
    // 패널이 없으면 중계기가 배지를 붙이고, 검토가 필요하면 팝업을 연다.
    if (!mounted) { respond(false); return; }
    mounted.show(message.view); respond(true);
  });
  await apply((await chrome.storage.local.get("panelMode")).panelMode);
})();
