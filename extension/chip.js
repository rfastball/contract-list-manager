// 수집 화면에 띄운 「문서 만들기」 단추(ADR-046). 중계기가 이 탭을 수집 화면으로 읽으면 심고(background.js placeChip), 아니게
// 되면 뗀다. 페이지 CSS 가 닿지 않게 닫힌 Shadow DOM 에 그리고, 바깥 상자는 인라인 !important 로 자리를 지킨다. 색은
// tokens.css 의 주 단추 값을 옮겨 적었다 — 페이지에 tokens.css 를 넣지 않는다.
(() => {
  if (document.getElementById("pclm-chip")) return;
  const host = document.createElement("div");
  host.id = "pclm-chip";
  host.style.cssText = "all:initial!important;position:fixed!important;right:16px!important;bottom:16px!important;z-index:2147483647!important";
  const root = host.attachShadow({ mode: "closed" });
  root.innerHTML = `<style>
    button{all:initial;box-sizing:border-box;display:inline-flex;align-items:center;min-height:40px;padding:8px 16px;border-radius:6px;
      background:#2f5fbf;color:#ffffff;font:700 15px/1.2 "Malgun Gothic","맑은 고딕",system-ui,sans-serif;letter-spacing:-.01em;
      box-shadow:0 4px 16px rgba(0,0,0,.2);cursor:pointer}
    button:hover{background:#274fa0}
    button:active{transform:translateY(1px)}
    button:focus-visible{outline:2px solid #2f5fbf;outline-offset:2px}
    @media print{button{display:none}}
    @media (forced-colors:active){button{border:1px solid ButtonText}}
  </style><button type="button">문서 만들기</button>`;
  root.querySelector("button").addEventListener("click", (event) => {
    // 사람이 누른 것만 — sidePanel.open 은 사람 동작 안에서만 열린다.
    if (event.isTrusted) chrome.runtime.sendMessage({ type: "pclm-open-panel" }).catch(() => {});
  });
  (document.body || document.documentElement).append(host);
})();
