import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// 프론트는 파일에서 바로 열리므로 상대 경로로 짓는다 — WebView2 가 가상 호스트로 폴더를 매핑한다.
export default defineConfig({
  root: fileURLToPath(new URL("./frontend/", import.meta.url)),
  base: "./",
  plugins: [react()],
  build: {
    outDir: fileURLToPath(new URL("./src/Pclm.App/wwwroot/", import.meta.url)),
    emptyOutDir: true,
    assetsInlineLimit: 0,
  },
  // 화면 시험. 표의 편집은 값이 조용히 어긋날 수 있는 자리라 시험을 붙여 둔다.
  test: {
    environment: "jsdom",
    // root 가 frontend/ 라 여기 경로도 그 아래를 가리킨다.
    include: ["src/**/*.test.tsx"],
  },
});
