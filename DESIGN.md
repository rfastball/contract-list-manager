---
name: 계약 목록
description: Windows 데스크톱의 조달 자료 검토·편집 화면
colors:
  a-primary: "#2f5fbf"
  a-primary-h: "#274fa0"
  a-on-accent: "#ffffff"
  a-card: "#ffffff"
  a-window: "#f6f7f9"
  a-ink: "#1c2126"
  a-border: "#e2e4e8"
  a-sel: "#dfe6fb"
  a-danger: "#c0392b"
  a-warn: "#a05a00"
  a-ok: "#1e8449"
  n-ink-soft: "#5c626b"
  n-hover: "#f0f2f5"
  n-header-bg: "#eef1f4"
  n-border-strong: "#cbd0d6"
  n-border-control: "#767b83"
  fb-fill-bg: "#e5f2ea"
  fb-blank-bg: "#f6ecdb"
typography:
  title:
    fontFamily: '"Pretendard GOV Variable", "Malgun Gothic", system-ui, sans-serif'
    fontSize: "1.5rem"
    fontWeight: 700
    lineHeight: 1.55
    letterSpacing: "-.025em"
  body:
    fontFamily: '"Pretendard GOV Variable", "Malgun Gothic", system-ui, sans-serif'
    fontSize: "0.875rem"
    lineHeight: 1.55
  label:
    fontFamily: '"Pretendard GOV Variable", "Malgun Gothic", system-ui, sans-serif'
    fontSize: "0.75rem"
    lineHeight: 1.55
rounded:
  control: "6px"
  surface: "9px"
  overlay: "12px"
  pill: "999px"
spacing:
  sp-4: "4px"
  sp-8: "8px"
  sp-12: "12px"
  sp-16: "16px"
  sp-24: "24px"
components:
  button-primary:
    backgroundColor: "{colors.a-primary}"
    textColor: "{colors.a-on-accent}"
    rounded: "{rounded.control}"
    padding: "8px 12px"
  button-primary-hover:
    backgroundColor: "{colors.a-primary-h}"
  button-secondary:
    backgroundColor: "{colors.a-card}"
    textColor: "{colors.a-ink}"
    rounded: "{rounded.control}"
    padding: "8px 12px"
  input-search:
    backgroundColor: "{colors.a-card}"
    textColor: "{colors.a-ink}"
    rounded: "{rounded.control}"
    padding: "8px 12px"
  card:
    backgroundColor: "{colors.a-card}"
    rounded: "{rounded.surface}"
    padding: "12px 16px"
---

# Design System: 계약 목록

## Overview

Windows WPF/WebView2 안에서 쓰는 자료 중심 화면이다. 기존 Pretendard GOV와 공유 토큰을 사용하며, 탐색·작업·자료 영역을 테두리와 여백으로 구분한다. 이 문서는 구현에서 추출한 사실을 기록하며 별도의 브랜드 비유나 사용자 취향을 정의하지 않는다.

스타일의 권위는 [tokens.css](frontend/css/tokens.css)와 [app.css](frontend/css/app.css)에 있다. 위 frontmatter는 반복되는 밝은 테마 토큰의 발췌다. 전체 척도와 어두운 테마 값은 원본 CSS를 따르며, 생성된 tokens.css는 직접 수정하지 않는다. 화면 작업 방향은 [ux-ui.md](docs/ux-ui.md)에 있다.

## Colors

파란색은 기본 작업·선택·키보드 초점에, 중성색은 바탕·구획·보조 문구에 쓰인다. 흰 자료 면과 옅은 창 바탕이 작업 영역을 나눈다. 오류·삭제는 danger, 주의는 warn, 사람이 채우는 열은 fill 바탕과 ok 글자로 구별한다. 문서에서 읽은 값을 정정한 칸은 blank 바탕과 모서리 표시를 함께 쓴다.

시스템 색상 설정을 따르며 명시적 light/dark 테마도 지원한다. 새 스타일은 원본 CSS 변수를 참조하여 같은 상태 의미를 유지한다.

## Typography

제목·본문·조작부 모두 같은 글꼴 계열을 쓴다. 표 머리글·건수·단계 제목은 주로 굵기 600, 본문과 보조 문구는 크기와 색으로 구분한다. 금액·건수·번호에는 tabular-nums를 적용한다. 글꼴 확대 설정은 루트 크기 125%와 150%를 지원한다.

## Layout

기본 골격은 11rem 탐색 영역과 남은 폭을 채우는 작업 영역이다. 작업 영역은 제목과 입출력, 자료 수와 보기 선택, 검색, 본문, 하단 안내 순서다. 표와 구조의 본문은 자체 영역에서 스크롤하고 표 머리글은 고정된다.

600–1100px에서는 제목과 입출력을 나란히 두고 검색 영역은 남은 폭에 맞춘다. 760px 이하에서는 탐색을 위로 옮기고 조달 단계를 세로로 잇는다. 표는 가로 스크롤을 유지한다. 이는 데스크톱 창 축소 대응이며 별도 모바일 제품을 뜻하지 않는다.

## Elevation & Depth

일반 자료 면은 그림자 없이 테두리와 바탕 차이로 구분한다. 설정 창에는 큰 그림자, 토스트에는 작은 그림자를 쓴다. 그림자·스크림 색은 테마 변수를 따른다. 현재 app.css에는 전환·애니메이션 선언이 없으며 reduced-motion에서는 두 효과를 모두 끈다.

## Shapes

조작부, 자료 면, 오버레이는 각각 control, surface, overlay 모서리를 쓴다. 둥근 pill은 건수·단계·차수 이름표에 사용한다. 보기 선택 단추는 pill 대신 control 모서리다. 빈 단계는 점선 테두리와 ‘아직 없음’ 문구로 구별한다.

## Components

- **단추:** 가져오기는 채운 primary, 내보내기는 테두리가 있는 secondary, 설정은 quiet, 삭제는 danger다. 비활성 단추는 불투명도 0.5와 기본 커서를 쓴다.
- **탐색과 보기:** 자료 종류의 선택 상태는 바탕·글자색·굵기를 함께 바꾼다. 보기 선택은 작은 단추와 aria-pressed를 쓴다.
- **입력:** 검색과 설정 입력은 control 테두리와 카드 바탕을 사용한다. 초점은 primary 외곽선으로 보인다.
- **자료 면:** 일반 카드는 얇은 테두리를 사용한다. 구조 보기의 한 행에는 접수·공고·계약과 방향 화살표가 놓인다.
- **표와 구조 커서:** 현재 칸에 primary 외곽선을 그리며, 본문이 초점을 잃으면 경계색으로 낮춘다. 표 정정 칸은 원래 값 안내와 모서리 표시를 유지한다.
- **빈 상태와 오류:** 빈 자료에는 설정 경로, 검색 결과 없음에는 조건을 걷는 동작을 둔다. 읽는 중 문구와 오류 토스트를 구분한다.
- **확인·설정 창:** 네이티브 dialog로 배경 조작을 막고, 창 내부에 초기 초점을 둔다. Esc로 닫으면 이전 조작부로 초점을 돌린다. 토스트는 열린 창 안에 표시한다.

## Do's and Don'ts

- Do 공유 CSS 변수와 Pretendard GOV를 사용한다.
- Do 선택·초점·정정 상태를 색 외의 굵기·외곽선·표시로도 구분한다.
- Do 긴 자료 제목과 좁은 창에서 줄바꿈·영역 스크롤을 유지한다.
- Don't 빈 단계의 문구 대비를 불투명도로 낮추지 않는다.
- Don't 테마에 따라 달라지는 값을 새 컴포넌트에 고정 색으로 복제하지 않는다.
