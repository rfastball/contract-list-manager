/**
 * 화면과 앱 사이의 유일한 통로.
 *
 * 창(Tauri)의 앱 명령 `bridge` 하나로 `{id, method, args}` 글을 보내고 다리의 답 글
 * (`{id, ok, result, error}` — C# Bridge 의 ReplyJson 과 같은 바이트)을 받는다. 앱이 먼저 건네는 알림
 * (`{notice}`)은 `pclm:notice` 이벤트로 온다. 전역(`withGlobalTauri`)을 쓰므로 npm 의존이 없다 —
 * 배포 exe 에 실리는 고지가 늘지 않는다.
 */

declare global {
  interface Window {
    __TAURI__?: {
      core?: { invoke(command: string, args?: Record<string, unknown>): Promise<unknown> };
      event?: {
        listen(event: string, handler: (event: { payload: unknown }) => void): Promise<() => void>;
      };
    };
  }
}

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;

/** 진짜 창 안인가. 개발 서버를 브라우저로 열면 아니다(가짜 다리로 넘긴다). */
export const inApp = invoke !== undefined;

let nextId = 1;

/** 앱이 먼저 건네는 알림의 받는 이. 답과 달리 번호가 없어 이름으로 찾는다. */
const listeners = new Map<string, Set<() => void>>();

/**
 * 앱이 묻지 않은 것을 먼저 알린다 — 지금은 창을 닫으려 한다는 것 하나다(<code>closeRequested</code>).
 * 받는 이가 없으면 그냥 지나간다. 풀 함수를 돌려준다.
 */
export function onNotice(name: string, handler: () => void): () => void {
  const set = listeners.get(name) ?? new Set();
  set.add(handler);
  listeners.set(name, set);
  return () => { set.delete(handler); };
}

/** 알림 하나를 받는 이들에게 넘긴다. 진짜 다리의 알림이 이리로 오고, 시험은 이것을 직접 부른다. */
export function receiveNotice(name: string) {
  for (const handler of [...(listeners.get(name) ?? [])]) handler();
}

void tauri?.event?.listen("pclm:notice", (event) => {
  try {
    const data = JSON.parse(String(event.payload)) as { notice?: unknown };
    if (typeof data.notice === "string") receiveNotice(data.notice);
  } catch {
    // 알림 꼴이 아니면 지나간다 — 답과 달리 기다리는 쪽이 없다.
  }
});

/**
 * 앱에 일을 시키고 결과를 기다린다.
 *
 * 개발 중에는 창이 없으므로 가짜 다리로 넘긴다 — `import.meta.env.DEV` 가 상수라
 * **배포 묶음에서는 이 가지와 mock 이 통째로 털린다**. 배포본을 브라우저로 열면 거절한다.
 */
export function call<T>(method: string, ...args: unknown[]): Promise<T> {
  if (!invoke) {
    if (import.meta.env.DEV) {
      return import("./mock").then((m) => m.invoke(method, args)) as Promise<T>;
    }

    return Promise.reject(new Error("앱 안에서만 쓸 수 있습니다."));
  }

  const id = nextId++;
  return invoke("bridge", { request: JSON.stringify({ id, method, args }) }).then(
    (text) => {
      const reply = JSON.parse(String(text)) as { id: number; ok: boolean; result?: unknown; error?: string | null };
      if (reply.ok) return reply.result as T;
      throw new Error(reply.error ?? "알 수 없는 오류");
    },
    (error: unknown) => {
      throw error instanceof Error ? error : new Error(String(error));
    },
  );
}

export const isHosted = inApp || import.meta.env.DEV;
