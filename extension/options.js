(async () => {
  const byId = id => document.getElementById(id);
  const saved = await chrome.storage.local.get(["panelMode", "erpDevelopment"]);
  const done = text => { byId("saved").textContent = text; };
  const radios = [...document.querySelectorAll('input[name="panelMode"]')];
  const check = mode => radios.forEach(radio => { radio.checked = radio.value === (mode === "button" ? "button" : "always"); });
  check(saved.panelMode);
  for (const radio of radios) radio.onchange = async () => {
    await chrome.storage.local.set({ panelMode: radio.value });
    done("표시 방식을 저장했습니다.");
  };
  // 표시 방식은 수집기의 「숨기기」 와 툴바 팝업에서도 바뀐다. 열린 설정 창이 옛 값을 보이지 않게 따라간다.
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local" && changes.panelMode) check(changes.panelMode.newValue);
  });
  // 자료 자리는 여기서 보거나 옮기지 않는다 — 계약 목록 앱에서만 다룬다(background.js).

  byId("development").checked = Boolean(saved.erpDevelopment);
  byId("development").onchange = async () => {
    await chrome.storage.local.set({ erpDevelopment: byId("development").checked });
    done(byId("development").checked ? "개발 DB에 저장합니다." : "업무 DB에 저장합니다.");
  };
  // 단축키는 브라우저의 확장 단축키 화면에서만 바꿀 수 있다. 돌아오면 다시 읽는다.
  const shortcut = async () => {
    const current = (await chrome.commands.getAll()).find(c => c.name === "save-current")?.shortcut;
    byId("shortcut").textContent = current ? `현재 단축키: ${current}` : "단축키 없음 — 아래 버튼으로 지정하세요.";
  };
  byId("shortcuts").onclick = () => chrome.tabs.create({
    url: `${navigator.userAgent.includes("Edg/") ? "edge" : "chrome"}://extensions/shortcuts` });
  document.addEventListener("visibilitychange", () => { if (!document.hidden) shortcut(); });
  await shortcut();
})();
