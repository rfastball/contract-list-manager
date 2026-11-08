import { afterEach, describe, expect, it, vi } from "vitest";

/**
 * 창의 통로 — 앱 명령 `bridge` 에 `{id, method, args}` 글 하나를 실어 보내고, 다리의 답 글을 풀어 돌려준다.
 * 알림은 `pclm:notice` 이벤트의 `{notice}` 글이다. 모듈이 읽을 때 전역을 보므로 사례마다 다시 읽는다.
 */
type Listener = (event: { payload: unknown }) => void;

async function load(reply: (request: string) => unknown) {
  const sent: { command: string; args: Record<string, unknown> }[] = [];
  const listeners = new Map<string, Listener>();
  window.__TAURI__ = {
    core: {
      invoke: async (command, args) => {
        sent.push({ command, args: args ?? {} });
        return reply(String(args?.request));
      },
    },
    event: {
      listen: async (name, handler) => {
        listeners.set(name, handler);
        return () => listeners.delete(name);
      },
    },
  };
  vi.resetModules();
  const bridge = await import("./bridge");
  return { bridge, sent, listeners };
}

afterEach(() => {
  delete window.__TAURI__;
  vi.resetModules();
});

describe("창의 통로", () => {
  it("요청 하나를 글로 실어 보내고 답의 result 를 돌려준다", async () => {
    const { bridge, sent } = await load((request) => {
      const { id } = JSON.parse(request) as { id: number };
      return JSON.stringify({ id, ok: true, result: { 건수: 3 }, error: null });
    });

    expect(bridge.inApp).toBe(true);
    await expect(bridge.call("status", "계약", null)).resolves.toEqual({ 건수: 3 });
    expect(sent).toHaveLength(1);
    expect(sent[0].command).toBe("bridge");
    expect(JSON.parse(String(sent[0].args.request))).toEqual({ id: 1, method: "status", args: ["계약", null] });
  });

  it("다리가 거절한 글을 그대로 오류로 낸다", async () => {
    const { bridge } = await load(() => JSON.stringify({ id: 1, ok: false, result: null, error: "모르는 요청: 없음" }));
    await expect(bridge.call("없음")).rejects.toThrow("모르는 요청: 없음");
  });

  it("명령 자체가 실패하면 그 글로 거절한다", async () => {
    const { bridge } = await load(() => { throw "다리가 놓였습니다."; });
    await expect(bridge.call("status")).rejects.toThrow("다리가 놓였습니다.");
  });

  it("알림 이벤트를 이름으로 넘긴다", async () => {
    const { bridge, listeners } = await load(() => "{}");
    const closing = vi.fn();
    const off = bridge.onNotice("closeRequested", closing);

    listeners.get("pclm:notice")!({ payload: JSON.stringify({ notice: "closeRequested" }) });
    listeners.get("pclm:notice")!({ payload: "깨진 글" });
    expect(closing).toHaveBeenCalledTimes(1);

    off();
    listeners.get("pclm:notice")!({ payload: JSON.stringify({ notice: "closeRequested" }) });
    expect(closing).toHaveBeenCalledTimes(1);
  });
});
