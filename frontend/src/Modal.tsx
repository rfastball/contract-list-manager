import { useEffect, useRef, type ReactNode } from "react";

/** 네 확인창이 같은 기본 동작을 쓴다. 초점 제한은 브라우저의 모달이 맡는다. */
export function Modal({ label, onClose, children, className, fallbackFocus }: {
  label: string; onClose: () => void; children: ReactNode;
  /** 판의 모양을 바꾸는 이름. 설정처럼 목차와 본문을 나눠 세우는 창만 쓴다. */
  className?: string;
  /**
   * 닫힐 때 연 자리가 이미 사라졌으면 초점을 줄 곳. 일을 마치면 연 단추가 함께 걷히는 창(일괄 변경·자료 삭제)이 준다 —
   * 주지 않으면 초점이 body 로 떨어져 키보드로는 어디에도 돌아올 수 없다.
   */
  fallbackFocus?: () => HTMLElement | null | undefined;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const fallback = useRef(fallbackFocus);
  fallback.current = fallbackFocus;
  useEffect(() => {
    const dialog = ref.current!;
    const previous = document.activeElement as HTMLElement | null;
    dialog.showModal();
    dialog.querySelector<HTMLElement>(".sheet")?.focus();
    return () => {
      dialog.close();
      if (previous?.isConnected && previous !== document.body) previous.focus();
      else fallback.current?.()?.focus();
    };
  }, []);
  return <dialog ref={ref} className="scrim" role="dialog" aria-modal="true" aria-label={label}
    onCancel={(event) => { event.preventDefault(); onClose(); }}
    onClick={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <div className={className ? `sheet ${className}` : "sheet"} tabIndex={-1}>{children}</div>
  </dialog>;
}
