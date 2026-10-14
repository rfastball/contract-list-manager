(async () => {
  const byId = id => document.getElementById(id);
  const saved = await chrome.storage.local.get("erpDevelopment");
  const done = text => { byId("saved").textContent = text; };
  // 자료 자리는 여기서 보거나 옮기지 않는다 — 계약 목록 앱에서만 다룬다.
  byId("development").checked = Boolean(saved.erpDevelopment);
  byId("development").onchange = async () => {
    await chrome.storage.local.set({ erpDevelopment: byId("development").checked });
    done(byId("development").checked ? "개발 DB에 저장합니다." : "업무 DB에 저장합니다.");
  };
})();
