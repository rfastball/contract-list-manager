import { useEffect, useRef, type ReactNode } from "react";

/** 네 확인창이 같은 기본 동작을 쓴다. 초점 제한은 브라우저의 모달이 맡는다. */
export function Modal({ label, onClose, children }: { label: string; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current!;
    const previous = document.activeElement as HTMLElement | null;
    dialog.showModal();
    dialog.querySelector<HTMLElement>(".sheet")?.focus();
    return () => { dialog.close(); if (previous?.isConnected) previous.focus(); };
  }, []);
  return <dialog ref={ref} className="scrim" role="dialog" aria-modal="true" aria-label={label}
    onCancel={(event) => { event.preventDefault(); onClose(); }}
    onClick={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <div className="sheet" tabIndex={-1}>{children}</div>
  </dialog>;
}
