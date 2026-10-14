import { describe, expect, it } from "vitest";
import { 은는, 을를, 받침 } from "./josa";

describe("받침에 맞는 조사", () => {
  it("연결 낱말·동사 뒤에 맞는 조사를 붙인다", () => {
    expect(은는("연결공고")).toBe("연결공고는");
    expect(은는("연결계약")).toBe("연결계약은");
    expect(은는("잇기")).toBe("잇기는");
    expect(은는("정렬")).toBe("정렬은");
    expect(을를("‘품명 모음’")).toBe("‘품명 모음’을");
  });
  it("숫자는 읽는 소리로 친다", () => {
    expect(받침("3")).toBe("ㅇ");
    expect(받침("1")).toBe("ㄹ");
    expect(받침("2")).toBe("");
  });
});
