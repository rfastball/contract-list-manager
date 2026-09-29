using Pclm.Core.Erp;
using System.IO;
using System.Reflection;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Web.WebView2.Core;
using Pclm.Core.Storage;

namespace Pclm.App;

/// <summary>
/// 폴더 고르기 요청. 제목과 처음 열 자리를 <b>부르는 쪽</b>이 정한다.
///
/// <para>고르기 창을 여는 것은 창(WPF)이지만 무엇을 고르라는 것인지 아는 것은 다리다 —
/// 자료를 옮길 폴더인지 제출본이 든 폴더인지에 따라 제목이 달라야 한다.</para>
/// </summary>
public sealed record FolderPrompt(string Title, string? Initial);

/// <summary>저장 자리 고르기 요청. 제목·거르개·확장자·지어 둔 이름을 부르는 쪽이 정한다.</summary>
public sealed record SavePrompt(string Title, string Filter, string DefaultExt, string FileName);

/// <summary>
/// 화면이 앱에 일을 시키는 유일한 통로.
///
/// <para>화면은 <c>{id, method, args}</c> 를 보내고 여기서 <c>{id, ok, result|error}</c> 로 답한다.
/// 번호로 짝을 맞추므로 여러 요청이 겹쳐도 섞이지 않는다.</para>
///
/// <para><b>화면에 SQL 을 열어 주지 않는다.</b> 부를 수 있는 일은 아래 <c>switch</c> 에 적힌 것뿐이고,
/// 표 이름도 계약면 뷰 목록에 있는 것만 받는다 — 화면이 내부 표를 직접 헤집지 못하게.</para>
/// </summary>
public sealed class Bridge(Database database)
{
    public bool AllowDataMove { get; init; } = true;
    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.Never,
    };

    private readonly Database _database = database;
    private readonly Store _store = new(database);
    private readonly SettingsStore _settings = new(database);

    /// <summary>폴더 고르기 창을 연다. 그만두면 <c>null</c>.</summary>
    public Func<FolderPrompt, string?>? PickFolder { get; set; }

    /// <summary>저장할 자리를 고르게 한다. 지어 둔 이름을 건네고, 그만두면 <c>null</c>.</summary>
    public Func<SavePrompt, string?>? PickSavePath { get; set; }

    /// <summary>계획 엑셀 고르기 창을 연다. 그만두면 <c>null</c>.</summary>
    public Func<string?>? PickPlanExcel { get; set; }

    public async Task HandleAsync(CoreWebView2 core, string messageJson)
    {
        var id = 0;

        try
        {
            using var document = JsonDocument.Parse(messageJson);
            var root = document.RootElement;

            id = root.GetProperty("id").GetInt32();
            var method = root.GetProperty("method").GetString() ?? "";
            var args = root.TryGetProperty("args", out var a)
                ? a.EnumerateArray().Select(x => x.ValueKind == JsonValueKind.Null ? null : x.GetString()).ToArray()
                : [];

            var result = await Task.Run(() => Invoke(method, args));
            Reply(core, id, true, result, null);
        }
        catch (Exception ex)
        {
            Reply(core, id, false, null, ex.Message);
        }
    }

    private object? Invoke(string method, string?[] args) => method switch
    {
        "prepareExtension" => PrepareExtension(),
        "extensionStatus" => ReadExtensionStatus(),
        "openExtensionSetup" => ExtensionSetup.Open(Require(args, 0)),
        "erpTools" => ErpTools.Invoke(_database, Require(args, 0), Require(args, 1)),
        "status" => new Reports(_database).Summary(),
        "현황" => new Pclm.Core.Storage.Status(_database).Report(),
        "sheet" => ReadSheet(Require(args, 0)),
        "outline" => ReadOutline(),
        "userColumns" => UserColumns(args.ElementAtOrDefault(0)),
        "addColumn" => AddColumn(Require(args, 0), Require(args, 1), Require(args, 2), args.ElementAtOrDefault(3)),
        "updateColumn" => UpdateColumn(Require(args, 0), Require(args, 1), Require(args, 2), Require(args, 3), args.ElementAtOrDefault(4)),
        "removeColumn" => RemoveColumn(Require(args, 0), Require(args, 1)),
        "moveColumn" => MoveColumn(Require(args, 0), Require(args, 1), Require(args, 2)),
        "setField" => SetField(Require(args, 0), Require(args, 1), args.ElementAtOrDefault(2)),
        "setOverride" => SetOverride(Require(args, 0), Require(args, 1), args.ElementAtOrDefault(2)),
        "clearOverride" => ClearOverride(Require(args, 0), Require(args, 1)),
        "deletionPlan" => Plan(Require(args, 0), Require(args, 1)),
        "deleteEntity" => Delete(Require(args, 0), Require(args, 1)),
        "settings" => ReadSettings(),
        "saveSettings" => SaveSettings(args.ElementAtOrDefault(0)),
        "linkCandidates" => LinkCandidates(),
        "relink" => Relink(),
        "requestLinkCandidates" => RequestLinkCandidates(),
        "compareRequestLink" => CompareRequestLink(Require(args, 0), Require(args, 1)),
        "confirmRequestLink" => ConfirmRequestLink(Require(args, 0), Require(args, 1)),
        "rejectRequestLink" => RejectRequestLink(Require(args, 0), Require(args, 1)),
        "unlinkRequest" => UnlinkRequest(Require(args, 0)),
        "compareLink" => CompareLink(Require(args, 0), Require(args, 1)),
        "confirmLink" => ConfirmLink(Require(args, 0), Require(args, 1)),
        "rejectLink" => RejectLink(Require(args, 0), Require(args, 1)),
        "unlink" => Unlink(Require(args, 0)),
        "export" => Export(),
        "submit" => Submit(),
        "merge" => MergeSubmissions(),
        "pickPlanExcel" => PickPlanFile(),
        "importPlan" => ImportPlan(),
        "dataLocation" => DataLocationInfo(),
        "moveDataLocation" => MoveDataLocation(),
        "revealDataFolder" => RevealDataFolder(),
        "about" => About(),
        _ => throw new InvalidOperationException($"모르는 요청입니다: {method}"),
    };

    /// <summary>지금 어느 자료를 열고 있는지. 창이 자기 자리를 말하지 않으면 사람은 알 길이 없다.</summary>
    private object DataLocationInfo()
    {
        var file = new FileInfo(_database.Path);
        return new
        {
            path = _database.Path,
            folder = System.IO.Path.GetDirectoryName(_database.Path),
            sizeBytes = file.Exists ? file.Length : 0L,
            isDefault = string.Equals(
                System.IO.Path.GetFullPath(_database.Path),
                System.IO.Path.GetFullPath(Database.DefaultPath),
                StringComparison.OrdinalIgnoreCase),
            configPath = AllowDataMove ? DataLocation.ConfigPath : "개발 실행: 자료 위치 고정",
        };
    }

    /// <summary>
    /// 자료를 옮길 자리를 고른다. <b>여기서는 적어 두기만 하고 옮기지 않는다</b> —
    /// 옮기는 것은 다음 실행 맨 앞이다(<see cref="DataLocation"/>).
    /// </summary>
    private object? MoveDataLocation()
    {
        if (!AllowDataMove)
            throw new InvalidOperationException("개발 실행에서는 자료 위치를 옮길 수 없습니다.");
        var picked = PickFolder?.Invoke(
            new FolderPrompt("자료를 옮길 폴더 고르기", System.IO.Path.GetDirectoryName(_database.Path)));
        if (string.IsNullOrEmpty(picked)) return null;

        // 사람은 자리만 가리키고, 우리 폴더는 우리가 만든다. 막을 것을 막는 일은 확장 호스트와 한 길이다.
        return new { folder = DataLocation.StageMoveUnder(_database.Path, picked) };
    }

    /// <summary>
    /// 판과 라이선스. 배포물은 exe 하나라 고지를 옆에 둘 수 없어 안에 박아 두고 여기서 꺼낸다.
    /// 판은 Directory.Build.props 의 것이다 — 정보 판에 붙는 <c>+커밋</c> 은 떼어 낸다.
    /// </summary>
    private static object About()
    {
        var assembly = typeof(Bridge).Assembly;
        var version = assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "";
        return new
        {
            version = version.Split('+')[0],
            license = EmbeddedText(assembly, "LICENSE"),
            notices = EmbeddedText(assembly, "THIRD-PARTY-NOTICES.txt"),
        };
    }

    private static string EmbeddedText(Assembly assembly, string name)
    {
        using var stream = assembly.GetManifestResourceStream(name)
            ?? throw new InvalidOperationException($"exe 안에 {name} 이 없습니다.");
        using var reader = new StreamReader(stream, Encoding.UTF8);
        return reader.ReadToEnd();
    }

    private object PrepareExtension()
    {
        RequireProductionExtension();
        ErpConnection.Prepare(_database);
        return ExtensionSetup.Prepare();
    }

    private object ReadExtensionStatus()
    {
        RequireProductionExtension();
        return ExtensionStatus.Read(ErpConnection.DirectoryPath, BundledExtension.EmbeddedVersion());
    }

    private void RequireProductionExtension()
    {
        if (!AllowDataMove)
            throw new InvalidOperationException("내장 확장 연결은 업무 DB용입니다. --db 또는 --erp-dev 없이 앱을 다시 실행하세요.");
    }

    private object RevealDataFolder()
    {
        var folder = System.IO.Path.GetDirectoryName(_database.Path)!;
        Directory.CreateDirectory(folder);
        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(folder) { UseShellExecute = true });
        return new { folder };
    }

    private static string Require(string?[] args, int index) =>
        args.ElementAtOrDefault(index) ?? throw new ArgumentException($"{index + 1}번째 값이 없습니다.");

    /// <summary>
    /// 뷰 하나를 통째로 읽어 준다. 이름이 계약면 목록에 있는지 먼저 보므로 아무 표나 열 수 없다.
    /// </summary>
    private object ReadSheet(string view)
    {
        if (!Views.Names.Contains(view))
            throw new InvalidOperationException($"내보이지 않는 표입니다: {view}");

        // 이름으로 종류를 가리는 규칙은 Views 한 군데에 둔다. 여기서 다시 짜면
        // 접수가 들어오는 순간 v_접수_v1 이 계약으로 읽혀 고친 값이 엉뚱한 표에 담긴다.
        var entityType = Views.EntityTypeOf(view);

        // 고칠 수 없는 표인가. 목록은 Views 한 군데에 둔다 — 여기서 조건을 다시 짜면
        // 목록이 두 벌이 되고, 갈리는 순간 고칠 수 없어야 할 표가 조용히 열린다.
        //
        // 계획은 plan 이 개체가 아니라서고(field_override·user_column 의 entity_type 에 자리가
        // 없다), 차수 뷰 둘과 통합 v3 은 Store.FaceView 가 종류당 뷰를 하나만 알기 때문이다 —
        // 옛 차수의 칸을 고치면 덮개의 original 을 그 뷰에서 찾지 못해 빈 문자열로 박히고,
        // "무엇을 무엇으로 고쳤나" 가 오류 없이 거짓이 된다. 고치는 자리는 공고 탭·계약 탭이다.
        var 읽기전용 = Views.ReadOnly.Contains(view);

        var editable = 읽기전용
            ? new HashSet<string>()
            : _store.UserColumns(entityType).Select(c => c.FieldName).ToHashSet();

        // 통합의 공고·접수 열은 이어진 다른 레코드의 것이라 이 줄의 키로는 주소가 잡히지 않는다.
        // 공고·접수 시트에서 고치면 같은 뷰를 타므로 여기에도 그대로 비친다.
        //
        // <b>v3 은 정반대다.</b> 저쪽 줄은 계약이라 공고·접수를 잠갔지만, v3 의 줄은 공고라
        // 계약·접수 쪽이 이어진 남의 레코드다 — UnifiedNoticeColumns 를 재활용할 수 없다.
        // 지금은 v3 이 통째로 읽기 전용이라 실효가 없지만, 두 목록이 갈린 채로 두면 나중에
        // 편집을 켜는 날 아무 오류 없이 샌다.
        HashSet<string> elsewhere = view switch
        {
            "v_통합_v1" => [.. Views.UnifiedNoticeColumns],
            "v_통합_v2" => [.. Views.UnifiedNoticeColumns, .. Views.UnifiedRequestColumns],
            "v_통합_v3" => [.. Views.ContractColumns, .. Views.UnifiedRequestColumns],
            _ => [],
        };

        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT * FROM \"{view}\";";

        using var reader = command.ExecuteReader();
        var columns = Enumerable.Range(0, reader.FieldCount).Select(reader.GetName).ToList();
        var rows = new List<Dictionary<string, string>>();

        while (reader.Read())
        {
            var row = new Dictionary<string, string>(columns.Count);
            for (var i = 0; i < columns.Count; i++)
                row[columns[i]] = reader.IsDBNull(i) ? "" : reader.GetValue(i).ToString() ?? "";
            rows.Add(row);
        }

        return new
        {
            name = view,
            columns,
            editable = columns.Where(editable.Contains).ToList(),
            correctable = 읽기전용
                ? new List<string>()
                : columns
                    .Where(c => !Views.KeyColumns.Contains(c)
                                && !editable.Contains(c)
                                && !elsewhere.Contains(c))
                    .ToList(),
            overrides = 읽기전용 ? [] : Overrides(entityType),
            rows,
        };
    }

    /// <summary>
    /// 지금 걸려 있는 덮개를 <b>줄 키로 묶어</b> 낸다 — 화면이 어느 칸이 손으로 고쳐졌는지
    /// 표시하는 데 쓴다. 값은 고치기 전에 그 자리에 있던 것이다.
    ///
    /// <para>최신이 아닌 차수에 걸린 덮개도 함께 온다. 이 함수를 부르는 표는 모두 최신 차수만
    /// 내므로(<c>v_공고_v1</c>·<c>v_계약_v1</c> 과 그 위에 얹힌 통합 v1·v2) 그 키를 가진 줄이
    /// 없어 화면이 찾지 못하고 지나간다 — 걸러 낼 것이 없다.</para>
    ///
    /// <para><b>"그런 줄은 없다" 가 이제 표 전체에 대한 말은 아니다.</b> 차수 뷰
    /// (<c>v_공고차수_v1</c>·<c>v_계약차수_v1</c>·<c>v_통합_v3</c>)는 옛 차수의 줄을 세운다.
    /// 다만 그 셋은 읽기 전용이라(<c>Views.ReadOnly</c>) 이 함수를 <b>부르지 않고</b>, 그래서
    /// 옛 차수의 덮개는 거기에도 뜨지 않는다 — 표시되지 않는 까닭이 "줄이 없어서" 에서
    /// "고칠 수 있는 표가 아니라서" 로 바뀐 것이다. 고치고 되돌리는 자리는 공고 탭·계약 탭이다.</para>
    /// </summary>
    private Dictionary<string, Dictionary<string, string>> Overrides(string entityType)
    {
        var byRow = new Dictionary<string, Dictionary<string, string>>();

        foreach (var o in _store.Overrides(entityType))
        {
            var display = new EntityRef(entityType, o.Base, o.Seq).Display;

            if (!byRow.TryGetValue(display, out var cells))
                byRow[display] = cells = [];

            cells[o.ColumnName] = o.Original;
        }

        return byRow;
    }

    /// <summary>
    /// 쌓인 것을 생긴 모양대로 — <b>접수 → 공고 → 계약</b> 사슬.
    ///
    /// <para>평평한 표로는 이 자료를 제대로 낼 수 없다. 계약 하나에 명세가 여러 줄이라
    /// 펴면 위쪽 값이 아래 줄마다 되풀이되고, 보는 사람에게는 <b>같은 것이 여러 번 들어간
    /// 것처럼</b> 보인다.</para>
    ///
    /// <para>옛 <c>loose</c> 는 없앴다. 매달리지 못한 계약도 <b>제 사슬</b>로 서므로 따로
    /// 낼 자리가 필요 없다 — 접수만 있는 것·공고만 있는 것과 같은 모양으로 다룬다.</para>
    /// </summary>
    private object ReadOutline()
    {
        var tree = new Outline(_database).Build();

        static object Contract(OutlineContract c) => new
        {
            key = c.Key,
            number = c.Number,
            title = c.Title,
            contractedOn = c.ContractedOn,
            amount = c.Amount,
            counterparty = c.Counterparty,
            demandAgency = c.DemandAgency,
            revisions = c.Revisions,
            items = c.Items.Select(i => new
            {
                lineNo = i.LineNo,
                name = i.Name,
                specification = i.Specification,
                quantity = i.Quantity,
                unit = i.Unit,
                unitPrice = i.UnitPrice,
                amount = i.Amount,
            }).ToList(),
        };

        static object? Request(OutlineRequest? r) => r is null ? null : new
        {
            key = r.Key,
            number = r.Number,
            title = r.Title,
            receivedOn = r.ReceivedOn,
            goodsAmount = r.GoodsAmount,
            budgetAmount = r.BudgetAmount,
            demandAgency = r.DemandAgency,
            requestNumbers = r.RequestNumbers,
            revisions = r.Revisions,
        };

        static object? Notice(OutlineNotice? n) => n is null ? null : new
        {
            key = n.Key,
            number = n.Number,
            title = n.Title,
            postedAt = n.PostedAt,
            agency = n.Agency,
            estimatedPrice = n.EstimatedPrice,
            revisions = n.Revisions,
        };

        // 가운데 칸은 공고 하나가 아니라 <b>건</b> 하나다. 대체된 것도 함께 건네는 것은,
        // 화면이 그 건의 장수를 세어야 취소·재공고로 갈린 것이 한 줄로 접힌 줄 알아보기
        // 때문이다 — 현행만 건네면 지나간 공고가 화면에서 통째로 없던 일이 된다.
        static object? Group(OutlineNoticeGroup? g) => g is null ? null : new
        {
            groupBase = g.GroupBase,
            current = Notice(g.Current),
            superseded = g.Superseded.Select(n => Notice(n)).ToList(),
        };

        return new
        {
            chains = tree.Chains.Select(c => new
            {
                key = c.Key,
                request = Request(c.Request),
                notice = Group(c.Notice),
                contracts = c.Contracts.Select(Contract).ToList(),
            }).ToList(),
        };
    }

    /// <summary>종류를 주지 않으면 계약·공고 것을 모두 낸다 — 열 고치는 화면이 둘 다 쓴다.</summary>
    private object UserColumns(string? entityType) =>
        _store.UserColumns(entityType).Select(c => new
        {
            entityType = c.EntityType,
            fieldName = c.FieldName,
            kind = c.Kind,
            choices = c.Choices,
        }).ToList();

    // ── 열 고치기 ────────────────────────────────────────
    // 고치고 나면 뷰가 다시 지어지므로, 화면은 표를 다시 읽어야 한다.

    private object AddColumn(string entityType, string fieldName, string kind, string? choices)
    {
        _store.AddUserColumn(entityType, fieldName, kind, Split(choices));
        return UserColumns(null);
    }

    private object UpdateColumn(
        string entityType, string fieldName, string newName, string kind, string? choices)
    {
        _store.UpdateUserColumn(entityType, fieldName, newName, kind, Split(choices));
        return UserColumns(null);
    }

    private object RemoveColumn(string entityType, string fieldName)
    {
        _store.RemoveUserColumn(entityType, fieldName);
        return UserColumns(null);
    }

    private object MoveColumn(string entityType, string fieldName, string delta)
    {
        _store.MoveUserColumn(entityType, fieldName, int.Parse(delta));
        return UserColumns(null);
    }

    private object? SetField(string key, string field, string? value)
    {
        var entity = _store.Resolve(key)
                     ?? throw new InvalidOperationException($"그런 공고·계약이 없습니다: {key}");

        var known = _store.UserColumns(entity.EntityType).Select(c => c.FieldName).ToList();
        if (!known.Contains(field))
            throw new InvalidOperationException($"손으로 채우는 열이 아닙니다: {field}");

        _store.SetUserField(entity, field, value);
        return null;
    }

    // ── 수집한 값 고치기 ────────────────────────────────────
    // 손으로 채우는 열(setField)과 길이 갈린다. 담기는 표가 다르다.

    /// <summary>
    /// 계약면의 칸 하나에 사람이 적은 값을 씌운다.
    ///
    /// <para>고치기 전에 그 자리에 있던 값을 <b>함께 담는다</b> — 화면이 무엇을 무엇으로
    /// 고쳤는지 보이고 되돌리기를 안내하는 데 쓴다.</para>
    /// </summary>
    private object? SetOverride(string key, string column, string? value)
    {
        _store.SetOverride(Target(key), column, value ?? "");
        return null;
    }

    /// <summary>덮개를 걷는다. 뷰는 다시 수집한 값을 낸다.</summary>
    private object? ClearOverride(string key, string column)
    {
        _store.ClearOverride(Target(key), column);
        return null;
    }

    // ── 지우기 ───────────────────────────────────────────────

    /// <summary>지우면 무엇이 사라지는지. <b>묻기 전에 세어 보인다.</b></summary>
    private object Plan(string key, string scope) => _store.PlanDeletion(Target(key), Whole(scope));

    private object? Delete(string key, string scope)
    {
        _store.Delete(Target(key), Whole(scope));
        return null;
    }

    private EntityRef Target(string key) =>
        _store.Resolve(key) ?? throw new InvalidOperationException($"그런 공고·계약이 없습니다: {key}");

    /// <summary>계열 전체인가 이 차수 하나인가. 모르는 말이 오면 <b>넓은 쪽으로 넘기지 않는다</b>.</summary>
    private static bool Whole(string scope) => scope switch
    {
        "series" => true,
        "seq" => false,
        _ => throw new InvalidOperationException($"모르는 삭제 범위입니다: {scope}"),
    };

    /// <summary>설정 화면이 받아 가는 것.</summary>
    private object ReadSettings() => Describe(_settings.Read());

    /// <summary>
    /// 설정을 저장한다. 이름이 오지 않으면(<c>null</c>) <b>적혀 있던 이름을 그대로 둔다</b> —
    /// 이름을 모르는 부르는 쪽이 저장할 때마다 그것이 조용히 지워지면 안 된다
    /// (<see cref="SettingsStore.Save"/> 가 이미 그런 자세다).
    /// </summary>
    private object SaveSettings(string? submitterName) =>
        Describe(_settings.Save(submitterName: submitterName));

    private static object Describe(AppSettings settings) => new
    {
        // 제출본 파일 이름에만 쓰는 값이다 — 자료의 임자를 정하지 않는다(ADR-023).
        submitterName = settings.SubmitterName,

        // 골라 둔 계획 엑셀의 자리. 창을 다시 열어도 무엇을 골라 두었는지 보여야 한다.
        planPath = settings.PlanPath,
    };

    /// <summary>화면은 후보를 세로줄로 이어 보낸다 — 저장 형식과 같아 옮겨 담을 것이 없다.</summary>
    private static IReadOnlyList<string> Split(string? choices) =>
        choices is null
            ? []
            : [.. choices.Split('|', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)];

    /// <summary>
    /// 잇기 화면이 쓸 것을 한 번에 내준다.
    ///
    /// <para>후보 목록만 내면 <b>후보가 하나도 없는 미연결 계약이 화면에서 사라진다</b> —
    /// 물리치고 나면 그런 계약이 생기는데, 그러고도 이어지지 않은 줄을 알 길이 없다.
    /// 그래서 계약 목록을 함께 낸다.</para>
    ///
    /// <para>공고 전부도 여기 실어 보낸다. 화면이 「직접 찾기」로 거르는 자리라 열 때 한 번
    /// 받으면 되고, <b>다리에 검색 요청을 새로 파지 않는다</b> — 이 저장소의 다른 목록도
    /// 다 받아 놓고 화면에서 거른다.</para>
    /// </summary>
    private object LinkCandidates()
    {
        var linker = new Linker(_database);
        var work = linker.Work();

        return new
        {
            contracts = work.Contracts.Select(c => new
            {
                key = c.Contract.Display,
                title = c.Title,
                candidateCount = c.CandidateCount,
                noticeKey = c.Notice?.Display,
                noticeTitle = c.NoticeTitle,
                decidedBy = c.DecidedBy,
            }).ToList(),

            notices = linker.Notices().Select(n => new
            {
                key = n.Notice.Display,
                title = n.Title,
                postedAt = n.PostedAt,
                linked = n.Linked,
            }).ToList(),

            candidates = work.Candidates.Select(c => new
            {
                contractKey = c.Contract.Display,
                contractTitle = c.ContractTitle,
                noticeKey = c.Notice.Display,
                noticeTitle = c.NoticeTitle,
                confidence = c.Confidence,
                reason = c.Reason,
                noticeContractCount = c.NoticeContractCount,
                titleMatched = c.TitleMatched,
                blocker = c.Blocker,
                facets = Facets(c.Compared),
            }).ToList(),
        };
    }

    /// <summary>
    /// 옛 자동 링크를 다시 본다. <b>규칙판이 오른 뒤</b> 그 규칙으로 다시 판정하는 길이자,
    /// 사람이 값을 고친 뒤 다시 돌리는 길이다.
    ///
    /// <para>사람이 확정한 링크는 규칙판과 무관하게 남는다 — 푸는 것은 기계가 이은 것뿐이다
    /// (ADR-007·ADR-012). 두 링커를 모두 돌린다: 규칙은 따로 오르지만 사람이 누르는 것은
    /// 한 단추다.</para>
    /// </summary>
    private object Relink()
    {
        return new { linked = ExplicitLinks.Resolve(_database), released = 0 };
    }

    /// <summary>추천에 오르지 않은 짝을 나란히 견준다. 사람이 손수 고른 공고를 잇기 전에 볼 재료다.</summary>
    private object CompareLink(string contractKey, string noticeKey)
    {
        var (contract, notice) = ResolvePair(contractKey, noticeKey);
        return Facets(new Linker(_database).Compare(contract, notice));
    }

    private static object Facets(IReadOnlyList<LinkFacet> facets) =>
        facets.Select(f => new
        {
            name = f.Name,
            contract = f.Contract,
            notice = f.Notice,
            agrees = f.Agrees,
        }).ToList();

    private object? ConfirmLink(string contractKey, string noticeKey)
    {
        var (contract, notice) = ResolvePair(contractKey, noticeKey);
        new Linker(_database).Confirm(contract, notice, 1.0);
        return null;
    }

    /// <summary>이 짝은 아니라고 판정한다. 물리친 후보가 다시 목록에 오르지 않게 남긴다.</summary>
    private object? RejectLink(string contractKey, string noticeKey)
    {
        var (contract, notice) = ResolvePair(contractKey, noticeKey);
        new Linker(_database).Reject(contract, notice);
        return null;
    }

    private object? Unlink(string contractKey)
    {
        var contract = _store.Resolve(contractKey);
        if (contract?.EntityType != "contract")
            throw new InvalidOperationException($"그런 계약이 없습니다: {contractKey}");

        new Linker(_database).Unlink(contract);
        return null;
    }

    // ── 접수 ↔ 공고 잇기 ─────────────────────────────────────
    // 계약 ↔ 공고와 길이 갈린다. 판정 근거가 다르기 때문이다 — 저쪽은 전파된 건명,
    // 이쪽은 품목의 수량·단가 다중집합이다(ADR-021).

    /// <summary>
    /// 접수 잇기 화면이 쓸 것을 한 번에 내준다. <see cref="LinkCandidates"/> 와 같은 모양이라
    /// 화면이 카드·근거 표·직접 찾기를 그대로 쓴다.
    /// </summary>
    private object RequestLinkCandidates()
    {
        var linker = new RequestLinker(_database);
        var work = linker.Work();

        return new
        {
            requests = work.Requests.Select(r => new
            {
                key = r.Request.Display,
                title = r.Title,
                candidateCount = r.CandidateCount,
                noticeKey = r.Notice?.Display,
                noticeTitle = r.NoticeTitle,
                decidedBy = r.DecidedBy,
            }).ToList(),

            candidates = work.Candidates.Select(c => new
            {
                requestKey = c.Request.Display,
                requestTitle = c.RequestTitle,
                noticeKey = c.Notice.Display,
                noticeTitle = c.NoticeTitle,
                confidence = c.Confidence,
                reason = c.Reason,
                noticeLinkedCount = c.NoticeLinkedCount,
                titleMatched = c.TitleMatched,
                blocker = c.Blocker,
                facets = RequestFacets(c.Compared),
            }).ToList(),

            notices = linker.Notices().Select(n => new
            {
                key = n.Notice.Display,
                title = n.Title,
                postedAt = n.PostedAt,
                linked = n.Linked,
            }).ToList(),
        };
    }

    /// <summary>추천에 오르지 않은 짝을 나란히 견준다. 사람이 손수 고른 공고를 잇기 전에 볼 재료다.</summary>
    private object CompareRequestLink(string requestKey, string noticeKey)
    {
        var (request, notice) = ResolveRequestPair(requestKey, noticeKey);
        return RequestFacets(new RequestLinker(_database).Compare(request, notice));
    }

    private static object RequestFacets(IReadOnlyList<RequestFacet> facets) =>
        facets.Select(f => new
        {
            name = f.Name,
            request = f.Request,
            notice = f.Notice,
            agrees = f.Agrees,
        }).ToList();

    private object? ConfirmRequestLink(string requestKey, string noticeKey)
    {
        var (request, notice) = ResolveRequestPair(requestKey, noticeKey);
        new RequestLinker(_database).Confirm(request, notice, 1.0);
        return null;
    }

    /// <summary>이 짝은 아니라고 판정한다. 물리친 후보가 다시 목록에 오르지 않게 남긴다.</summary>
    private object? RejectRequestLink(string requestKey, string noticeKey)
    {
        var (request, notice) = ResolveRequestPair(requestKey, noticeKey);
        new RequestLinker(_database).Reject(request, notice);
        return null;
    }

    private object? UnlinkRequest(string requestKey)
    {
        var request = _store.Resolve(requestKey);
        if (request?.EntityType != "request")
            throw new InvalidOperationException($"그런 접수가 없습니다: {requestKey}");

        new RequestLinker(_database).Unlink(request);
        return null;
    }

    private (EntityRef Request, EntityRef Notice) ResolveRequestPair(string requestKey, string noticeKey)
    {
        var request = _store.Resolve(requestKey);
        var notice = _store.Resolve(noticeKey);

        if (request?.EntityType != "request") throw new InvalidOperationException($"그런 접수가 없습니다: {requestKey}");
        if (notice?.EntityType != "notice") throw new InvalidOperationException($"그런 공고가 없습니다: {noticeKey}");

        return (request, notice);
    }

    private (EntityRef Contract, EntityRef Notice) ResolvePair(string contractKey, string noticeKey)
    {
        var contract = _store.Resolve(contractKey);
        var notice = _store.Resolve(noticeKey);

        if (contract?.EntityType != "contract") throw new InvalidOperationException($"그런 계약이 없습니다: {contractKey}");
        if (notice?.EntityType != "notice") throw new InvalidOperationException($"그런 공고가 없습니다: {noticeKey}");

        return (contract, notice);
    }

    /// <summary>
    /// 계획 엑셀을 고르게 하고 <b>그 자리를 설정에 적어 둔다</b>. 그만두면 <c>null</c>.
    ///
    /// <para><b>경로가 곧 링크다.</b> 서식이 표본에 고정된 뒤(ADR-023 개정) 계획 가져오기에
    /// 남은 결정은 "어느 파일인가" 하나뿐이라, 다리가 메모리에 들고 있던 것을 설정으로 옮긴다 —
    /// 창을 닫았다 열어도, 명령줄에서 넣어도 같은 파일을 본다.</para>
    /// </summary>
    private object? PickPlanFile()
    {
        var picked = PickPlanExcel?.Invoke();
        if (string.IsNullOrEmpty(picked)) return null;

        // 열리는지 먼저 본다 — 열지도 못하는 파일을 골라 둔 것으로 남기지 않는다.
        PlanImport.Headers(picked);

        _settings.SavePlanPath(picked);

        return new { name = System.IO.Path.GetFileName(picked), path = picked };
    }

    /// <summary>
    /// 골라 둔 계획 엑셀을 넣는다. <c>pclm plan</c> 과 같은 길, 같은 파일이다.
    ///
    /// <para>필수 머리글이 없는 것은 고장이 아니라 <b>사람이 파일을 다시 보면 되는 일</b>이라
    /// 던지지 않고 <see cref="PlanImportResult"/> 로 담아 낸다 — 무엇이 없었는지가 화면에 서야
    /// 한다. 던지는 것은 고른 파일이 없거나 그 자리에 파일이 없을 때뿐이다.</para>
    /// </summary>
    private object ImportPlan()
    {
        var path = _settings.Read().PlanPath;

        if (string.IsNullOrEmpty(path))
            throw new InvalidOperationException("가져올 계획 엑셀을 고르지 않았습니다.");

        if (!File.Exists(path))
            throw new InvalidOperationException($"그런 파일이 없습니다: {path}");

        return new PlanImport(_database).Import(path);
    }

    /// <summary>
    /// 쌓인 것을 엑셀 한 권으로 뽑는다. <c>pclm export</c> 와 같은 파일, 같은 시트다.
    ///
    /// <para>자리를 <b>사람이 고르게 한다.</b> 이 파일은 그 시점의 사진이라 손으로 적은 것이
    /// DB 로 돌아오지 않는데, 앱이 정한 자리에 조용히 떨어뜨리면 어디로 갔는지 모르는 사진이
    /// 쌓인다 — 나중에 어느 것이 최신인지 가릴 길이 없다.</para>
    ///
    /// <para>돌려주는 것은 <b>실제로 쓴 전체 경로</b>다. 고르기를 그만두면 <c>null</c> —
    /// 화면이 아무 말도 하지 않고 그대로 있는다.</para>
    /// </summary>
    private object? Export()
    {
        var target = PickSavePath?.Invoke(new SavePrompt(
            "엑셀로 내보내기", "엑셀 통합 문서 (*.xlsx)|*.xlsx", ".xlsx", Exporter.DefaultFileName()));
        if (string.IsNullOrEmpty(target)) return null;

        try
        {
            return new Exporter(_database).Export(target);
        }
        catch (IOException)
        {
            // 엑셀이 그 파일을 붙들고 있으면 덮어쓰지 못한다. 원문은 영어 시스템 메시지라
            // 그대로 내보이면 무엇을 하라는 것인지 보이지 않는다.
            throw new InvalidOperationException(
                $"파일을 쓰지 못했습니다 — 엑셀에서 열려 있다면 닫고 다시 해 주세요.  {target}");
        }
    }

    /// <summary>
    /// 내 자료 한 벌을 제출본 파일로 뜬다. <c>pclm submit</c> 과 <b>같은 파일</b>이다.
    ///
    /// <para>자리를 사람이 고르게 한다 — 메일로 보낼 파일이라 어디에 떨어졌는지 모르면
    /// 그다음 손이 이어지지 않는다(<see cref="Export"/> 와 같은 자세).</para>
    ///
    /// <para><c>nameMissing</c> 을 여기서 내는 까닭: <b>실제로 무엇을 파일 이름에 넣었는지
    /// 아는 것은 앱뿐</b>이다. 화면이 들고 있는 설정은 저장 전 입력값과 어긋날 수 있어,
    /// 화면이 스스로 판정하면 이름을 적어 두고도 "비어 있다" 는 말을 듣는다.</para>
    /// </summary>
    private object? Submit()
    {
        var name = _settings.Read().SubmitterName;
        var stamp = DateTime.Now.ToString("yyyyMMdd");
        var suggested = name.Length == 0 ? $"제출_{stamp}.pclm" : $"제출_{name}_{stamp}.pclm";

        var target = PickSavePath?.Invoke(new SavePrompt(
            "제출본 만들기", "제출본 (*.pclm)|*.pclm", ".pclm", suggested));

        if (string.IsNullOrEmpty(target)) return null;   // 그만두면 화면은 아무 말도 하지 않는다

        return new { path = _database.Snapshot(target, overwrite: true), nameMissing = name.Length == 0 };
    }

    /// <summary>
    /// 제출본을 모아 취합본 하나로 짓는다. <c>pclm merge</c> 와 같은 길, 같은 보고다.
    ///
    /// <para>보고를 취합본 <b>옆에 글로 남긴다</b> — 창에는 알림 한 줄만 서는데, 겹친 것은
    /// 한 건씩 읽어야 하는 것이라 그 자리에서 지나가면 무엇이 밀려났는지 볼 데가 없다.</para>
    /// </summary>
    private object? MergeSubmissions()
    {
        var folder = PickFolder?.Invoke(new FolderPrompt("제출본이 든 폴더 고르기", null));
        if (string.IsNullOrEmpty(folder)) return null;

        var files = Merger.FindSubmissions(folder);

        // 조용히 빈 취합본을 만들지 않는다 — 받는 사람은 그것을 취합 결과로 안다.
        if (files.Count == 0)
            throw new InvalidOperationException(
                $"합칠 제출본을 찾지 못했습니다 — 이 폴더에 제출본(.pclm)이 없습니다.  {folder}");

        var stamp = DateTime.Now.ToString("yyyyMMdd");
        var path = System.IO.Path.Combine(folder, $"취합_{stamp}.pclm");
        var report = new Merger().Merge(files, path);

        // BOM 을 붙인다. 메모장은 BOM 없는 UTF-8 을 시스템 코드페이지로 읽어, 한글이 깨진
        // 글을 받는다 — 이 파일은 사람이 곧바로 열어 보라고 남기는 것이다.
        var reportPath = System.IO.Path.Combine(folder, $"취합_{stamp}.txt");
        File.WriteAllText(reportPath, MergeReportText.Render(report, path), new UTF8Encoding(true));

        return new
        {
            path,
            reportPath,
            submissions = report.제출본,
            plans = report.조달요구,
            requests = report.접수,
            notices = report.공고,
            contracts = report.계약,
            conflicts = report.겹친것.Count,
            rejected = report.거절한제출본.Count,
        };
    }

    private static void Reply(CoreWebView2 core, int id, bool ok, object? result, string? error) =>
        core.PostWebMessageAsJson(JsonSerializer.Serialize(new { id, ok, result, error }, Json));
}
