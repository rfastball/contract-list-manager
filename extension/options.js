(async () => {
  const byId = id => document.getElementById(id);
  const saved = await chrome.storage.local.get(["panelMode", "erpDevelopment"]);
  const done = text => { byId("saved").textContent = text; };
  for (const radio of document.querySelectorAll('input[name="panelMode"]')) {
    radio.checked = radio.value === (saved.panelMode === "button" ? "button" : "always");
    radio.onchange = async () => {
      await chrome.storage.local.set({ panelMode: radio.value });
      done("표시 방식을 저장했습니다.");
    };
  }
  // 저장 위치: 자리는 앱의 호스트가 말한다. 옮기기는 대상 경로를 먼저 보이고 한 번 더 눌러야 예약된다.
  // 예약만 한다 — 실제 이동은 앱 다음 실행 맨 앞이고, 그때까지는 지금 자리에 저장된다.
  const ask = message => chrome.runtime.sendMessage({ type: "pclm-location", ...message })
    .catch(() => ({ ok: false, connection: true }));
  let place, armed = "";
  const note = text => { byId("locationTarget").textContent = text; };
  const disarm = () => { armed = ""; byId("locationMove").textContent = "이 폴더로 옮기기 예약"; note(""); };
  const showLocation = async () => {
    disarm();
    const reply = await ask({ action: "location" });
    place = reply?.ok ? reply.result : undefined;
    byId("locationBody").hidden = !place;
    byId("locationStatus").hidden = Boolean(place);
    if (!place) {
      byId("locationStatus").textContent = reply?.connection
        ? "계약 목록 앱에 연결하지 못했습니다. 앱의 설정에서 Edge · Chrome 확장 연결을 준비한 뒤 이 창을 다시 여세요."
        : reply?.message || "저장 위치를 확인하지 못했습니다.";
      return;
    }
    byId("locationPath").value = place.path;
    byId("locationFolderName").textContent = place.folderName;
    byId("locationPending").hidden = !place.pending;
    byId("locationPending").textContent = place.pending
      ? `다음 앱 실행 때 ${place.pending.from} → ${place.pending.to} 로 옮깁니다.` : "";
  };
  byId("locationFolder").oninput = disarm;
  byId("locationMove").onclick = async () => {
    const folder = byId("locationFolder").value.trim();
    if (!folder) { disarm(); note("옮길 폴더를 입력하세요."); return; }
    if (armed !== folder) {
      armed = folder;
      byId("locationMove").textContent = "예약 확인";
      note(`${folder.replace(/[\\/]+$/, "")}\\${place.folderName} 로 옮깁니다. 맞으면 한 번 더 누르세요.`);
      return;
    }
    byId("locationMove").disabled = true;
    const reply = await ask({ action: "moveLocation", datasetId: place.datasetId, folder });
    byId("locationMove").disabled = false;
    if (!reply?.ok) {
      disarm();
      note(reply?.connection ? "계약 목록 앱에 연결하지 못했습니다." : reply?.message || "예약하지 못했습니다.");
      return;
    }
    await showLocation();
    byId("locationFolder").value = "";
    note(`${reply.result.folder} 로 옮기도록 예약했습니다. 계약 목록 앱을 다음에 실행할 때 옮깁니다.`);
  };

  byId("development").checked = Boolean(saved.erpDevelopment);
  byId("development").onchange = async () => {
    await chrome.storage.local.set({ erpDevelopment: byId("development").checked });
    done(byId("development").checked ? "개발 DB에 저장합니다." : "업무 DB에 저장합니다.");
    await showLocation();
  };
  // 단축키는 브라우저의 확장 단축키 화면에서만 바꿀 수 있다. 돌아오면 다시 읽는다.
  const shortcut = async () => {
    const current = (await chrome.commands.getAll()).find(c => c.name === "save-current")?.shortcut;
    byId("shortcut").textContent = current ? `현재 단축키: ${current}` : "단축키 없음 — 아래 버튼으로 지정하세요.";
  };
  byId("shortcuts").onclick = () => chrome.tabs.create({
    url: `${navigator.userAgent.includes("Edg/") ? "edge" : "chrome"}://extensions/shortcuts` });
  document.addEventListener("visibilitychange", () => { if (!document.hidden) shortcut(); });
  await Promise.all([shortcut(), showLocation()]);
})();
