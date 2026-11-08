# 동등성 계기 (Pclm.Parity · pclm-parity)

C# 구현과 Rust 이식(#8·#9)이 같은 `.pclm` 을 같게 보는지, 같은 일을 하면 같은 파일을 남기는지 잰다.
두 도구가 있다 — C# 은 `tests/Pclm.Parity`, Rust 는 `crates/pclm-parity`. 둘은 **재는 자**라서, 같은 파일을
덤프하면 **바이트까지** 같아야 한다. 갈리면 자를 고친다 — `--mask` 로 덮지 않는다.

이 문서가 덤프 꼴의 정본이다. 두 구현(`Dump.cs`, `dump.rs`)은 이것을 따른다.

C# 쪽(`tests/Pclm.Parity` · `src/Pclm.*`)은 ⑨ 에서 걷었다 — 지금은 그때 C# 이 낸 박제(`tests/parity-golden`)와 견준다.
아래의 C# 파일 이름은 그때의 것이고, 걷은 커밋을 되돌리면 다시 선다.

## 덤프 꼴 1판

UTF-8(BOM 없음), 줄 끝은 LF, 마지막 줄 뒤에도 LF.

```
pclm-parity-dump 1
user_version <n>
application_id <n>
object	<type>	<name>	<tbl_name>	<enc(sql)>        … sqlite_master 의 모든 행
table	<name>	<행 수>                                 … 표마다
columns	<열>	<열>…
row	<칸>	<칸>…
view	<name>	<행 수>	<차례 해시>                       … 뷰마다(--skip-views 가 아니면)
columns	…
row	…
view	<name>	error	<enc(SQLite 의 말)>                  … SELECT 가 실패한 뷰
```

- `object` 줄: 이름이 `sqlite_stat` 으로 시작하는 것을 뺀 `sqlite_master` 의 모든 행(`--skip-views` 면 `type='view'` 도 뺀다 —
  뷰를 아직 짓지 않는 쪽과 표만 견줄 때). `sql` 이 NULL 이면 `N`.
  **줄 전체**를 UTF-8 바이트 차례로 늘어놓는다.
- 표: `type='table'` 인 것(`sqlite_stat*` 제외, `sqlite_sequence` 는 든다)을 **이름의 UTF-8 바이트 차례**로.
  열 이름은 `SELECT * FROM "<name>"` 의 결과 열 이름. `row` 줄은 줄 전체의 UTF-8 바이트 차례로.
- 뷰: 이름 차례로. 행 수 뒤의 **차례 해시**는 SQLite 가 돌려준 차례 그대로 `row` 줄마다 UTF-8 바이트와 LF 를
  넣은 FNV-1a 64비트(오프셋 `0xcbf29ce484222325`, 소수 `0x100000001b3`), 16자리 소문자. `row` 줄 자체는
  표처럼 정렬해 적는다 — 내용은 정렬로, 차례는 해시로 본다. 도중에 실패한 뷰는 읽은 행을 버리고 `error` 줄 하나다.
  말은 `sqlite3_errmsg` 그대로다(C# 은 Microsoft.Data.Sqlite 의 `SQLite Error n: '…'.` 껍질을 벗긴다).
- 칸: NULL `N` · 정수 `i<십진>` · 실수 `r<IEEE-754 비트 16자리 소문자>` · 글 `t<enc(글)>` · BLOB `b<소문자 16진>`.
  꼴은 **값이 실제로 담긴 저장 꼴**(`sqlite3_column_type`)로 가린다 — 선언형이 아니다. INTEGER 열의 글은 `t` 다.
- `enc()`: `\` → `\\`, 탭 → `\t`, LF → `\n`, CR → `\r`. 나머지는 그대로. 이름(표·뷰·열·`tbl_name`)에도 같은 규칙을
  쓴다 — 보통 이름에는 차이가 없다.
- `--mask <object>.<column>`(여럿 가능, `object` 는 `*` 도 된다. 첫 마침표에서 가른다): 그 열의 NULL 이 아닌 칸을
  `m` 으로 바꾼다. 정렬·해시보다 먼저 바꾼다. 시각·무작위 식별자처럼 **일부러** 갈리는 칸에만 쓴다.
- `--mask-hex <object>.<column>`: 칸 전체가 아니라 그 안의 **소문자 16진 32자리 덩이**(앞뒤가 16진 글자가 아닌 것 —
  `Guid.ToString("N")`)만 `m` 으로 바꾸고 나머지 글자는 견준다. 칸은 `t` 로 적는다. 식별자를 품은 JSON 칸(가공열의
  `user_column.options`·`v_가공열.가공식`)에 쓴다. 같은 열에 `--mask` 가 있으면 그것이 이긴다.
- 차례는 언제나 UTF-8 바이트로 견준다. C# 의 `string.CompareOrdinal` 은 UTF-16 단위라 보조 평면 글자와
  U+E000~U+FFFF 의 차례가 뒤집힌다.

파일은 `Mode=ReadOnly`(Rust 는 `SQLITE_OPEN_READ_ONLY`)로 열고 `PRAGMA query_only=ON` 을 건다. 덤프는 파일에
쓰지 않는다.

## 명령

| 명령 | C# | Rust |
|---|---|---|
| `info` — SQLite 판, `compile_option` 줄들, 아무 PRAGMA 없이 연 `:memory:` 의 `foreign_keys`·`recursive_triggers` | ○ | ○ |
| `dump <file> [--out <path>] [--mask o.c]... [--mask-hex o.c]... [--skip-views]` | ○ | ○ |
| `create <file> <role>` — `PclmFile.Create` | ○ | ○ |
| `migrate <file>` — `Database.Migrate` | ○ | ○ |
| `refresh <file>` — `Database.RefreshViews`(뷰만 지웠다 다시 짓는다) | ○ | ○ |
| `readcheck <file>` — E5: `PclmFile.Inspect` → `Database(Access.Read).Migrate` → 뷰마다 `count(*)`. 쓰지 않는다 | ○ | — |
| `snapshot <src> <dst>` — 읽기 전용 연결의 `VACUUM INTO`. 원본에 쓰지 않는다 | ○ | ○ |
| `synth <file> <19..31> [--odd]` — 지어낸 행을 심은 옛 판(아래) | ○ | 아직 없음 |
| `seed <file> [--erp <dir>]` — 지금 판의 비교용 자료(아래) | ○ | ○ |
| `fixture <file>` — 자 자체를 시험하는 작은 SQLite(저장 꼴·이스케이프·정렬·깨진 뷰) | ○ | — |
| `run <file> <script.json>` — 편집 대본(아래). 결과는 `<file>.out.json` | ○ | ○ |
| `submit <src> <dst>` — `pclm submit` 의 뜨기(`PclmFile.Snapshot`, 역할 `submission`, 덮어쓰기). 설정·이름 짓기는 뺀다 | ○ | ○ |
| `merge <out> <src\|folder>...` — 취합(아래). 보고 글은 `<out>.txt` | ○ | ○ |
| `sql <file> <script.sql>` — 취합 입력을 짓는 날 SQL(풀 없는 연결, 없으면 짓는다). Core 를 거치지 않는다 | ○ | — |
| `host <home> <transcript> <out> [origin]` — 확장 호스트(아래). Rust 쪽은 계기가 아니라 `pclm-erp-host` 다 | ○ | — |
| `host-home <seed> <home> <variant>` — 호스트 시험용 홈 | ○ | — |
| `host-transcripts <seed> <dir>` — 호스트 대본 넷 | ○ | — |
| `lock <home> [--hold]` — 창 잠금(`Home.TryLock`)을 잡아 본다. `free`·`busy`·`held` | ○ | ○ |
| `inspect <file>` — `PclmFile.Inspect` 의 종류·역할·판·신원(탭 사이). 드라이버가 홈을 지을 때 신원을 집는다 | ○ | — |
| `config-write <home> <workfile> <datasetId> [retired]` — `Home.WriteConfig`. 셋 다 아래 「홈」 | ○ | ○ |
| `resolve <home> [--legacy <config>]` — `Home.Resolve` | ○ | ○ |
| `switch <op> <home> <current> <args>...` — 작업자료 바꾸기 하나(`HomeSwitch`) | ○ | ○ |
| `bridge <home> <file> <script.json> [--view] [--files <dir>] [--clock now]` — 다리의 요청(아래). 답은 `<file>.bridge.txt`, 창에 건넨 것은 `<file>.prompts.txt` | ○ | ○ |
| `xlsx-dump <file.xlsx> <out.txt>` — 엑셀 한 권의 덤프(아래 「엑셀」) | ○ | ○ |
| `plan-fixtures <dir>` — 다리 대본이 고르는 계획 엑셀 넷을 ClosedXML 로 짓는다(아래 「엑셀」) | ○ | — |

"아직 없음" 은 종료 코드 2 다.

- `synth`: v19 기준선에 지어낸 행을 심고, 지금 코드의 `Database.Migrate` 가 밟는 길(단계, v29→30 의
  `SchemaRepairs.Upgrade30`, v26·v27 의 예시 보기, v25 부터 명시 참조로 연결 다시 짓기)을 그 판까지만 밟는다.
  뷰는 짓지 않는다 — 판올림이 뷰를 먼저 치우고 끝에 다시 짓으므로 입력에 없어도 결과는 같다. 판올림이 다루는
  자리(INTEGER 열의 글, 실수, 사용자 열 이름 겹침, 옛 채권자, 덮개)를 일부러 심었다. `--odd`(v21 부터)는 손으로 고친 파일에서만
  생기는 글을 더 심는다 — 깨진 가공식·같은 키가 두 번 든 가공식(v21), 객체가 아닌 맞춤 뷰 정의·같은 키가 두 번 든 정의(v25).
- `seed`: `PclmFile.Create` 로 빈 작업자료를 짓고 `tests/flow-cases.json` 의 시험 문서를 `FlowCaseTests` 와 같은
  길로 넣는다. `--erp` 폴더의 `*.json` 은 창의 「JSON 파일로 가져오기」 와 같은 저장 길(`Mirror.Prepare` →
  `Mirror.DefaultChoices` → `ErpCapture.Save`, 프로필은 `pointInfo` 의 메뉴 번호로)을 기본 선택으로 지난다.
  받지 않는 파일은 표준 오류에 까닭을 적고 건너뛴다. 끝에 개체별 수를 낸다. 수집 ID 는 파일 차례(1부터)로
  `00000000-0000-0000-0000-000000000001` 처럼 짓는다 — 두 계기가 같은 ID 를 써야 수집 기록이 견주어진다.
  Rust 계기(#10)는 시험 문서를 C# 의 `Store.Upsert…`(시험 픽스처 전용, ADR-028 — Rust Core 에는 옮기지 않았다)가 짓는
  줄을 SQL 로 바로 짓고(묶는 꼴은 Microsoft.Data.Sqlite 의 것), ERP JSON 은 Rust Core 의 같은 저장 길
  (`mirror::prepare` → `mirror::default_choices` → `ErpCapture::save`)로 들인다. 드라이버의 `seed-rs`·`seed+erp-rs`
  사례가 두 seed 파일을 C# 덤프로 견주고(`.out` 은 표준 출력의 수와 건너뛴 까닭), 판마다 갈리는 칸 — 고친 시각
  (`*.updated_at`·`erp_capture.captured_at`·`v_ERP원천.수집시각`), 그 시각을 담은 baseToken 에서 오는
  `erp_capture.request_hash`, 영수증(`result_json`) 안의 datasetId — 만 가린다.

## 편집 대본 (`run`)

`run <file> <script.json>` 은 이미 지금 판인 파일 하나에 `Store` 를 열고(C# `new Store(new Database(file))`, Rust
`Store::new(Database::new(file, Access::Write), &SystemEnv)`) 대본의 연산을 차례로 시킨다. 판올림은 하지 않는다.

**대본**은 UTF-8 JSON 배열이고, 원소 하나가 연산 하나다 — `{"op": "<Store 의 C# 메서드 이름>", 인자...}`. 인자 이름은
C# 매개변수 이름 그대로다:

| 연산 | 인자 |
|---|---|
| `UserColumns` | `entityType?` |
| `AddUserColumn` · `UpdateUserColumn` | `entityType`, `fieldName`, (`newName`), `kind`, `choices?`(글 배열, 없으면 `[]`) |
| `RemoveUserColumn` · `MoveUserColumn` | `entityType`, `fieldName`, (`delta` 정수) |
| `SaveComputedColumn` | `entityType`, `fieldName`, `json`, `previousName?` |
| `PreviewComputedColumn` · `TraceComputedColumn` · `CompileComputedColumn` | `entityType`, `json` |
| `ColumnContext` · `Overrides` · `UserViewReaders` | `entityType` |
| `ValidateComputedSql` | `entityType`, `sql` |
| `PlanFreezeColumn` · `FreezeColumn` | `entityType`, `fieldName`, (`expected`: 정수 · `"$cells"` · 없음) |
| `SetUserField` · `SetOverride` · `ClearOverride` · `CheckCorrectable` · `FaceValue` | `key`, `fieldName`/`columnName`, (`value`, `SetUserField` 은 `null` 도) |
| `PlanDeletion` · `Delete` | `key`, `wholeSeries?`(없으면 false) |
| `Resolve` | `key` |
| `BrokenColumns` · `UserViews` · `UserViewContext` · `BuiltinViewPackage` | — |
| `TraceUserViewColumn` | `json`, `sampleKeys?`(글 배열 · 없음) |
| `UserViewSamples` | `viewJson`, `query?`, `edge?` |
| `PreviewUserView` · `SaveUserView` | `viewJson`, `sampleKey?` / `previousId?` |
| `DeleteUserView` · `ExportViewPackage` | `id` / `ids`(글 배열) |
| `PreviewViewPackage` · `ImportViewPackage` | `json`, (`token`) |
| `BatchEdit` | `view`, `changes`: `[{rowKey, field, value, original, operation}]` — 빠진 자리는 null 로 넘긴다 |
| `PlanBatchDeletion` · `BatchDelete` | `keys`(글 배열), `wholeSeries?`, (`fingerprint`) |
| `PinClock` | — **계기의 연산**(Store 가 아니다). 아래 |

- `key` 는 `Store.Resolve` 로 푼다. 못 찾으면 `그런 공고·계약이 없습니다: {key}` 를 오류로 적는다(다리와 같은 글).
- `json`·`viewJson` 은 글이어도 되고 JSON 값을 그대로 적어도 된다 — 값이면 기본 인코더의 압축 글로 다시 써서 넘긴다
  (두 계기가 같은 글자를 짓는다).
- **같은 키가 두 번 든 입력은 글(string) 인자로만 적는다.** 날것 값으로 적으면 Rust 계기는 읽으며 뒤 값 하나로 접어 C# 와
  다른 입력이 되므로, 대본 어디에든 겹친 키가 있으면 Rust 계기가 멎는다.
- 인자의 형이 틀리면(글 배열에 수, `wholeSeries` 에 글·null, `delta` 에 글, `changes` 가 배열이 아니거나 원소가 객체가 아님)
  C# 의 `JsonElement` 가 던지는 글(`The requested operation requires an element of type '…', but the target element has
  type '…'.`)을 오류 줄로 적고 다음으로 간다 — Rust 계기도 같은 글을 적는다.
- **자리표시** — 글 하나가 통째로 이것일 때만 푼다. 실제 파일의 값을 대본에 적지 않고 집어 오는 길이다.
  `$firstContract`·`$firstNotice`·`$firstRequest`(각 `v_계약`·`v_공고`·`v_접수` 의 번호 열을 `ORDER BY 1` 해 첫 것),
  `$viewId:<이름>`(`user_view` 에서 그 이름의 `id`), `$token`(마지막 `PreviewViewPackage` 의 표),
  `$fingerprint`(마지막 `PlanBatchDeletion` 의 지문), `$cells`(마지막 `PlanFreezeColumn` 의 칸 수, `expected` 에만).
  `BatchEdit` 의 `original` 이 `"$face"` 면 그 칸의 지금 값(`FaceValue`)을 넣는다. 번호를 못 집으면
  `자리표시 값이 없습니다: {자리표시}` 를 오류로 적는다.
- `PinClock` 은 `contract_user_field`·`notice_user_field`·`request_user_field`·`field_override` 의 `updated_at` 을 한
  값(`2000-01-01T00:00:00.0000000Z`)으로 못박는다. 일괄 지우기의 지문은 그 행들을 담으므로 두 Core 가 다른 순간에 적은
  시각이 지문을 가른다 — 지문을 견주려면 그 앞에 둔다.

**결과**는 `<file>.out.json` 하나(UTF-8, BOM 없음, LF)다 — `[`, 연산마다 한 줄, `]`. 줄은
`{"op":"<이름>","result":<값>}` 또는 `{"op":"<이름>","error":"<메시지>"}` 이고 줄 사이는 `,\n` 이다. 값은 다리
(`Bridge.Json` — camelCase, null 도 적고, 기본 인코더, 사전 열쇠는 그대로)의 꼴이다. 돌려줄 것이 없는 연산은 `null`,
`BrokenColumns` 는 튜플 열쇠라 `[[종류, 이름, 까닭], ...]` 로 적는다. 오류로 적는 것은 C# 의
`InvalidOperationException`·`ArgumentException`·`SqliteException`(Rust 는 그에 해당하는 `Error`)의 `Message` 이고, 적은 뒤
다음 연산으로 간다. 모르는 연산·인자 꼴의 잘못·앞선 연산 없는 `$token` 따위는 대본의 버그라 적지 않고 멎는다.

대본은 `scripts/` 에 둔다. **지어낸 값만** 적는다(`홍길동`, `123-45-67890` 처럼).

- `scripts/seed.json` — `seed` 파일(`tests/flow-cases.json` 의 TEST… 번호)에 대고 짠 것. 사람 열의 종류마다 더하기·
  고치기·옮기기·지우기와 그 거절, 사람 값·덮개와 되돌리기, `CheckCorrectable` 거절과 차수 뷰 거절, 가공열(흐름·SQL,
  깨진 식, `sqrt` 거절, 입력열을 가공열로), 깨진 열, 굳히기 계획과 굳히기, 맞춤 뷰의 표본·미리보기·보표·저장·지우기,
  보기 구성 내보내기·미리보기·가져오기(이름 겹침·열 충돌·틀린 표), 일괄 편집(틀린 칸이 섞이면 아무것도 안 함), 지우기
  계획과 지우기, 일괄 지우기 계획과 일괄 지우기(틀린 지문).
- `scripts/real.json` — 실제 자료의 사본에도 돌리는 작은 대본. 번호는 자리표시로만 집는다.

드라이버의 `Compare-Run` 이 같은 입력의 사본 둘을 C# 로 판올림한 뒤 C# 와 Rust 가 각자 같은 대본을 돌리게 하고, 사례
둘을 낸다 — `<이름>`(끝난 파일을 같은 자로 덤프해 견준 것)과 `<이름>.out`(결과 글).

**가림.** 결과 글은 소문자 16진 32자리 덩이(앞뒤가 16진 글자가 아니거나 `\uXXXX` 이스케이프 뒤)만 `m` 으로 바꾼 뒤
견준다 — 맞춤 뷰의 `id`, 가공식의 `id`, 그것을 품은 내보낸 구성처럼 `Guid.NewGuid()` 로 뽑는 것이다. 다른 것은 가리지
않는다(지문·표는 대문자 64자리라 걸리지 않는다). 파일 덤프는 판마다 뽑는 칸(`pclm_file.dataset_id`·`created_at`,
`user_view.id`, `v_맞춤뷰.뷰식별자`)에 더해, 편집이 적는 시각(`contract_user_field`·`notice_user_field`·
`request_user_field`·`field_override` 의 `updated_at`)과 가공열의 식별자(`v_가공열.식별자`)를 `--mask` 로,
식별자를 품은 JSON 칸(`user_column.options`·`v_가공열.가공식`)을 `--mask-hex` 로 가린다.

`synth`·`seed` 의 업서트·판올림 수선은 `internal` 이라 `Pclm.Core.csproj` 가 이 프로젝트에 `InternalsVisibleTo` 를 준다.

## 취합 (`merge`)

`merge <out> <src|folder>...` 는 `Merger.Merge(sources, out)`(Rust `merger::merge`)를 부르고 `MergeReportText.Render(report, out)`
(Rust `merge_report_text::render`)의 글을 `<out>.txt` 에 적는다 — UTF-8, BOM 없음, 줄바꿈은 `Environment.NewLine`(Windows 는
CR LF). `pclm merge` 가 화면에 찍는 글과 같은 바이트이고, 다른 것은 표준 출력이 아니라 파일이라는 것뿐이다. 받아들인
제출본이 없어 `InvalidOperationException`(Rust `Error::InvalidOperation`)이 나면 명령줄처럼 그 글과 줄바꿈을 같은 자리에
적고 2 로 끝낸다.

명령줄과 달리 원천을 고르지 않는다 — 폴더는 `Merger.FindSubmissions` 로 펴고, 그 밖의 인자는 **없는 파일이라도** 차례
그대로 넘긴다(명령줄은 없는 경로를 `※` 줄로 빼지만, 여기서는 `Merger` 의 `파일이 없음` 거절을 견준다). 작업 폴더는 두 계기
모두 임시 폴더 아래 `pclm-merge-<8자>` 이다 — C# 은 `Merger` 가, Rust 는 계기가 짓고 지운다(Core 는 폴더를 짓지 않는다).
보고의 월/일은 `Z` 붙은 시각을 그 기계의 시각으로 바꾼다 — C# 은 `DateTime.TryParse`, Rust 계기는 `Env::utc_offset_seconds`
를 Windows 의 `SystemTimeToTzSpecificLocalTime` 으로 잰다.

드라이버의 `Compare-Merge` 가 같은 입력 묶음을 두 계기에 넘기고 사례 둘을 낸다 — `<이름>`(두 취합본을 C# 덤프로 견준 것)과
`<이름>.txt`(보고 글을 바이트로 견준 것). 입력 파일은 취합 전후의 SHA256 이 같아야 한다.

**입력.** C# 계기가 짓는다 — `seed` 의 작업자료 사본 셋(`work-a`·`work-b`·`work-c`)에 편집 대본 `scripts/merge-{a,b,c}.json`
(사람 값·덮개·사용자 열, `PinClock` 으로 같은 시각을 만든 자리 포함)과 날 SQL `scripts/merge-{a,b,c}.sql`(접수·공고·계약 줄,
품목, 상대자, 계획, 맞춤 뷰 이름·정의, 제출자 이름)을 시키고, 둘은 `submit` 으로 제출본을 뜬다. 셋째는 작업자료 그대로
넣는다(제출자 이름이 없어 파일 이름으로 불린다). 거절할 것은 없는 파일, PCLM 이 아닌 파일(글 파일), 옛 시험판
(`merge-old.sql`, v11), 새 판(`merge-newer.sql`, v39), 취합본, 백업이고, 옛 판(v31) 작업자료(`synth`)가 사본 위에서 올라간다.
겹침은 상대자·계획·접수·공고·계약·접수/공고/계약 사람값·덮개·사용자 열·맞춤 뷰마다 하나 이상이고, 같은 값을 다른
시각에 든 줄(알리지 않음)과 이름을 바꾼 예시 보기(같은 식별자 → 새 식별자)가 섞인다.

| 사례 | 입력 |
|---|---|
| `merge-conflicts` | 위 전부(제출본 둘·작업자료·v31 + 거절할 것 여섯) |
| `merge-folder` | 폴더 하나(제출본 둘, 취합본, `.txt`) — `FindSubmissions` |
| `merge-single` | `seed` 작업자료 하나 |
| `merge-none` | 없는 파일·PCLM 아님·취합본 — 둘 다 2 로 멈추고 취합본을 짓지 않는다. 글만 견준다 |

**가림.** 취합본 덤프는 `--mask pclm_file.dataset_id`·`pclm_file.created_at`(취합본이 새로 받는 신원과 시각),
`user_view.id`·`v_맞춤뷰.뷰식별자`(이름이 갈린 맞춤 뷰가 같은 식별자로 오면 뒤의 것이 `Guid.NewGuid()` 로 새 식별자를 받는다)
만 가린다. 다른 시각 칸은 입력에서 그대로 오므로 가리지 않는다. 보고 글은 마지막 줄의 취합본 경로만 `<out>` 으로 바꾼다
— 두 계기가 다른 자리에 짓기 때문이다. 파일 이름·제출자·시각은 가리지 않는다.

## 확장 호스트 (`host`)

`host <home> <transcript> <out> [origin]` 은 `src/Pclm.App/Program.cs` 의 호스트 갈래를 그대로 옮긴 것이다 — 홈만
`new Home(<home>)` 으로, stdin·stdout 만 파일로 바꿨다. `ExtensionPresence.Serve` 에 같은 dispatch(`OpenBound` →
`ErpCapture.Dispatch`, 묶지 못하면 `setup_required`)를 넘기고 등록은 보지 않는다(`--native-dev` 처럼). `origin` 을 주면
업무 호스트처럼 `IsAllowedOrigin` 으로 대조해 아니면 2. 종료 코드는 Program.cs 와 같다 — 0 정상, 1 틀·입출력 오류, 2 확장.
Rust 쪽은 실행 파일 자체다: `pclm-erp-host --native-dev --home <home>`(origin 이 있으면 `--home <home> <origin>`), stdin·stdout
을 파일에 바로 잇는다.

**홈.** `host-home` 이 `seed` 의 읽기 전용 사본(`VACUUM INTO`)을 `<home>\work.pclm` 으로 두고, `extension-origin.txt` 에
등록된 적 없는 ID 를 적고, 갈래대로 쪽지를 쓴다 — `ok`·`missing`(쪽지 없음)·`corrupt`·`duplicate`(workfile 이 둘, 뒤의 것이
진짜)·`relative`·`foreign`(다른 datasetId)·`version`(2)·`nulls`·`array`·`submission`(제출본을 가리킴). 드라이버는 **같은 경로**에
홈을 지어 C# 을 돌리고 `.cs` 로 이름을 바꾼 뒤, 다시 지어 Rust 를 돌리고 `.rs` 로 바꾼다 — 답의 `destination` 과 쪽지의
경로가 두 쪽에서 같다.

**대본.** `host-transcripts` 가 짓는다. 값은 모두 지어낸 것이다(공고 번호 `T26BK00000901` 등). 요청 ID 는 고정 GUID.

| 대본 | 내용 | 종료 |
|---|---|---|
| `full` | 인사(dispatch) · 상시 연결 인사 · 탭 보고 · 사건(오류·풀림) · 미리보기 · 저장 · 결과 조회 · 같은 저장 다시 · 저장 뒤 미리보기 · 없는 결과 · 다른 datasetId · 파일 범위 · 받지 않는 요청 · baseToken 없는 저장 · 프로토콜 1 · GUID 아닌 요청 ID · 배열 · 글 · method 없음 · 깨진 JSON · 겹친 열쇠 · 깊이 33 · 끝에 길이 초과 머리 | 1 |
| `setup` | 상시 연결 인사 · 인사 · 미리보기 · 결과 조회 · 배열 · 글 · 수 요청 ID · 깨진 JSON — 모든 홈 갈래에 흘린다 | 0 |
| `truncated` | 길이 100 에 몸 7바이트 | 1 |
| `zero` | 길이 0 | 1 |

저장의 `baseToken` 과 선택은 `host-transcripts` 가 seed 의 사본에서 미리 본다 — 두 호스트가 같은 seed 의 사본에서 시작하므로
같은 것을 본다(저장이 둘 다 받아들여지는 것으로 확인된다).

**견주기.** 사례마다 셋 — `<이름>`(틀을 푼 답, 한 줄에 하나, 꼬리가 남으면 그 바이트 수), `<이름>.home`(끝난 홈의 파일·폴더
목록과 `.pclm*` 아닌 파일의 글), `host-full` 은 `<이름>.db`(작업자료 C# 덤프)도. 두 쪽 종료 코드가 기대와 같아야 한다.

**가림.** 이것만 가린다.

- 답: `"baseToken":"<64자 16진>"` → `<m>`. 지금 상태의 지문이라 저장 뒤에는 고친 시각을 담는다. 저장 앞의 것은 두 쪽이 같아야
  저장이 받아들여지므로 가려도 잃는 것이 없다.
- 홈 글: `yyyy-MM-ddTHH:mm:ss` → `<t>`(연결 기록의 줄 머리, 마지막 연결의 `at`), 홈 경로(쪽지의 workfile, 글 그대로와 JSON
  이스케이프 둘 다) → `<home>`.
- 덤프: `erp_capture.captured_at`·`erp_source.updated_at`·`notice.updated_at`·`v_ERP원천.수집시각`.

pid·프로세스 시작 시각이 든 연결 파일(`extension-live\<pid>.json`)과 탭 보고는 포트가 닫히면 지워지므로 가리지 않는다 —
남으면 목록에서 갈린다. 시각을 담지 않은 칸(영수증·요청 지문 등)은 가리지 않는다.

**창 잠금(`host-lock`).** 한쪽이 `lock <home> --hold` 로 잡은 채 머무는 동안 다른 쪽이 같은 홈·대문자 경로·끝 구분자를 붙인
경로를 `busy` 로, 다른 홈을 `free` 로 보는지 — C# 이 잡고 Rust 가 보기, Rust 가 잡고 C# 이 보기. 홈 이름에 `ı`·`ä` 를 넣는다
(.NET `ToUpperInvariant` 는 `ı` 를 그대로 둔다 — Rust `to_uppercase` 를 쓰면 다른 커널 객체 이름이 된다). 기대 글과 견준다.

**견주지 않는 것.** 등록으로 뜬 업무 호스트의 등록 확인(실제 HKCU 를 읽는다), 실행 파일 비키기로 떠나기(시간에 달렸다 —
`pclm-host` 의 단위 시험이 본다), 창이 놓은 명령의 밀어 보내기, 실제 브라우저.

## 홈 (`config-write` · `resolve` · `switch`)

`pclm-home`(#8 ④)이 C# `Home`·`HomeSwitch` 와 같은 홈을 남기는지 잰다. `resolve`·`switch` 는 **글을 표준 출력에** 낸다 — UTF-8(BOM
없음), 줄 끝 LF, 칸 사이는 탭, 글은 덤프의 `enc()`. 예외는 `error	<갈래>	<enc(글)>` 한 줄로 적고 0 으로 끝낸다. 갈래는
C# `PclmFileException`·`InvalidOperationException`·`IOException`·`ArgumentException`·`SqliteException` 이 `pclm`·`invalid`·
`io`·`argument`·`sqlite`(Rust `Error` 의 같은 갈래)다.

- `config-write <home> <workfile> <datasetId> [retired]` — `Home.WriteConfig`. 낸 것은 없고 `<home>\config.json` 을 견준다.
- `resolve <home> [--legacy <config>]` — `new Home(home, legacy).Resolve()`. 열었으면 `ready`, `database	<경로>`, 알림마다
  `note	<글>`. 열지 않았으면 `problem	<ProblemKind>`, `path	<경로>`, `message	<글>`.
- `switch <op> <home> <current> <args>...` — `current` 를 쓰기 손잡이로 쥔 창이 하는 바꾸기 하나. `move <target>` ·
  `use <path>` · `from-snapshot <source> <target>` · `new <target>` · `backup <target>` · `check <target>`(`CheckNewTarget`).
  `ok	<낸 경로>` 또는 `error` 줄.
- `inspect <file>`(C# 만) — 드라이버가 홈을 지을 때 신원을 집는다. `<Kind>	<role>	<version>	<datasetId>`.

드라이버의 `Compare-ConfigWrite` 는 두 계기가 각자의 홈에 같은 인자로 쪽지를 쓰게 하고 `config.json` 을 바이트로 견준다 —
한글·`&'+<>` 와 `` ` ``·NBSP·U+00AD·BMP 밖 글자·`/` 와 `..` 와 끝의 점·상대 경로(작업 폴더는 저장소 뿌리)·`retired`.

`Compare-HomeCase` 는 입력 자리(`<사례>\home`·`data`·`old`·`roaming`)를 **한 번** 짓고 복사해 둔 뒤, 같은 경로에 두어 C# 를
돌리고 `.cs` 로, 다시 두어 Rust 를 돌리고 `.rs` 로 이름을 바꾼다 — 입력의 무작위 신원과 쪽지·알림의 경로가 두 쪽에서 같다.
사례마다 셋 — `<이름>`(낸 글), `<이름>.tree`(끝난 자리의 파일·폴더 목록과 SQLite 가 아닌 파일의 글 — 쪽지·옛 쪽지·남이 둔
파일), `<이름>.db`(SQLite 파일마다 C# 덤프).

| 사례 | 입력 |
|---|---|
| `resolve-first-run` | 빈 자리 — 작업자료와 쪽지를 짓는다 |
| `resolve-legacy-home` · `-roaming` · `-pending` | 홈의 `pclm.db`(v19) · 옛 쪽지의 `dataDir`(홈의 것보다 먼저) · 밀린 이사(`pendingMoveFrom`) |
| `resolve-legacy-refused` | 홈의 `pclm.db` 가 v11 — 짓지 않고 멈춘다 |
| `resolve-default-exists` | 홈에 `계약자료.pclm` 과 `pclm.db` — 옛 것을 옮기지 않는다 |
| `resolve-ok` · `-case-path` · `-mismatch` | 쪽지가 작업자료를 · 대문자 경로로 · 다른 신원으로 가리킨다 |
| `resolve-missing` · `-not-pclm` · `-submission` · `-too-old` · `-too-new` | 시작하지 않는 갈래 |
| `resolve-old-v19` · `-old-v31` | 판올림 전 백업 — v19 는 날것, v31 은 역할 `backup` 의 사본 |
| `resolve-prune` | 지어낸 백업 열둘(같은 시각 둘)·메모 + v19 — 열 벌만 남긴다 |
| `resolve-retired` · `-retired-not-ours` | 옮기기가 남긴 옛 자리(역할 `retired` · 남의 작업자료) |
| `resolve-corrupt-*` | 판 2 · 상대 경로 · `workfile` 없음 · 공백 · `null` 칸 · 뿌리 `null` |
| `switch-move` · `-exists` · `-wal` · `-same` · `-unc` | 옮기기와 그 거절 |
| `switch-use` · `-submission` · `-current` · `-missing` · `-stale` | 다른 작업자료 쓰기와 그 거절 — `stale` 은 쪽지가 이미 딴 데를 가리킨다 |
| `switch-from-submission` · `-v19` · `-v31` · `-not-pclm` · `-exists` | 사본에서 새 작업자료 |
| `switch-new` · `-exists` | 새 계약자료(없는 폴더 안) |
| `switch-backup` · `-over` · `-work` · `-unc` | 백업(없는 폴더 안 · 옛 백업 덮기 · 작업자료는 덮지 않기 · 네트워크) |
| `switch-check` · `-exists` | `CheckNewTarget` |

**가림.** 낸 글·목록에서는 사례 자리(글 그대로와 JSON 이스케이프 둘 다)를 `<root>` 로, 백업 이름의 시각
(`계약자료_v<n>_yyyyMMdd-HHmmss`)을 `<t>` 로 바꾼다. 새 신원을 뽑는 사례(`-RandomId` — `resolve-first-run`·
`switch-from-*`·`switch-new`)만 소문자 16진 32자리 덩이를 `<id>` 로 바꾼다(쪽지의 `datasetId`). 덤프는 `$randomColumns`
(`pclm_file.dataset_id`·`created_at`, `user_view.id`, `v_맞춤뷰.뷰식별자`)만 가린다.

**견주지 않는 것.** 깨진 JSON·틀린 형의 쪽지 — C# 은 `System.Text.Json` 의 영어 예외 글(`'0xEA' is an invalid start of …`)을
`message` 에 싣고 Rust 는 `쪽지의 JSON 을 읽지 못했습니다.` 를 싣는다. 열람 사본 걷기(`PruneViews`, 지금 시각에 달렸다 —
`pclm-home` 의 단위 시험이 본다), 네트워크 드라이브(UNC 만 견준다), 전환 중 줄 선 저장(경합 — 두 쪽 단위 시험이 본다).

## 다리 (`bridge`)

`bridge <home> <file> <script.json> [--view] [--files <dir>] [--clock now]` 는 진짜 다리를 세우고 대본의 부름을 차례로
넘긴다 — C# 은 `new Bridge(database, new Home(home), viewed)` 의 `Invoke`(Pclm.App, `InternalsVisibleTo`), Rust 는
`pclm-bridge` 의 `Bridge::invoke`. `--view` 면 `PclmFile.OpenView(file, home.ViewDirectory)`(Rust `pclm_file::open_view`,
사본은 홈의 `view` 에 `열람-<id>.pclm`)로 열람 창처럼 연다. 창(WPF·WebView2)은 띄우지 않는다. 두 계기 모두 `IsDefaultHome`
은 거짓이다 — 업무 홈에서만 받는 요청(`prepareExtension`·`extensionStatus`)은 거절 글을 견주고, 그 몸은 `direct` 로 견준다.

**대본**은 부름의 JSON 배열이다 — `[{"method": "sheet", "args": ["v_계약"]}, {"method": "status"}]`. `args` 는 없어도 된다.
원소는 화면의 `HandleAsync` 처럼 편다: 글은 글, `null` 은 null, 그 밖의 값은 JSON 글(C# 은 원문 `GetRawText`, Rust 는
다시 지은 짧은 글 — 공백·이스케이프를 원문과 다르게 적은 대본에서만 갈리므로 대본은 인자를 글이나 null 로 둔다).
부름에는 더 적을 수 있다:

- `"pick"` — 고르기 창의 답(아래 「엑셀」). 저장 · 계획 엑셀 · 폴더 · 열기 · 보기 구성 JSON 고르기가 모두 이것을 받는다.
- `"tabs"` — 부르기 전에 이 계기가 **확장 호스트인 것처럼** 홈의 `extension-live` 에 연결(`<pid>.json` — 브라우저 Chrome,
  판 0.9.0, 이 프로세스의 시작 시각, `connectedAt` 은 `2026-09-10T09:00:00`)과 그 탭 보고(`<pid>.tabs.json`)를 적는다. 값이
  객체면 그 JSON, 글이면 그 글이 보고다(수 자리에 자리표를 쓰려면 글로 적는다). 끝나면 `extension-live` 를 통째로 걷는다.
- `"direct"` — 다리의 문을 거치지 않는 부름. `extensionStatus`(`ExtensionStatus.Read(home, EmbeddedVersion, Now)` /
  `extension_status_at`), `prepareExtension`(`args` 는 `--files` 아래의 폴더 · exe — `BundledExtension.Prepare` /
  `prepare_bundled_extension`, 등록부는 쓰지 않는다), `autoCaptureTick`(창이 1초마다 도는 자동 수집 한 번 — C# 은 private
  `AutoCaptureTick` 을 비춰 부르고, Rust 는 `Bridge::tick_auto_capture` 에 계기가 쥔 `AutoCapture` 를 넘긴다),
  `pinClock`(사람 값 · 덮개의 시각을 못박는다 — 일괄 지우기의 지문이 그 시각을 담는다, 편집 대본의 `PinClock`).

**자리표.** 인자 글과 탭 보고 안의 `{$…}` 는 넘기기 전에 바꾼다 — `{$pid}`(이 계기의 프로세스 번호), `{$mapping}`(작업자료의
쓰는 매핑 판, `Mapping.Hash`), `{$dataset}`(작업자료의 신원), `{$nowMs}`(지금의 유닉스 밀리초 — 자동 수집은 8초 안의 읽기만
받는다), `{$face:<열쇠>:<열>}`(그 칸의 지금 계약면 값 — `batchEdit` 의 `original`), `{$view:<이름>}`(그 이름의 맞춤 뷰
식별자), `{$<n>.<경로>}`(n 번째 답의 `result` 에서 `.` 로 이은 열쇠 · 차례를 따라간 값 — 글이면 글, 아니면 JSON 글. 실패한
답은 값이 없다). 찾지 못한 자리표는 그대로 둔다. 바꾼 값을 JSON 으로 이스케이프하지 않는다 — 따옴표 · 역빗금이 없는 값에만 쓴다.

**답**은 부름마다 한 줄, 번호는 1부터 차례로다. 줄은 C# `Bridge.ReplyJson(id, ok, result, error)` 의 글 그대로 —
`{"id":…,"ok":…,"result":…,"error":…}`(camelCase, null 도 적는다, System.Text.Json 기본 인코더 — 한글은 `\uXXXX`,
`+` 는 `+`). 실패는 `HandleAsync` 처럼 무엇이든 그 메시지를 `error` 에 싣는다. UTF-8(BOM 없음) · LF. 이 계기의
프로세스 번호(`"pid":<n>` · `"<n>:` · `<n>.cmd`)는 `<pid>` 로 바꾼다 — 두 계기는 다른 프로세스다.

**창이 다는 것**은 계기가 단다(Rust 는 `pclm_bridge::Host` 를 구현한다). 건네받은 것과 창에 넘긴 일은 한 줄씩
`<file>.prompts.txt` 에 적는다(UTF-8 · LF, 번호는 그 부름의 것):

    <번호>	save	<제목>	<거르개>	<기본 확장자>	<지어 둔 이름>	<덮어쓰기 true|false>
    <번호>	planExcel
    <번호>	folder	<제목>	<처음 자리>
    <번호>	open	<제목>
    <번호>	json	<제목>
    <번호>	closeToTray	<true|false>
    <번호>	restart
    <번호>	close
    <번호>	command	<확장에 보낸 명령 파일의 글>

`restart` · `close` 는 답을 보낸 뒤 창에 넘길 일이다 — C# `HandleAsync` 가 답 뒤에 바꿔 쥐는 `_restart` · `_close` 를 계기가
부름마다 비춰 읽고 내린다(Rust `take_restart` · `take_close`). `command` 는 부름 뒤 `extension-live\<pid>.cmd` 에 놓인 명령
파일을 이름 차례로 읽어 적고 지운 것이다. 운영체제는 Rust 계기에서 읽기만 진짜고(등록부 읽기 · 판 · 이름 붙은 신호), 등록부
쓰기 · 탐색기 · 새 프로세스 · 클립보드는 거절한다 — 대본은 그것을 부르지 않는다(아래).

**「지금」.** 기본은 두 계기가 같은 순간 `2026-09-10T00:00:00Z` 로 못박는다 — Rust 는 다리에 넘기는 `Env::now_utc`, C# 은
`BridgeParity.AsOf`. C# 다리의 `dashboard` 는 시계를 받지 않고 `Reports.Dashboard()` 가 `UtcNow` 를 읽으므로, C# 계기는
`dashboard` 만 다리를 거치지 않고 같은 `Reports(database).Dashboard(AsOf)` 를 불러 답 꼴만 다리의 것으로 싼다.
`dashboard` 는 열람 중에도 받는 요청이라(`ViewerAllowed`) 문지기를 건너뛰어도 갈리는 것이 없다. `--clock now` 면 Rust 도
진짜 시계로 돌고 C# `dashboard` 도 다리를 거친다 — 수집 기록 · 파일 이름의 날짜를 C# 다리가 `DateTime.Now` 로 읽으므로 홈
대본(아래)은 모두 그렇게 돌고, 드라이버가 시각을 가린다.

다리의 요청은 C# `Dispatch` 의 70 개 전부를 옮겼다(#8 ⑧a). 운영체제에 닿는 몸은 계기에서 견주지 않는다 — 부르는 자리
(창이 건네받는 것 · 거절 글)까지만 견준다: `openOther` 가 새 프로세스를 띄우는 길, `revealDataFolder`, `openExtensionSetup`
의 폴더 · 브라우저 길(모르는 대상의 거절만 견준다), 업무 홈의 `prepareExtension`(등록부), 자동 실행을 바꾸는
`saveWindowPrefs`(시험 홈이라 거절 글을 견준다). 그 밖은 `pclm-bridge` 의 단위 시험이 가짜 `Os` 로 본다.

| 대본 | 내용 |
|---|---|
| `scripts/bridge-read.json` | 읽기 요청, `Views.Names` 의 뷰마다 `sheet`, 저장한 맞춤 뷰(`u_서식용 예시` — 예시 보기), 저장되지 않은 `u_` 이름·내부 표·따옴표 든 이름(내보이지 않는 표), 값 없음·null(1번째 값이 없습니다), 남는 인자, 다시 읽은 `dataVersion`, 모르는 요청 둘(빈 이름 포함) |
| `scripts/bridge-view.json` | 열람 문지기 — 고치는 요청(`setField`·`saveSettings`·`batchEdit`·`switchWorkfile`)과 어느 목록에도 없는 요청, 그리고 읽기 둘 |
| `scripts/bridge-edit.json` | `seed.json`(편집 대본)을 다리 부름으로 옮긴 것 — 사용자 열 · 사람 값 · 덮개 · 일괄 편집(`{$face:…}`) · 가공열 · 굳히기(`{$n.cells}`) · 맞춤 뷰 · 보기 구성(내보내기는 `pick`, `{$view:…}` · `{$n.token}`) · 지우기 · 일괄 지우기(`pinClock` 뒤 `{$n.fingerprint}`). 다리에 없는 연산(FaceValue · Overrides · CheckCorrectable · Resolve · PreviewComputedColumn · BrokenColumns · UserViewReaders)은 뺐다. 끝에 다리에만 있는 꼴 거절 셋 — 칸의 꼴이 틀린 `batchEdit`(.NET 은 기록의 이름 `CellChange` 를 든다) · null 원소 뒤의 틀린 칸 · 글이 아닌 원소의 `batchDeletionPlan` — 을 더했다 |
| `scripts/bridge-erp.json` | 확장 요청의 거절 글과 `direct` 몸(내장 확장 준비 · 상태), 지금 보는 화면(탭 보고 없음 · 수집할 탭 · 읽는 중 · 모르는 화면 · 모양이 틀린 보고), 확장에 보내는 명령(보낸 것은 `command` 줄), 가져오기(수집 ID · 낡은 지문 · 닫힌 탭 · 고른 자리 · 저장), 고급 도구, 수집 기록, 자동 수집 설정(대소문자 · 건너뛰는 모르는 칸 · 꼴이 틀린 칸 · null 이름 · 빈 값)과 `autoCaptureTick`(저장 · 같은 화면 · 기다림 · 다른 자료 · 다른 화면 · 이름 조건 · 깨진 보고) |
| `scripts/bridge-files.json` | 제출본(취소 · 덮어쓰기 · 이름 넣은 이름 · 없는 폴더 · 작업자료 자리), 취합(취소 · 빈 폴더 · 없는 폴더 · 두 번), 백업 |
| `scripts/bridge-workfile.json` | 작업자료 바꾸기의 고르기(옮기기 · 새로 · 쓰기 — 작업자료 · 남의 작업자료 · 제출본 · 깨진 파일 · 없는 파일)와 `moveWorkfile`·`switchWorkfile`·`newWorkfile`(쪽지를 고친 뒤의 `restart`) |
| `scripts/bridge-window.json` | `session`·`about`·`windowPrefs`·`saveWindowPrefs`(알림 영역 · 자동 실행 거절 · 틀린 깃발)·`setUnsaved`·`openOther` 의 거절·`dataLocation`·`closeWindow` |
| `scripts/bridge-gate-all.json` | 열람 창의 문 — `Dispatch` 의 요청 전부를 값 없이(`revealDataFolder` 만 뺐다 — 열람 중에도 받아 탐색기를 띄운다), 그리고 열람 창에서 작업자료를 열어 보기 |

| 사례 | 입력 |
|---|---|
| `bridge-seed` | `seed` 의 사본에 `bridge-read.json` — 작업자료 창 |
| `bridge-seed-view` | 같은 대본을 `--view` 로 — 모든 표가 읽기 전용이 된다(`editable`·`correctable` 빈 배열, `overrides` 빈 객체) |
| `bridge-seed-gate` | `bridge-view.json` 을 `--view` 로 |
| `bridge-edit`·`bridge-erp`·`bridge-files`·`bridge-workfile`·`bridge-window` | 그 대본을 홈에 — `Compare-BridgeHome` |
| `bridge-gate-all` | `bridge-gate-all.json` 을 `--view` 로 홈에 |
| `bridge-real-<n>`·`bridge-real-<n>-view` | `-Real` 이면 실제 사본마다 `bridge-read.json`. 답 글은 `artifacts/` 에만 남는다 |

드라이버의 `Compare-Bridge` 는 입력을 C# 로 쪽마다의 폴더에 **같은 이름**(`계약자료.pclm` — 문지기의 거절 글에 파일 이름이
든다)으로 떠 판올림한 뒤 두 계기를 돌리고, 답 글을 바이트로 견준다. 콘솔에는 부름 수와 거절 수만 낸다. 다리를 놓은 뒤
홈의 `view` 에 열람 사본이 남았으면 실패다.

`Compare-BridgeHome` 은 홈 사례(아래 「홈」)처럼 **같은 자리**에 같은 입력 — seed 의 사본 `data\계약자료.pclm` 을 쪽지로 묶은
`home`, 고르기가 답할 `files` — 을 두 번 두고 두 계기를 `--clock now` 로 돌린 뒤 `.cs`/`.rs` 로 이름을 바꾼다. 사례는 넷이다 —
이름(답 글), 이름.prompts(창에 건넨 것), 이름.tree(끝난 자리의 목록과 글 파일의 글 — 쪽지 · 창 설정 · 자동 수집 설정 · 확장
준비물 · 보기 구성 · 취합 보고), 이름.db(SQLite 파일마다 C# 덤프 — 작업자료 · 제출본 · 취합본 · 백업 · 새 작업자료). 가리는
것은 사례 자리(`<root>`), 신원(32자 16진 · GUID), 시각(ISO 꼴 · 가져오기 답의 `HH:mm`), 기준 지문(`baseToken` — 갱신 시각을 담는다), 「오늘」 을 박은 이름(`_yyyyMMdd.`),
백업 이름의 시각과, 덤프의 `$cliColumns` · `$hostColumns` · `erp_capture.request_hash` · `result_json` 의 식별자 덩이다.
다리를 놓은 뒤 `view` 에 열람 사본이나 `extension-live` 가 남았으면 실패다. 두 계기는 다리를 놓은 뒤 작업자료 폴더와
`files` 에 남은 `-wal` 마다 쓰기 연결을 열어 읽고 닫는다 — 읽기 전용 연결이 마지막에 닫히면 `-wal` · `-shm` 이 남는데 C# 은
그 차례가 연결 풀에 달려 있어, 걷지 않으면 목록이 차례에 따라 갈린다. 깨진 JSON(문법 오류)의 거절 글은 .NET 의 것을 흉내 내지 않으므로 대본에 넣지 않는다.

## 엑셀 (`xlsx-dump` · `plan-fixtures` · 다리의 고르기)

#8 ⑥ 은 다리의 엑셀 부름 — `settings`·`saveSettings`·`export`·`exportSheet`·`pickPlanExcel`·`importPlan` — 을 옮겼다.
C# 은 ClosedXML 0.105.1(`Exporter`·`PlanImport`), Rust 는 `pclm-excel`(쓰개·읽개를 직접 지었다)이다.

**고르기 창.** `bridge … --files <dir>` 이면 부름의 `"pick"` 이 고르기 창의 답이다 — 글이면 `<dir>` 아래의 그 자리(`/` 는
`\` 로 바꿔 `<dir>\<pick>` 으로 잇는다), `null` 이거나 없으면 취소. 창에 건넨 물음은 한 줄씩 `<file>.prompts.txt` 에 적는다
(꼴은 위 「다리」).

지어 둔 이름의 날짜는 `<오늘>` 로 바꾼다 — C# 다리는 `DateTime.Now` 로, Rust 다리는 못박은 「지금」 의 로컬 날짜로 짓기
때문이다(답 글에는 날짜가 들지 않는다 — 고른 경로를 쓴다). 두 쪽이 같은 `--files` 폴더를 쓰므로 답 글의 전체 경로도 같다.

**`plan-fixtures <dir>`**(C# 만)은 `pickPlanExcel` 이 고를 계획 엑셀 넷을 ClosedXML 로 그 자리에서 짓는다 — 값은 모두
지어낸 것이고 저장소에는 엑셀을 두지 않는다.

| 파일 | 내용 |
|---|---|
| `표본.xlsx` | 표본 머리글 열둘. 정상 줄 · 통째로 빈 줄 · 번호가 빈 줄 · 한 파일 안의 같은 번호 · 자릿점 수량(`1,200`) · 수로 적은 수량 |
| `이상한칸.xlsx` | 머리글이 셋째 줄 B 열부터 · 앞뒤 공백 머리글 · 같은 머리글 둘 · 표본 밖 열. 칸은 큰 수 · 소수 · 음수 · 참거짓 · 오류 · 줄바꿈 글 · 런 글 · 날짜(DateTime · 시각 · 사용자 꼴 · 꼴 번호 14 · 1900 윤년 앞뒤) · 날짜 꼴의 글 · 시간 · `0.00` · `@` 꼴의 수 · `_x0041_` 꼴의 글 · 값 없이 꼴만 · 메모만 있는 칸 |
| `머리글틀림.xlsx` | 필수 둘이 없다 — 한 줄도 넣지 않는다 |
| `소수.xlsx` | 둘째 줄의 지시수량이 수 1.5 — 앞선 줄까지 되돌린다 |

**`xlsx-dump <file.xlsx> <out.txt>`**(두 계기)는 통합 문서 한 권을 정규 글로 낸다. 견줄 것을 지은 쪽의 코드로 읽지 않도록
두 계기 모두 zip 을 열어 부분(part)의 XML 을 그대로 읽는다(C# `ZipFile` + `XDocument`, Rust `zip` + `quick-xml` —
`pclm-excel` 의 읽개를 쓰지 않는다). 관계(`_rels`)를 따라 통합 문서 · 꼴 · 공유 문자열 · 시트를 찾는다. UTF-8 · LF, 줄마다
탭으로 가른다:

    pclm-parity-xlsx 1
    sheet	<차례>	<이름>
    pane	<xSplit>	<ySplit>	<topLeftCell>	<activePane>	<state>
    autofilter	<ref>
    col	<min>	<max>	<width>	<customWidth>	<style>
    cell	<ref>	<t>	<수 꼴>	<굵게>	<채움>	<줄바꿈>	<따옴표 머리>	<=식|->	<값>

- 시트는 통합 문서의 차례, 그 안에서 `pane` · `autofilter` · `col` · `cell` 차례. 없는 속성은 `-`.
- `col` 의 너비는 **적힌 글 그대로** 견준다 — 너비에 허용 오차를 두지 않는다. Rust 는 ClosedXML 의 `AdjustToContents(1, 40, 60)`
  을 같은 글꼴(이 기계의 Calibri)로 다시 재어 같은 글(`40.710625` 따위)을 적는다. 이웃한 같은 너비의 열은 ClosedXML 처럼
  `<col>` 하나로 접힌다.
- `cell` 은 줄, 그다음 열의 차례. `t` 는 적힌 형(없으면 `n`). 꼴은 칸의 `s` 가 가리키는 `cellXfs` 를 풀어 적는다 — 수 꼴
  (`numFmts` 에 있으면 그 꼴 글, 없으면 `#번호`), 굵게(글꼴의 `<b>`, 1/0), 채움(`patternType[:fgColor rgb]`), 줄바꿈
  (`alignment wrapText`), 따옴표 머리(`quotePrefix`). 꼴의 **번호와 차례는 견주지 않는다** — 칸마다 푼 값만 견준다.
- 값 — `s` 는 공유 문자열(`<t>` 하나거나 런을 이은 것, 발음 표기 뺌), `inlineStr` 은 그 글, 그 밖에는 `<v>` 의 글. `_xHHHH_`
  는 풀지 않는다(적힌 대로). 글은 XML 이 정한 대로만 푼다 — 날 줄끝(`\r\n`)은 `\n` 이 된다. 식은 `<f>` 의 글.
- 이스케이프는 덤프와 같다 — `\` · 탭 · `\n` · `\r` 은 `\\` · `\t` · `\n` · `\r`, 그 밖의 제어 글자는 `\uXXXX`.

**견주지 않는 것.** 칸에 닿지 않는 부분 — 테마(`theme1.xml`), 문서 속성(`docProps/app.xml`, 지을 때마다 이름과 시각이 바뀌는
core-properties), 관계 번호(`rId`), XML 접두사(`x:`)와 속성 차례, 공유 문자열의 차례, 시트 꼬리(`printOptions` ·
`pageMargins` · `pageSetup` · `headerFooter`). Rust 쓰개는 테마·문서 속성을 적지 않는다. 나머지 꼬리는 ClosedXML 이 적는
그대로 적지만 덤프에 들지 않는다.

| 대본 | 내용 |
|---|---|
| `scripts/bridge-excel.json` | `settings` · `saveSettings`(글 · null) · 고르지 않은 `importPlan` · 취소 · 없는 파일 · 계획 엑셀 넷의 고르기와 가져오기(같은 파일 두 번 포함) · `export` 취소와 내보내기 · `exportSheet` — 수식 꼴 · 줄바꿈(`\n` · `\r\n`) · `_x0041_` · 탭 · 앞뒤 공백 · 이모지와 묶음 · 제어 글자 · `<&>"'` · `'` 머리(값 · 머리글, `'` 하나뿐인 칸) · 빈 표 · 빈 칸만 · 있는 파일 덮어쓰기, 그리고 거절 — 시트 이름(`/` · `'` 머리 · 32자) · null 표 · 빠진 값 · 줄 길이 · 같은 머리글 · 빈 열 · 빠진 rows · null 열 · null 줄 · 취소 |
| `scripts/bridge-excel-view.json` | 열람 창 — `settings` · `export` · `exportSheet` 는 받고 `saveSettings` · `pickPlanExcel` · `importPlan` 은 문지기가 거절한다 |

| 사례 | 견주는 것 |
|---|---|
| `bridge-excel` · `bridge-excel-view` | 답 글(바이트) |
| `….prompts` | 고르기 창에 건넨 물음(바이트) |
| `….db` | 끝난 사본의 C# 덤프. 가리는 것은 `$randomColumns` 와 `plan.imported_at`(가져온 시각 — 두 계기가 각자 지금을 읽는다) |
| `…/<파일>` | 내보낸 통합 문서마다 — 먼저 **두 계기의 덤프가 같은 파일에서 서로 같아야** 하고(자끼리), 그다음 C# 이 쓴 파일과 Rust 가 쓴 파일의 C# 덤프를 견준다 |

드라이버의 `Compare-BridgeExcel` 은 `plan-fixtures` 를 C# 로 한 번 지어 두 쪽이 같은 `files` 폴더에서 읽게 하고, 쪽마다 다리를
돌린 뒤 내보낸 `files\out` 을 그 쪽의 `out` 으로 옮긴다. 실제 자료(`-Real`)에는 돌리지 않는다 — 계획 가져오기가 쓰는 요청이다.

**알고 둔 차이**(대본에 넣지 않는다):

- 깨진 JSON 표(`exportSheet` 의 2번째 값) — 거절 글의 뒤(System.Text.Json 의 영어 글과 줄·바이트 위치)가 serde_json 의 것과
  다르다. 앞(`JSON 을 읽지 못했습니다:`)만 같다.
- 열지 못하는 계획 엑셀(zip 이 아닌 파일 · 깨진 zip) — ClosedXML 의 예외 글을 옮기지 않았다. 없는 파일(`Could not find file
  '…'.`)만 같다.
- 값이 저장되지 않은 식 칸 — ClosedXML 은 그 자리에서 계산하고 Rust 는 빈 칸으로 읽는다. 엑셀이 저장한 파일에는 값이 있다.
- 따옴표가 닫히지 않은 수 꼴(`"0.00`) — C# 의 꼴 풀이(`GetDataTypeFromFormat`)가 처음으로 돌아가 멈추지 않는다(C# 버그,
  고치지 않았다). Rust 는 수로 본다.
- 1904 날짜 체계의 시간 · 셀 메타데이터 — 옮기지 않았다.
- 열 너비는 이 기계의 Calibri 로 잰다. Calibri 가 없으면 C# 은 ClosedXML 에 든 대체 글꼴(Carlito)로, Rust 는 가장 좁은 너비로
  둔다(대체 글꼴은 옮기지 않았다).
- `_xlnm._FilterDatabase` 의 시트 이름 — ClosedXML 은 따옴표가 필요할 때만, Rust 는 늘 싼다. 엑셀은 둘을 같게 읽고 덤프에
  들지 않는다.
- 묶음(grapheme) 경계는 두 쪽의 유니코드 판이 다르면 새 이모지에서 갈릴 수 있다(너비).

## 명령줄 (`pclm`)

#8 ⑦ 은 명령줄 `pclm`(C# `src/Pclm.Cli`)을 `crates/pclm-cli` 로 옮겼다. 계기 명령이 아니라 **두 실행 파일 자체**를 견준다 —
C# 은 `src/Pclm.Cli/bin/Release/net8.0/pclm.exe`, Rust 는 `target/release/pclm.exe`. 개발 명령줄(`run.ps1 -Cli`)도 Rust 쪽이다.

드라이버의 `Compare-Cli` 는 홈 사례처럼 입력 자리(`<사례>\home` · `data` · `in` · `out` …)를 **한 번** 짓고 복사해 둔 뒤, 같은 경로에
두어 C# 에 명령들을 차례로 시키고 `.cs` 로, 다시 두어 Rust 에 같은 명령들을 시키고 `.rs` 로 이름을 바꾼다. 명령마다 `--home
<사례>\home` 을 준다 — `--home` 없이는 돌리지 않는다(업무 홈 `%LOCALAPPDATA%\Pclm` 을 연다). 작업 폴더는 `<사례>\cwd` 다 — 기본
이름으로 떨어지는 파일(`관리대장_` · `제출_` · `취합_`)과 상대 경로가 거기 선다. 표준 입력은 비워 넘긴다(읽는 명령이 없다 —
`delete` 는 묻지 않고 `--예` 를 본다).

사례마다 셋 — `<이름>`(명령마다 `== <n> exit <종료 코드>` · 표준 출력 · 표준 오류. 바이트 그대로 UTF-8 로 풀고 CR 은 `<CR>` 로
드러낸다 — `Console.WriteLine` 의 CR LF 와 글 안의 LF 가 섞인다), `<이름>.tree`(끝난 자리의 파일 · 폴더 목록과 글 파일의 글,
통합 문서는 C# `xlsx-dump`), `<이름>.db`(SQLite 파일마다 C# 덤프). 작업자료 곁에 `-wal` · `-shm` 이 남으면 목록에서 갈린다.

입력은 C# 계기가 짓는다. `seed` 의 사본을 쪽지로 묶은 홈(`New-CliHome`), `-Rich` 면 거기에 `scripts/cli.sql`(계약 하나의 둘째
차수)과 `scripts/cli.json`(고르는 열 `단계표시` · 사람 값 · 덮개)을 더한다. 계획 엑셀은 `plan-fixtures` 로 한 번 짓고(`cli-plans`),
취합할 제출본은 「취합」 의 `merge-in` 에서 가져온다.

| 사례 | 명령 |
|---|---|
| `cli-args` | 없어진 `--db` · 모르는 옵션 · 값 누락(끝 · 다음이 옵션) · `--` 뒤의 `--긴급 --예` · `help` · 모르는 명령 · 대문자 명령 |
| `cli-file-refused` | `--file` 을 쓰는 명령에 — 거절 글에 소문자로 바꾼 명령(`PLAN` → `plan`)이 든다 |
| `cli-view` | `--file` 열람 — 제출본(`status` · 상대 경로의 `현황` · 경로 준 `export`) · 취합본의 기본 이름 `export` · v19(사본을 올린다) · PCLM 아님 · 없는 파일 |
| `cli-home-missing` · `-corrupt` · `-submission` | 시작하지 않는 홈 — 글에 경로가 들면 경로 줄을 더 쓰지 않고(없는 파일), 들지 않으면 쓴다(판 2 쪽지) |
| `cli-first-run` · `cli-legacy` · `cli-old-v31` | 처음 실행 · 옛 `pclm.db` 옮겨 오기 · 판올림 전 백업 — 알림마다 빈 줄 |
| `cli-read` | `status`(차수가 쌓인 건) · `현황` · `export` 기본 이름 · 없는 폴더 안 · 상대 경로 |
| `cli-plan` | 고른 것 없음 · 없는 파일 · 필수 머리글 없음 · 소수 지시수량 · 정상(건너뛴 줄) · 인자 없이 다시 읽기 · 이상한 칸 |
| `cli-set` | 열 목록 · 쓰는 법 · 없는 키 · 모르는 열 · 여러 낱말 · `--` 뒤 · 후보에 없는 값 · 후보 · 지우기 · 공고 · 접수 |
| `cli-fix` | 쓰는 법 · 없는 키 · 키 열 · 사람 열 · 없는 열 · 고치기 · 빈 값 `(빔)` · 되돌리기 · 공고종류(연결 다시) |
| `cli-delete` | 쓰는 법 · 없는 키 · 세어 보이기(차수 하나 · 전 차수 · 연결된 공고) · 지우기(차수 하나 · 사람 값과 덮개가 든 계열 · 접수) |
| `cli-submit` | 이름 없음(작업 폴더) · `--이름` 과 폴더 · 정해 둔 이름과 파일 자리 · 공백 이름 · 없는 폴더 안 · 끝 구분자 붙인 폴더 |
| `cli-merge` | 찾은 것 없음(인자 없음 · 없는 자리 · `.pclm` 없는 폴더) · 폴더와 `--out` 폴더 · 겹친 파일(대소문자)과 없는 파일 · 받을 것 없음 · 작업자료 덮기 · 기본 이름 |

**가림.** 글과 목록에서 사례 자리(글 그대로와 JSON 이스케이프 둘 다)를 `<root>` 로, 백업 이름의 시각을 `<t>` 로, 기본 파일
이름의 날짜(`_yyyyMMdd.xlsx|.pclm`)를 `<오늘>` 로 바꾼다 — 두 쪽은 같은 날 다른 순간에 돌고, C# 은 `DateTime.Now` 를 읽는다(Rust
`pclm-cli` 는 `Context` 의 시계 · 시간대로 읽어 단위 시험이 못박는다). `-RandomId` 사례(`cli-first-run` · `cli-legacy`)는 새로 뽑은
신원(소문자 16진 32자리)을 `<id>` 로. 덤프는 `$cliColumns` — `$editColumns`(신원 · 지은 시각 · 사람 값 · 덮개의 시각)와
`plan.imported_at` — 와 `$editHexColumns` 만 가린다. 계획 엑셀 · 취합 입력의 경로는 두 쪽이 같은 자리라 가리지 않는다.

**견주지 않는 것.** 인자 없이 부르기와 `--home` 이 첫 인자인 부르기(업무 홈을 연다). C# 이 잡지 않는 예외(내보낼 자리가 잠김,
작업자료 자리에 제출본 뜨기 따위) — .NET 은 「Unhandled exception.」 과 스택을 적고 `0xE0434352` 로 끝나는데, Rust 는 첫 줄의 꼴로
글만 적고 같은 종료 코드로 끝낸다(스택은 옮기지 않았다).

## 돌리기

```powershell
./scripts/parity.ps1                      # Rust 를 박제(tests/parity-golden)와 — fixture · create · seed · synth · merge · host · 홈 · 명령줄
$env:PCLM_PARITY_RECORD = '1'; ./scripts/parity.ps1   # C# 와 나란히 견주고, 모두 같으면 박제를 새로 뜬다(C# 계기가 있어야 한다)
./scripts/parity.ps1 -Erp <ERP JSON 폴더>  # (C# 와 나란히) seed 에 ERP JSON 을 더한다(C# seed 와 Rust seed 를 견준다)
./scripts/parity.ps1 -Real                # (C# 와 나란히) 실제 홈의 backups\*.pclm 과 지금 작업자료도
./scripts/parity.ps1 -NoBuild             # 지어 둔 계기로
```

사례마다 한 줄 — 이름, 표·뷰 수와 행 수, `same` 또는 `DIFF (줄 …)`. 하나라도 갈리면 종료 코드 1.

### 박제 (`tests/parity-golden`)

C# 계기가 없어도 같은 사례를 같은 자로 재도록, C# 계기의 부름을 테이프로 떠 둔다. 사례마다의 견주기(가리기 · 짝짓기 ·
목록)는 그대로이고, C# 부름만 갈아 끼운다.

- **뜨기**(`PCLM_PARITY_RECORD=1`). C# 와 Rust 를 나란히 돌리면서 C# 부름마다 인자 · 표준 출력 · 표준 오류 · 종료 코드와,
  그 부름 동안 작업 폴더에 생긴 변화(지은 · 고친 · 지운 파일과 폴더)를 적는다. 모든 사례가 `same` 이고 아래의 막음에 걸린
  것이 없을 때만 `tests/parity-golden` 을 갈아 쓴다. 지어낸 자료로만 뜬다 — `-Real` · `-Erp` 와 함께 뜨지 않는다.
- **견주기**(기본). C# 부름을 돌리는 대신 차례대로 되살린다 — 적힌 변화를 다시 쓰고 적힌 출력을 돌려준다. 인자가 박제와
  다르면(사례를 고쳤다) 그 자리에서 멈춘다. C# 덤프(`dump` · `xlsx-dump`)가 C# 가 짓지 않은 파일(Rust 의 산출물)을 읽는
  부름은 Rust 계기의 같은 명령이 맡는다 — 뜰 때 두 계기가 그 파일을 같은 바이트로 덤프하는지 본다. C# 부름이 Rust 의
  산출물을 다른 명령으로 읽으면 뜨기를 막는다(그때의 바이트를 되살려서는 Rust 를 재지 못한다).
- **C# 이 있을 때만 잴 수 있던 것.** 창 잠금(`host-lock`)은 두 자리 모두 Rust 가 잡고 본다. 교차 읽기(`read-*`)의 C# 읽기
  손잡이(`readcheck`)는 되살리지 않는다 — 박제는 그때 C# 이 그 파일을 `$editColumns` 를 가려 덤프한 것이고, 지금 Rust 가 지은
  같은 파일의 같은 덤프가 그와 같은지 · 깨진 뷰가 없는지 · 읽은 뒤 해시가 그대로인지 본다.
- **꼴.** `tape.tsv.gz` 는 줄마다 `call`(차례 · 갈래 tool/cli · replay/rust · 종료 코드 · 인자 · 출력, 글은 base64) ·
  `op`(차례 · file/dir/del/deldir · 상대 경로 · 내용) · `named`(교차 읽기의 기대 덤프) · `meta`(뜬 날 · 사례 수)다. 내용은
  `blobs/<sha256>.gz` 에 한 벌씩 둔다. 작업 폴더와 저장소 경로는 `<<work>>` · `<<root>>`(JSON 의 `\\` · 슬래시 꼴도)로 적고
  되살릴 때 지금 자리로 바꾼다 — 길이 머리로 이은 틀(`.bin`)은 틀마다 바꾸고 머리를 다시 적는다.
- **날짜.** 뜬 날과 다른 날 견주면 「오늘」(파일 이름 밖의 `yyyy-MM-dd` · `yyyyMMdd`)이 두 쪽에서 다르다. 그날과 오늘의 두
  꼴을 두 쪽에서 함께 `<<date>>` 로 바꾼 뒤 견준다. 갈리면 처음 갈린 줄과 양쪽 줄 몇 개를 보인다(박제는 지어낸 자료뿐이다).
- **다시 뜨기는 눈으로 본 뒤에만** — 계약면 박제(`PCLM_UPDATE_GOLDEN`)와 같다. 뜨기는 회귀를 덮어쓴다. C# 계기를 걷은 뒤로는
  다시 뜰 수 없다.

C# 은 원시 문자열의 줄바꿈을 **작업 트리의 바이트 그대로** 뷰 SQL 에 싣는다. `.gitattributes` 가 생기기 전에 받은 작업 트리에
CRLF 파일이 남아 있으면(`git ls-files --eol | grep w/crlf`) git 은 깨끗하다고 하는데 C# 의 뷰 SQL 에만 `\r` 이 들어, 뷰를
짓는 사례가 모두 `object view` 줄에서 갈린다 — 그 파일들을 LF 로 되돌린 뒤 돌린다.
산출물은 `artifacts/parity/<시각>/` 에 남는다(gitignore).

동작을 견줄 사례(「C# 가 사본 1 에 X, Rust 가 사본 2 에 X」)는 드라이버의 사례 목록에 `Compare-Op` 한 줄을 더한다.
두 사본은 같은 입력에서 읽기 전용으로 뜨고, 결과는 같은 자(C# 덤프)로 견준다. 편집 대본은 `Compare-Run` 이다 —
`seed` 파일에 `seed.json`·`real.json` 을, `-Real` 이면 실제 사본마다 `real.json` 을 돌린다. 다리 대본은 `Compare-Bridge`, 엑셀 부름이 든 다리 대본은 `Compare-BridgeExcel`, 홈에 닿는 다리 대본은 `Compare-BridgeHome`, 명령줄은 `Compare-Cli` 다.

## 실제 자료

`-Real` 과 `-Erp` 는 실제 조달 자료를 읽는다.

- 원본은 `snapshot`(읽기 전용 `VACUUM INTO`)으로만 떠 온다. 뜨기 전후의 SHA256 이 다르면 실패한다.
- 덤프·로그·편집 결과 글(`*.out.json`)·다리 답 글(`*.bridge.txt`)은 `artifacts/` 에만 남는다. 콘솔에는 수와 줄 번호만 낸다 — 칸 값을 내지 않는다.
- ERP JSON 폴더(`corpus/`)는 저장소 밖 자료다. 안으로 복사하지 않는다. 픽스처·문서에 그 값을 옮겨 적지 않는다.
