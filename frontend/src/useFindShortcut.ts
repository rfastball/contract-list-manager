import { useEffect, type RefObject } from "react";

/**
 * Ctrl+F 로 찾기 칸에 초점을 준다.
 *
 * <p><b>창 전체에서 듣는다.</b> 예전에는 표가 초점을 쥐고 있을 때만 들었는데, 창을 막 연
 * 사람은 아무 데도 초점이 없어 눌러도 아무 일이 없었다 — 칸에 적힌 안내가 사실이 아니게 되는
 * 자리다. 안내를 뗄 수도 있었지만, 표를 다루는 사람이 가장 먼저 찾는 단축키라 배선을 올렸다.</p>
 *
 * <p><b>대화창이 떠 있으면 물러난다.</b> 그 뒤에 가려진 칸으로 초점이 가면, 눌러도 아무 일이
 * 없어 보이는 채로 글자가 보이지 않는 곳에 쌓인다.</p>
 */
export function useFindShortcut(box: RefObject<HTMLInputElement | null>) {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "f" || e.altKey || !(e.ctrlKey || e.metaKey)) return;
      if (document.querySelector('[role="dialog"]') !== null) return;

      e.preventDefault();
      box.current?.focus();
      box.current?.select();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [box]);
}
