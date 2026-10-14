/** 열 관리의 그림. 한 굵기(1.6)의 선으로만 그린다. */
const paths = {
  back: "M15 5 8 12l7 7",
  plus: "M12 5v14M5 12h14",
  alert: "M12 4 3 20h18L12 4Zm0 6v4m0 2.6v.8",
  up: "M12 19V5m-6 6 6-6 6 6",
  down: "M12 5v14m-6-6 6 6 6-6",
  remove: "M6 6l12 12M18 6 6 18",
  trash: "M5 7h14M10 11v6m4-6v6M7 7l1 12h8l1-12M10 7V4h4v3",
  more: "M4 12a1.4 1.4 0 1 0 2.8 0a1.4 1.4 0 1 0-2.8 0Zm6.6 0a1.4 1.4 0 1 0 2.8 0a1.4 1.4 0 1 0-2.8 0Zm6.6 0a1.4 1.4 0 1 0 2.8 0a1.4 1.4 0 1 0-2.8 0Z",
  check: "m5 12 5 5 9-10",
  arrow: "M5 12h14m-6-6 6 6-6 6",
  chevron: "m9 5 7 7-7 7",
  grip: "M8.2 6a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0Zm6 0a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0ZM8.2 12a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0Zm6 0a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0ZM8.2 18a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0Zm6 0a.8.8 0 1 0 1.6 0a.8.8 0 1 0-1.6 0Z",
  search: "M10.5 17a6.5 6.5 0 1 0 0-13a6.5 6.5 0 0 0 0 13Zm4.6-1.9L20 20",
};
export type IconName = keyof typeof paths;

export function Icon({ name, size = 16 }: { name: IconName; size?: number }) {
  return <svg className="cm-icon" aria-hidden="true" viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor"
    strokeWidth={1.6} strokeLinecap="round" strokeLinejoin="round"><path d={paths[name]} /></svg>;
}
