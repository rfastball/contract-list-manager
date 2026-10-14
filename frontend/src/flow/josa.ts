/**
 * 받침에 맞는 조사. 열 이름·동사·연결 낱말처럼 문구 안에 끼는 말 뒤에 붙인다 — 「연결공고은」 같은 틀린 조사를 막는다.
 * 숫자는 읽는 소리(0 영·3 삼·6 육 은 받침, 1 일·7 칠·8 팔 은 ㄹ)로, 그 밖의 글자는 받침 없음으로 친다.
 */
export function 받침(word: string): "" | "ㄹ" | "ㅇ" {
  const last = word.trim().replace(/[’'」』)\]]+$/, "").slice(-1);
  if (/[0-9]/.test(last)) return "036".includes(last) ? "ㅇ" : "178".includes(last) ? "ㄹ" : "";
  if (/[가-힣]/.test(last)) {
    const code = (last.charCodeAt(0) - 0xac00) % 28;
    return code === 0 ? "" : code === 8 ? "ㄹ" : "ㅇ";
  }
  return "";
}
export const 은는 = (w: string) => w + (받침(w) ? "은" : "는");
export const 을를 = (w: string) => w + (받침(w) ? "을" : "를");
export const 이가 = (w: string) => w + (받침(w) ? "이" : "가");
export const 으로 = (w: string) => w + (받침(w) && 받침(w) !== "ㄹ" ? "으로" : "로");
