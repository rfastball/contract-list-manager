using Pclm.Core.Erp;
using System.Text;
using Pclm.Core.Storage;

Console.OutputEncoding = Encoding.UTF8;

var command = args.Length > 0 ? args[0].ToLowerInvariant() : "help";
// 창과 같은 자리를 본다. 갈라 두면 같은 컴퓨터에 DB 가 여럿 생기고,
// 밖에서 읽는 쪽은 어느 것이 진짜인지 알 수 없다(Database.DefaultPath).
var overridden = Option("--db");
var resolved = overridden is null ? DataLocation.Resolve() : null;
var dbPath = overridden ?? resolved!.DbPath;
if (resolved?.Note is { } locationNote) { Console.WriteLine(locationNote); Console.WriteLine(); }

// 값을 받지 않는 깃발. Positional 이 뒤에 오는 자리 인자를 옵션 값으로 삼키지 않게 알려 준다.
string[] switches = ["--아님", "--해제", "--재평가", "--계열", "--예"];

var database = new Database(dbPath);

// 판올림이 건을 다시 지으며 사람이 이어 둔 접수나 링크를 밀어냈으면 알린다. 오류를 내지 않는
// 자리라 여기서 말하지 않으면 이어 둔 사람은 사라진 줄도 모른다 — 글은 창과 한 벌을 쓴다.
if (NoticeGroupText.Render(database.Migrate()) is { } 건알림)
{
    Console.Write(건알림);
    Console.WriteLine();
}

switch (command)
{
    case "plan": Plan(); break;
    case "status": Status(); break;
    case "현황": 현황(); break;
    case "link": Link(); break;
    case "export": Export(); break;
    case "set": Set(); break;
    case "fix": Fix(); break;
    case "delete": Remove(); break;
    case "submit": Submit(); break;
    case "merge": Merge(); break;
    default: Help(); break;
}

return;

/// <summary>
/// 연간 조달계획 엑셀을 읽어 넣는다.
///
/// <para>접수·공고·계약은 확장과 ERP JSON 으로 들어오고(ADR-028), 계획은 여기 엑셀로 들어온다.
/// 계획은 이미 일어난 일이 아니라 <b>분모</b>라, 접수·공고·계약과 같은 표에 담지 않는다.</para>
///
/// <para>서식은 표본에 고정되어 있다(ADR-023 개정) — 짚을 것이 없으므로 <b>어느 파일인가</b>
/// 하나만 고른다. 인자가 없으면 <b>설정에 적어 둔 엑셀</b>을 다시 읽는다. 인자를 주면 그
/// 파일을 읽고 그 자리를 설정에 적어 둔다 — 창과 같은 파일을 본다.</para>
/// </summary>
void Plan()
{
    var settings = new SettingsStore(database);
    var target = Positional().FirstOrDefault();

    if (target is null)
    {
        target = settings.Read().PlanPath;
        if (target.Length == 0)
        {
            Console.Error.WriteLine("읽을 엑셀을 지정하거나 설정에서 골라 두세요.");
            Environment.Exit(2);
            return;
        }

        Console.WriteLine($"골라 둔 엑셀  {target}\n");
    }

    if (!File.Exists(target)) { Console.Error.WriteLine($"그런 파일이 없습니다: {target}"); Environment.Exit(2); return; }

    var result = new PlanImport(database).Import(target);

    if (!result.Ok)
    {
        // 무엇이 없는지와 무엇이 있었는지를 함께 적는다. 없는 것만 보여 주면
        // 사람은 파일이 통째로 잘못됐다고 여기고 만다.
        Console.Error.WriteLine($"없는 열: {string.Join(", ", result.MissingRequired)}");
        if (result.Found.Count > 0)
            Console.Error.WriteLine($"읽은 열: {string.Join(", ", result.Found)}");

        Console.Error.WriteLine("\n한 줄도 넣지 않았습니다. 첫 줄이 표본의 머리글인지 보세요.");
        Environment.Exit(2);
        return;
    }

    // 넣고 나서 적어 둔다 — 읽지도 못한 파일을 다음번의 기본으로 남기지 않는다.
    settings.SavePlanPath(target);

    Console.WriteLine($"읽은 열: {string.Join(", ", result.Found)}");

    Console.WriteLine(
        $"\n{result.Rows}줄 · 신규 {result.Created} · 갱신 {result.Updated} · 건너뜀 {result.Skipped}" +
        $" · DB {database.Path}");

    if (result.Skipped > 0)
        Console.WriteLine($"  ※ 건너뛴 {result.Skipped}줄은 조달요구번호가 비어 있습니다.");
}

void Status()
{
    var reports = new Reports(database);
    var s = reports.Summary();

    Console.WriteLine($"DB  {database.Path}\n");
    Console.WriteLine($"  접수   {s.RequestRows,4}행 / {s.RequestBases,4}건");
    Console.WriteLine($"  공고   {s.NoticeRows,4}행 / {s.NoticeBases,4}건");
    Console.WriteLine($"  계약   {s.ContractRows,4}행 / {s.ContractBases,4}건");
    Console.WriteLine($"  상대자 {s.Counterparties,4}");
    Console.WriteLine($"  미연결 접수 {s.UnlinkedRequests}건 · 미연결 계약 {s.UnlinkedContracts}건");

    var revisions = reports.Revisions().Where(r => r.Revisions > 1).ToList();
    if (revisions.Count > 0)
    {
        Console.WriteLine("\n  차수가 쌓인 건");
        foreach (var r in revisions)
            Console.WriteLine($"    {r.Kind} {r.Base} · {r.Revisions}차 (최신 {r.Latest}) {r.Title}");
    }
}

/// <summary>
/// 현황을 <b>오는 대로</b> 적는다.
///
/// <para>배치를 지표에 맞춰 짜지 않는다 — 지표는 <c>Status.Metrics</c> 배열 하나에 있고,
/// 거기 줄을 더하는 것만으로 여기에도 화면에도 그 칸이 서야 한다. 여기가 지표를 알기
/// 시작하면 그 성질이 곧바로 무너진다.</para>
///
/// <para><c>status</c> 와 나란히 선다. 저쪽은 <b>표마다의 행 수</b>(쌓기가 제대로 됐는가)이고
/// 이쪽은 <b>계획을 분모로 본 진행</b>이다 — 묻는 것이 달라 한 명령으로 합치지 않는다.</para>
/// </summary>
void 현황()
{
    var report = new Pclm.Core.Storage.Status(database).Report();

    Console.WriteLine($"DB  {database.Path}");
    Console.WriteLine();

    foreach (var group in report.Groups)
    {
        Console.WriteLine($"  {group.이름}");

        foreach (var cell in group.Cells)
        {
            // 거르개가 있으면 창의 계획 탭에서 눌러 볼 수 있는 줄이다. 알아보게만 적는다.
            var mark = cell.거르개 is null ? "" : $"   ▸ {cell.거르개}";
            Console.WriteLine($"    {cell.이름,-10} {cell.수,6}{mark}");
        }

        Console.WriteLine();
    }
}

void Link()
{
    var linker = new Linker(database);
    var requestLinker = new RequestLinker(database);
    var rest = Positional();

    if (Flag("--재평가"))
    {
        Console.WriteLine($"명시 참조 연결 {ExplicitLinks.Resolve(database)}건 · 기존 링크 유지");
        return;
    }

    if (Flag("--해제"))
    {
        if (FindLinkable(rest.FirstOrDefault()) is not { } target) { Environment.Exit(2); return; }

        if (target.EntityType == "request") requestLinker.Unlink(target);
        else linker.Unlink(target);

        Console.WriteLine($"해제  {target.Display}");
        return;
    }

    if (Flag("--아님"))
    {
        if (FindLinkable(rest.FirstOrDefault()) is not { } rejected) { Environment.Exit(2); return; }
        if (Find(rest.Skip(1).FirstOrDefault(), "notice") is not { } from) { Environment.Exit(2); return; }

        if (rejected.EntityType == "request") requestLinker.Reject(rejected, from);
        else linker.Reject(rejected, from);

        Console.WriteLine($"아님  {rejected.Display}  ↮  {from.Display}");
        return;
    }

    if (rest.Count >= 2)
    {
        // 왼쪽이 무엇이냐로 길이 갈린다. 접수번호와 계약번호는 Resolve 가 가려 준다.
        if (FindLinkable(rest[0]) is not { } left) { Environment.Exit(2); return; }
        if (Find(rest[1], "notice") is not { } notice) { Environment.Exit(2); return; }

        if (left.EntityType == "request") requestLinker.Confirm(left, notice, 1.0);
        else linker.Confirm(left, notice, 1.0);

        Console.WriteLine($"확정  {left.Display}  ←  {notice.Display}");
        return;
    }

    Console.WriteLine($"명시 참조 연결 {ExplicitLinks.Resolve(database)}건");

    var requestCandidates = requestLinker.Candidates();
    var candidates = linker.Candidates();

    if (requestCandidates.Count == 0 && candidates.Count == 0)
    {
        Console.WriteLine("사람이 볼 후보가 없습니다.");
        return;
    }

    if (requestCandidates.Count > 0)
    {
        Console.WriteLine("접수 ↔ 공고 (확정: pclm link <접수번호|요청번호> <공고번호> · 물리치기: --아님)\n");
        foreach (var group in requestCandidates.GroupBy(c => c.Request.Display))
        {
            Console.WriteLine($"  {group.Key}  {group.First().RequestTitle}");
            foreach (var c in group.Take(3))
            {
                var linked = c.NoticeLinkedCount > 0 ? " · 이미 접수가 붙어 있음" : "";
                Console.WriteLine($"      품목  {c.Confidence,5:P0}  {c.Notice.Display}  {c.NoticeTitle}   ({c.Reason}{linked})");
                if (c.Blocker is not null) Console.WriteLine($"              └ 자동으로 잇지 않은 까닭: {c.Blocker}");
            }
            Console.WriteLine();
        }
    }

    if (candidates.Count == 0) return;

    Console.WriteLine("계약 ↔ 공고 (확정: pclm link <계약번호> <공고번호> · 물리치기: --아님)\n");
    foreach (var group in candidates.GroupBy(c => c.Contract.Display))
    {
        Console.WriteLine($"  {group.Key}  {group.First().ContractTitle}");
        foreach (var c in group.Take(3))
        {
            // 이미 다른 계약이 붙어 있어도 후보에서 빼지 않는다 — 한 공고에 계약이 여럿일 수 있다.
            var linked = c.NoticeContractCount > 0 ? $" · 이미 {c.NoticeContractCount}건 연결됨" : "";
            var mark = c.TitleMatched ? "건명" : "약함";

            Console.WriteLine($"      {mark}  {c.Confidence,5:P0}  {c.Notice.Display}  {c.NoticeTitle}   ({c.Reason}{linked})");
            if (c.Blocker is not null) Console.WriteLine($"              └ 자동으로 잇지 않은 까닭: {c.Blocker}");
        }
        Console.WriteLine();
    }
}

/// <summary>기계가 스스로 이은 접수를 알린다. 근거는 수량·단가 다중집합이다.</summary>
void ReportRequests(AutoRequestLinkResult result)
{
    foreach (var link in result.Linked)
        Console.WriteLine($"  자동  {link.Request.Display}  →  {link.Notice.Display}   ({link.Evidence})");

    if (result.Count > 0) Console.WriteLine($"\n접수 {result.Count}건을 자동으로 이었습니다.\n");
}

/// <summary>
/// 잇는 쪽이 될 수 있는 것 — <b>계약이거나 접수</b>. 공고는 언제나 이어지는 쪽이라 여기 오지 않는다.
/// </summary>
EntityRef? FindLinkable(string? key)
{
    var entity = key is null ? null : new Store(database).Resolve(key);
    if (entity?.EntityType is "contract" or "request") return entity;

    Console.Error.WriteLine($"그런 계약·접수가 없습니다: {key ?? "(빠짐)"}");
    return null;
}

/// <summary>기계가 스스로 이은 것을 알린다. 무엇을 근거로 삼았는지 함께 적는다.</summary>
void Report(AutoLinkResult result)
{
    foreach (var link in result.Linked)
        Console.WriteLine($"  자동  {link.Contract.Display}  ←  {link.Notice.Display}   ({link.Evidence})");

    if (result.Count > 0) Console.WriteLine($"\n자동으로 {result.Count}건 이었습니다.\n");
}

/// <summary>사람이 친 키를 실제 행으로 옮긴다. 없으면 까닭을 적고 null.</summary>
EntityRef? Find(string? key, string entityType)
{
    var entity = key is null ? null : new Store(database).Resolve(key);
    if (entity?.EntityType == entityType) return entity;

    Console.Error.WriteLine(entityType == "contract"
        ? $"그런 계약이 없습니다: {key ?? "(빠짐)"}"
        : $"그런 공고가 없습니다: {key ?? "(빠짐)"}");

    return null;
}

void Export()
{
    // 자리를 짚지 않으면 지금 폴더에 떨어뜨린다 — 앱 데이터 폴더에 조용히 넣으면
    // 어디로 갔는지 모르는 사진이 쌓인다.
    var target = Positional().FirstOrDefault() ?? Exporter.DefaultFileName();

    var saved = new Exporter(database).Export(target);
    Console.WriteLine($"내보냄  {saved}");
    Console.WriteLine($"시트    {string.Join(" · ", Views.Exported.Select(Views.SheetName))}");
}

/// <summary>
/// 내 자료 한 벌을 <b>제출본 파일</b>로 뜬다.
///
/// <para>담당자마다 자기 DB 가 하나씩 생겼는데 그것들을 모을 길이 없었다. 웹ERP 에 닿을 수
/// 없어 시작된 저장소라 취합도 파일로 한다 — 이쪽이 내는 것을 <c>pclm merge</c> 가 받는다.</para>
///
/// <para><c>--이름</c> 은 <b>파일 이름에만</b> 쓴다. 자료의 임자를 정하는 값이 아니다(ADR-023).</para>
/// </summary>
void Submit()
{
    var settings = new SettingsStore(database);
    var current = settings.Read();

    // 이름을 주면 설정에 남긴다 — 다음부터 말이 없다.
    if (Option("--이름") is { } given)
        current = settings.Save(submitterName: given);

    var name = current.SubmitterName;
    var stamp = DateTime.Now.ToString("yyyyMMdd");
    var fileName = name.Length == 0 ? $"제출_{stamp}.pclm" : $"제출_{name}_{stamp}.pclm";

    // 자리를 짚지 않으면 지금 폴더에 떨어뜨린다 — 앱 데이터 폴더에 조용히 넣으면
    // 어디로 갔는지 모르는 파일이 쌓인다(export 와 같은 자세).
    var target = Positional().FirstOrDefault();
    var path = target is null ? fileName
        : Directory.Exists(target) ? Path.Combine(target, fileName)
        : target;

    var saved = database.Snapshot(path, overwrite: true);

    Console.WriteLine($"제출본  {saved}");
    Console.WriteLine($"        {new FileInfo(saved).Length / 1024d / 1024d:0.#}MB · DB {database.Path}");

    if (name.Length == 0)
        Console.WriteLine(
            "\n  ※ 이름이 비어 있어 파일 이름에 넣지 못했습니다.\n" +
            "     pclm submit --이름 <이름> 으로 정합니다 — 제출본 파일 이름에만 쓰는 값입니다.");

    Console.WriteLine(
        "\n  알맹이는 SQLite 이지만 확장자는 .pclm 입니다 — 메일 게이트웨이가 .db 를 막는 일이 있어서입니다.\n" +
        "  그것마저 막히면 압축해서 보내세요.");
}

/// <summary>
/// 제출본을 모아 <b>취합본 하나</b>로 짓는다.
///
/// <para>기계는 고르기만 하고 판정하지 않으므로 <b>알림이 본체다</b> — 무엇이 누구와 누구에게서
/// 왔고 어느 것을 실었는지 한 줄씩 적는다.</para>
/// </summary>
void Merge()
{
    var 취합본 = Option("--out") ?? $"취합_{DateTime.Now:yyyyMMdd}.pclm";
    if (Directory.Exists(취합본)) 취합본 = Path.Combine(취합본, $"취합_{DateTime.Now:yyyyMMdd}.pclm");
    취합본 = Path.GetFullPath(취합본);

    var found = new List<string>();
    var 못찾음 = new List<string>();

    foreach (var target in Positional())
    {
        if (Directory.Exists(target))
        {
            var picked = Merger.FindSubmissions(target);
            if (picked.Count == 0) 못찾음.Add($"{target} — 이 폴더에 .pclm 이 없습니다");
            found.AddRange(picked);
        }
        else if (File.Exists(target)) found.Add(target);
        else 못찾음.Add($"{target} — 그런 폴더도 파일도 없습니다");
    }

    // 지난 취합본이 그 폴더에 있어도 제출본으로 세지 않는다.
    var files = found
        .Select(Path.GetFullPath)
        .Where(f => !string.Equals(f, 취합본, StringComparison.OrdinalIgnoreCase))
        .Distinct(StringComparer.OrdinalIgnoreCase)
        .Order(StringComparer.OrdinalIgnoreCase)
        .ToList();

    if (files.Count == 0)
    {
        // 조용히 빈 것을 만들지 않는다 — 받는 사람은 그것을 취합 결과로 안다.
        Console.Error.WriteLine("합칠 제출본을 찾지 못했습니다.");
        foreach (var m in 못찾음) Console.Error.WriteLine($"  {m}");
        if (Positional().Count == 0) Console.Error.WriteLine("  pclm merge <폴더|파일…> [--out <경로>]");
        Console.Error.WriteLine("\n제출본은 pclm submit 이 뜬 .pclm 파일입니다.");
        Environment.Exit(2);
        return;
    }

    foreach (var m in 못찾음) Console.WriteLine($"※ {m}");

    var report = new Merger().Merge(files, 취합본);

    // 글은 Core 가 짓는다 — 창이 취합본 옆에 남기는 것과 한 글자도 다르면 안 된다.
    Console.Write(MergeReportText.Render(report, 취합본));
}

void Set()
{
    var store = new Store(database);
    var rest = Positional();

    if (rest.Count == 0)
    {
        Console.WriteLine("사람이 채우는 열\n");
        foreach (var group in store.UserColumns().GroupBy(c => c.EntityType))
        {
            Console.WriteLine($"  [{(group.Key == "contract" ? "계약" : "공고")}]");
            foreach (var c in group)
                Console.WriteLine($"    {c.FieldName,-12} {c.Kind,-7} {string.Join(" / ", c.Choices)}");
        }
        Console.WriteLine("\n  넣기: pclm set <키> <열> <값>   (값을 비우면 지운다)");
        return;
    }

    if (rest.Count < 2) { Console.Error.WriteLine("pclm set <키> <열> [값]"); Environment.Exit(2); return; }

    var key = rest[0];
    var entity = store.Resolve(key);
    if (entity is null) { Console.Error.WriteLine($"그런 공고·계약이 없습니다: {key}"); Environment.Exit(2); return; }

    var entityType = entity.EntityType;
    var field = rest[1];
    var known = store.UserColumns(entityType).Select(c => c.FieldName).ToList();
    if (!known.Contains(field))
    {
        Console.Error.WriteLine($"모르는 열입니다: {field}");
        Console.Error.WriteLine($"쓸 수 있는 열: {string.Join(", ", known)}");
        Environment.Exit(2);
        return;
    }

    var value = rest.Count >= 3 ? string.Join(" ", rest.Skip(2)) : null;

    // 후보에 없는 값도 넣게 두되 조용히 넘기지는 않는다 — 오타인지 새 값인지는 사람이 안다.
    var definition = store.UserColumns(entityType).First(c => c.FieldName == field);
    if (value is not null && definition.Choices.Count > 0 && !definition.Choices.Contains(value))
        Console.WriteLine($"  ※ 후보에 없는 값입니다. 후보: {string.Join(" / ", definition.Choices)}");

    store.SetUserField(entity, field, value);
    Console.WriteLine(value is null ? $"지움  {entity.Display}.{field}" : $"넣음  {entity.Display}.{field} = {value}");
}

/// <summary>
/// 수집한 값을 손으로 고친다.
///
/// <para><c>set</c> 과 길을 가른다 — 저쪽은 사람이 세운 열(진행상태·메모)이고 이쪽은
/// <b>수집한 칸의 정정</b>이다. 담기는 표가 다르다.</para>
///
/// <para>값을 아예 주지 않으면 <b>되돌린다</b>. 칸을 <b>비우려면</b> 빈 값을 준다
/// (<c>pclm fix 키 열 ""</c>) — 잘못 읽힌 값을 지우는 것과 수집한 값으로 돌아가는 것은 다른 일이다.</para>
/// </summary>
void Fix()
{
    var store = new Store(database);
    var rest = Positional();

    if (rest.Count < 2)
    {
        Console.Error.WriteLine("pclm fix <키> <열> [값]     (값이 없으면 원래대로 되돌린다)");
        Environment.Exit(2);
        return;
    }

    var key = rest[0];
    var entity = store.Resolve(key);
    if (entity is null) { Console.Error.WriteLine($"그런 공고·계약이 없습니다: {key}"); Environment.Exit(2); return; }

    var column = rest[1];

    try
    {
        store.CheckCorrectable(entity, column);
    }
    catch (InvalidOperationException ex)
    {
        Console.Error.WriteLine(ex.Message);
        Console.Error.WriteLine($"쓸 수 있는 열: {Store.FaceView(entity.EntityType)} 의 열 (키 열과 사람 열은 뺀다)");
        Environment.Exit(2);
        return;
    }

    var was = store.FaceValue(entity, column);

    if (rest.Count < 3)
    {
        store.ClearOverride(entity, column);
        Console.WriteLine($"되돌림  {entity.Display}.{column}");
        Console.WriteLine($"  {Shown(was)}  →  {Shown(store.FaceValue(entity, column))}");
        return;
    }

    var value = string.Join(" ", rest.Skip(2));
    store.SetOverride(entity, column, value);

    Console.WriteLine($"고침  {entity.Display}.{column}");
    Console.WriteLine($"  {Shown(was)}  →  {Shown(value)}");
}

/// <summary>
/// 레코드를 지운다. <b>세어 보이고, <c>--예</c> 없이는 지우지 않는다</b> — 되돌릴 수 없다.
/// </summary>
void Remove()
{
    var store = new Store(database);

    if (Positional().FirstOrDefault() is not { } key)
    {
        Console.Error.WriteLine("pclm delete <키> [--계열] [--예]");
        Environment.Exit(2);
        return;
    }

    var entity = store.Resolve(key);
    if (entity is null) { Console.Error.WriteLine($"그런 공고·계약이 없습니다: {key}"); Environment.Exit(2); return; }

    var whole = Flag("--계열");
    var plan = store.PlanDeletion(entity, whole);

    Console.WriteLine($"{(plan.EntityType == "notice" ? "공고" : "계약")}  {plan.Display}  {plan.Title}\n");
    Console.WriteLine($"  쌓인 차수   {string.Join(" · ", plan.Revisions)}");
    Console.WriteLine($"  지울 것     {(whole ? "전 차수" : $"{entity.Seq} 차수 하나")}");
    Console.WriteLine($"  딸린 줄     {plan.ChildRows}");
    Console.WriteLine($"  손으로 고친 칸 {plan.Overrides}");

    if (plan.SeriesGoes)
    {
        Console.WriteLine($"  사람이 적은 값 {plan.UserFields}   ← 계열이 통째로 걷힌다");
        if (plan.Linked) Console.WriteLine("  확정한 링크도 함께 풀린다");
    }

    if (!Flag("--예"))
    {
        Console.WriteLine("\n지우려면 --예 를 붙이세요. 되돌릴 수 없습니다.");
        return;
    }

    store.Delete(entity, whole);
    Console.WriteLine($"\n지움  {plan.Display}");
}

/// <summary>빈 값은 눈에 보여야 한다. 아무것도 찍히지 않으면 지워진 것인지 알 수 없다.</summary>
string Shown(string value) => value.Length == 0 ? "(빔)" : value;

void Help() => Console.WriteLine("""
    pclm plan [엑셀]                    연간 조달계획을 읽어 넣는다 (조달요구번호 기준 업서트)
                                        인자가 없으면 지난번에 고른 엑셀을 다시 읽는다
    pclm status                         쌓인 상태를 본다
    pclm 현황                            계획을 분모로 본 진행 (지금 지표는 가상이다)
    pclm link [키 공고번호]              공고에 계약·접수를 잇는다 (인자 없으면 자동 확정 + 후보)
      --아님 <키> <공고번호>            이 짝은 아니라고 판정한다 (다시 후보로 오르지 않는다)
      --해제 <키>                       링크를 푼다
      --재평가                          기계가 이은 것만 풀고 규칙을 다시 적용한다
    pclm export [경로]                  계약면을 엑셀로 내보낸다 (그 시점의 사진)
    pclm submit [경로]                  내 자료 한 벌을 제출본 파일로 뜬다 (.pclm)
      --이름 <이름>                     제출본 파일 이름에 쓸 내 이름을 정해 둔다
    pclm merge <폴더|파일…>              제출본을 모아 취합본 하나로 짓는다 (매번 처음부터)
      --out <경로>                      취합본을 놓을 자리 (기본은 지금 폴더)
    pclm set [키 열 값]                 손으로 채우는 값 (인자 없으면 열 목록)
    pclm fix <키> <열> [값]             수집한 값을 고친다 (값 없으면 원래대로)
    pclm delete <키>                    레코드를 지운다 (무엇이 사라지는지 세어 보인다)
      --계열                            그 본번호의 전 차수를 지운다
      --예                              실제로 지운다 (없으면 세어 보이기만 한다)
      --db <경로>                       다른 자료를 연다 (기본은 창과 같은 자리)

    <키> 는 계약번호·공고번호·접수번호다. 접수는 아무 요청번호로도 찾는다.
    쌓이는 자리는 pclm status 가 첫 줄에 적는다.
    """);

string? Option(string name)
{
    var i = Array.IndexOf(args, name);
    return i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
}

bool Flag(string name) => Array.IndexOf(args, name) >= 0;

// 명령 뒤의 자리 인자만. --옵션과 그 뒤에 붙는 값을 함께 건너뛴다 —
// 건너뛰지 않으면 "set 키 열 값 --db 경로" 에서 경로가 값에 딸려 들어간다.
// 다만 값을 받지 않는 깃발은 저 혼자 건너뛴다. 함께 건너뛰면
// "link --아님 계약 공고" 에서 계약번호가 깃발의 값으로 사라진다.
List<string> Positional()
{
    var result = new List<string>();

    for (var i = 1; i < args.Length; i++)
    {
        if (!args[i].StartsWith("--")) { result.Add(args[i]); continue; }
        if (switches.Contains(args[i])) continue;
        i++; // 옵션의 값
    }

    return result;
}
