/**
 * 화면과 앱 사이의 유일한 통로.
 *
 * WebView2 는 메시지를 주고받을 뿐 호출·응답 짝을 맺어주지 않아서, 여기서 번호를 붙여
 * 짝을 맞추고 Promise 로 감싼다. hwpx-filler 의 bridge.js 와 같은 얼개다 —
 * 저쪽은 pywebview, 이쪽은 WebView2 라 붙이는 자리만 다르다.
 */

type Pending = { resolve: (value: unknown) => void; reject: (reason: Error) => void };

const pending = new Map<number, Pending>();
let nextId = 1;

declare global {
  interface Window {
    chrome?: {
      webview?: {
        postMessage(message: unknown): void;
        addEventListener(type: string, handler: (event: { data: unknown }) => void): void;
      };
    };
  }
}

const webview = window.chrome?.webview;

webview?.addEventListener("message", (event) => {
  const reply = event.data as { id: number; ok: boolean; result?: unknown; error?: string };
  const waiting = pending.get(reply.id);
  if (!waiting) return;

  pending.delete(reply.id);
  if (reply.ok) waiting.resolve(reply.result);
  else waiting.reject(new Error(reply.error ?? "알 수 없는 오류"));
});

/**
 * 앱에 일을 시키고 결과를 기다린다.
 *
 * 개발 중에는 창이 없으므로 가짜 다리로 넘긴다 — `import.meta.env.DEV` 가 상수라
 * **배포 묶음에서는 이 가지와 mock 이 통째로 털린다**. 배포본을 브라우저로 열면 거절한다.
 */
export function call<T>(method: string, ...args: unknown[]): Promise<T> {
  if (!webview) {
    if (import.meta.env.DEV) {
      return import("./mock").then((m) => m.invoke(method, args)) as Promise<T>;
    }

    return Promise.reject(new Error("앱 안에서만 쓸 수 있습니다."));
  }

  const id = nextId++;
  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
    webview.postMessage({ id, method, args });
  });
}

export const isHosted = webview !== undefined || import.meta.env.DEV;

