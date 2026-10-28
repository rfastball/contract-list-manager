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

/// <summary>
/// 저장 자리 고르기 요청. 제목·거르개·확장자·지어 둔 이름을 부르는 쪽이 정한다.
///
/// <para><paramref name="AllowOverwrite"/> 가 거짓이면 대화상자가 덮어쓰기를 묻지 않는다. 작업자료를 새로 세우는 자리는
/// 무엇이 있든 덮지 않으므로(<see cref="Home.CheckNewTarget"/>), 「바꾸시겠습니까?」 에 「예」 를 받은 뒤 거절하게 두지 않는다.
/// 엑셀·제출본·백업 같은 결과물은 덮는 것이 맞아 그대로 묻는다.</para>
/// </summary>
public sealed record SavePrompt(string Title, string Filter, string DefaultExt, string FileName, bool AllowOverwrite = true);

/// <summary>
/// 화면이 앱에 일을 시키는 유일한 통로.
///
/// <para>화면은 <c>{id, method, args}</c> 를 보내고 여기서 <c>{id, ok, result|error}</c> 로 답한다.
/// 번호로 짝을 맞추므로 여러 요청이 겹쳐도 섞이지 않는다.</para>
///
/// <para><b>화면에 SQL 을 열어 주지 않는다.</b> 부를 수 있는 일은 아래 <c>switch</c> 에 적힌 것뿐이고,
/// 표 이름도 계약면 뷰 목록에 있는 것만 받는다 — 화면이 내부 표를 직접 헤집지 못하게.</para>
/// </summary>
public sealed class Bridge(Database database, Home home, ViewedFile? viewing = null) : IDisposable
{
    /// <summary>
    /// 자료를 고치는 요청. <b>열람 중이면 SQLite 까지 가기 전에 여기서 사람 말로 거절한다</b>(ADR-031 셋째 겹).
    ///
    /// <para>열람 손잡이는 이미 읽기 전용이라 빠뜨려도 SQLite 가 막는다. 그래도 목록을 따로 두는 까닭은
    /// 그 거절이 <c>attempt to write a readonly database</c> 라는 영어 한 줄이라서다 — 사람은 무엇이 왜
    /// 안 되는지 모른다. 고르기 창·확장 준비처럼 SQLite 에 닿기 전에 일을 벌이는 요청도 여기서 막힌다.</para>
    ///
    /// <para>새 요청을 <see cref="Invoke"/> 에 더하면 여기에 넣을지 <see cref="ViewerAllowed"/> 에 넣을지 정한다 —
    /// 둘 중 어디에도 없으면 열람 중에 부르는 순간 거절된다. 빠뜨렸을 때 막히는 쪽으로 틀리게 한다.</para>
    /// </summary>
    public static IReadOnlySet<string> Writes { get; } = new HashSet<string>(StringComparer.Ordinal)
    {
        "addColumn", "updateColumn", "removeColumn", "moveColumn", "saveComputedColumn", "freezeColumn",
        "saveUserView", "deleteUserView", "pickViewPackage", "builtinViewPackage", "previewViewPackage", "importViewPackage",
        "setField", "setOverride", "clearOverride", "deleteEntity", "batchEdit", "batchDeletionPlan", "batchDelete",
        "saveSettings",
        "pickPlanExcel", "importPlan", "submit", "merge",
        "prepareExtension", "openExtensionSetup", "erpTools",
        // 지금 보는 화면(ADR-036). 가져오기는 쓰고, 명령은 열람 창에 비추는 탭이 없어 보낼 까닭이 없다.
        "importShot", "mirrorCommand",
        // 작업자료를 바꾸는 일은 본 창에서만 한다. 열람 창은 따로 뜬 프로세스라 거기서 바꾸면 본 창이 옛 자료를
        // 쥔 채 계속 고친다. 백업은 바꾸는 일이 아니지만 지금 작업자료의 사진이라 열람 창의 일이 아니다.
        "pickWorkfile", "moveWorkfile", "switchWorkfile", "newWorkfile", "backupWorkfile",
        // 창의 몸가짐은 자료가 아니라 이 컴퓨터의 작업자료 창의 것이다. 열람 창은 알림 영역에 두지도 않으니 보이지도 않는다.
        "windowPrefs", "saveWindowPrefs", "autoCaptureSettings", "saveAutoCaptureSettings",
    };

    /// <summary>열람 중에도 받는 요청. 읽기와, 밖으로 사진을 뜨는 것(<c>export</c>)과, 창 바깥의 일뿐이다.</summary>
    public static IReadOnlySet<string> ViewerAllowed { get; } = new HashSet<string>(StringComparer.Ordinal)
    {
        "session", "status", "dashboard", "dataVersion", "현황", "sheet", "outline", "userColumns", "columnContext",
        "traceComputedColumn", "compileComputedColumn", "validateComputedSql", "planFreezeColumn",
        "userViews", "userViewContext", "traceUserViewColumn", "userViewSamples", "previewUserView", "exportViewPackage",
        "deletionPlan", "settings",
        "extensionStatus", "captures", "mirror",
        "export", "exportSheet", "dataLocation", "revealDataFolder", "about", "openOther",
        // 창을 닫을 때 물을지와 정말 닫기. 자료가 아니라 창의 일이다.
        "setUnsaved", "closeWindow",
    };

    /// <summary>열어 본 파일. 작업자료를 연 창이면 <c>null</c> 이다.</summary>
    private readonly ViewedFile? _viewing = viewing;

    /// <summary>열람 창인가. 그렇다면 손잡이는 사본 위의 읽기 손잡이다.</summary>
    public bool ReadOnly => _viewing is not null;

    /// <summary>
    /// 업무 홈으로 띄웠는가. 내장 확장 연결은 업무 홈에서만 다룬다 — 레지스트리의 호스트 연결은 컴퓨터에
    /// 하나뿐이라, 시험 홈의 창이 그것을 고치면 사람의 확장이 시험 자료에 묶인다. 기본이 거짓인 것은
    /// 빠뜨렸을 때 막히는 쪽으로 틀리게 하려는 것이다.
    /// </summary>
    public bool IsDefaultHome { get; init; }

    private readonly Home _home = home;
    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.Never,
    };

    private readonly Database _database = database;
    private readonly Store _store = new(database);
    private readonly SettingsStore _settings = new(database);
    // data_version은 같은 연결에서 읽은 값끼리만 비교할 수 있다.
    private readonly Microsoft.Data.Sqlite.SqliteConnection _changes = database.OpenReadOnly();

    private long DataVersion()
    {
        lock (_changes)
        {
            using var command = _changes.CreateCommand();
            command.CommandText = "PRAGMA data_version;";
            return Convert.ToInt64(command.ExecuteScalar());
        }
    }

    public void Dispose()
    {
        _autoStopped = true;
        System.Threading.Interlocked.Exchange(ref _autoTimer, null)?.Dispose();
        lock (_changes) _changes.Dispose();
    }

    /// <summary>폴더 고르기 창을 연다. 그만두면 <c>null</c>.</summary>
    public Func<FolderPrompt, string?>? PickFolder { get; set; }

    /// <summary>저장할 자리를 고르게 한다. 지어 둔 이름을 건네고, 그만두면 <c>null</c>.</summary>
    public Func<SavePrompt, string?>? PickSavePath { get; set; }

    /// <summary>계획 엑셀 고르기 창을 연다. 그만두면 <c>null</c>.</summary>
    public Func<string?>? PickPlanExcel { get; set; }

    /// <summary><c>.pclm</c> 고르기 창을 연다. 제목은 부르는 쪽이 정한다. 그만두면 <c>null</c>.</summary>
    public Func<string, string?>? PickOpenFile { get; set; }
    public Func<string, string?>? PickOpenJson { get; set; }

    /// <summary>
    /// 이 창을 다시 띄운다(ADR-032 의 5). 작업자료를 바꾼 뒤 답을 보낸 다음에 부른다 — 창이 먼저 내려가면
    /// 화면은 무엇이 되었는지 듣지 못한다. 창(WPF)이 단다.
    /// </summary>
    public Action? Restart { get; set; }

    /// <summary>
    /// 닫기를 알림 영역으로 돌릴지 <b>지금</b> 바꾼다 — 아이콘이 곧바로 서거나 거둬진다. 창(WPF)이 단다.
    /// 달지 않았으면 적어 두기만 하고 다음에 띄울 때 선다.
    /// </summary>
    public Action<bool>? ApplyCloseToTray { get; set; }

    /// <summary>답을 보낸 뒤 다시 띄울 것인가. 다리의 일은 다른 실마리에서 돌므로 한 번만 넘기게 바꿔 쥔다.</summary>
    private int _restart;

    /// <summary>
    /// 화면에 저장하지 않은 편집(열 관리·내 보기)이 있는가. 화면이 바뀔 때마다 알린다. 서 있으면 창(WPF)은 닫기·끝내기를
    /// 멈추고 <see cref="Notify"/> 로 「닫기 요청」 을 보낸다 — 묻지 않고 닫으면 그 편집이 말없이 사라진다.
    /// </summary>
    public bool Unsaved => _unsaved;
    private volatile bool _unsaved;

    /// <summary>화면이 확인을 받은 뒤의 「정말 닫기」. 답을 보낸 다음에 부른다. 창(WPF)이 단다.</summary>
    public Action? CloseWindow { get; set; }

    /// <summary>답을 보낸 뒤 창을 닫을 것인가. <see cref="_restart"/> 와 같은 까닭으로 바꿔 쥔다.</summary>
    private int _close;

    /// <summary>먼저 알릴 화면. 창이 WebView2 를 세운 뒤 단다.</summary>
    private CoreWebView2? _core;

    /// <summary>화면에 먼저 알릴 자리를 단다. 창(WPF)이 WebView2 를 세운 뒤 부른다.</summary>
    public void Attach(CoreWebView2 core)
    {
        _core = core;
        // 화면이 죽으면 「닫기 요청」 에 답할 쪽이 없다. 깃발이 선 채 남으면 X 를 눌러도 창이 영영 닫히지 않으므로 내린다 —
        // 지킬 편집도 화면과 함께 이미 사라졌다. 다시 읽힌 화면은 처음 설 때 제 상태를 다시 알린다.
        core.ProcessFailed += (_, _) => _unsaved = false;
    }

    /// <summary>
    /// 화면에 번호 없는 알림을 보낸다(<c>{notice}</c>). 답과 같은 통로라 UI 실마리에서 부른다. 화면이 아직 서지 않았으면
    /// 아무것도 하지 않는다.
    /// </summary>
    public void Notify(string notice) =>
        _core?.PostWebMessageAsJson(JsonSerializer.Serialize(new { notice }, Json));

    /// <summary>
    /// 화면을 딴 데서 열고 있으면 그 자리(<c>--ui</c>). 열람 창도 같은 자리를 짚어야 개발 중에
    /// 지어 둔 화면이 없다는 말 대신 같은 화면이 선다.
    /// </summary>
    public string? UiOrigin { get; init; }

    public async Task HandleAsync(CoreWebView2 core, string messageJson)
    {
        var id = 0;

        try
        {
            using var document = JsonDocument.Parse(messageJson);
            var root = document.RootElement;

            id = root.GetProperty("id").GetInt32();
            var method = root.GetProperty("method").GetString() ?? "";
            // 글이 아닌 값(맞춤 뷰 보표의 표본 열쇠 목록)은 JSON 원문으로 넘긴다 — 받는 쪽이 읽는다.
            var args = root.TryGetProperty("args", out var a)
                ? a.EnumerateArray().Select(x => x.ValueKind switch
                {
                    JsonValueKind.Null or JsonValueKind.Undefined => null,
                    JsonValueKind.String => x.GetString(),
                    _ => x.GetRawText(),
                }).ToArray()
                : [];

            var result = await Task.Run(() => Invoke(method, args));
            Reply(core, id, true, result, null);
        }
        catch (Exception ex)
        {
            Reply(core, id, false, null, ex.Message);
        }

        if (Interlocked.Exchange(ref _restart, 0) == 1) Restart?.Invoke();
        if (Interlocked.Exchange(ref _close, 0) == 1) CloseWindow?.Invoke();
    }

    private object? Invoke(string method, string?[] args)
    {
        // 고치는 요청은 물론이고, 어느 쪽으로도 갈라 두지 않은 요청도 열람 중에는 받지 않는다.
        if (_viewing is not null && (Writes.Contains(method) || !ViewerAllowed.Contains(method)))
            throw new InvalidOperationException(
                $"열람 중인 자료라 고칠 수 없습니다 — {PclmRole.Name(_viewing.Role)} " +
                $"「{System.IO.Path.GetFileName(_viewing.SourcePath)}」 는 이 창에서 읽기만 합니다.");

        return Dispatch(method, args);
    }

    private object? Dispatch(string method, string?[] args) => method switch
    {
        "session" => Session(),
        "openOther" => OpenOther(),
        "prepareExtension" => PrepareExtension(),
        "extensionStatus" => ReadExtensionStatus(),
        // 오늘 들어온 수집과 마지막 하나. 읽기만 하므로 열람 창(읽기 전용 사본)에서도 같은 길이다.
        "captures" => CaptureLog.Read(_database, DateTime.Now),
        "openExtensionSetup" => OpenExtensionSetup(Require(args, 0)),
        "erpTools" => ErpTools.Invoke(_database, Require(args, 0), Require(args, 1), _home),
        "mirror" => ReadMirror(),
        "autoCaptureSettings" => ReadAutoCaptureSettings(),
        "saveAutoCaptureSettings" => SaveAutoCaptureSettings(Require(args, 0)),
        "mirrorCommand" => MirrorCommand(Require(args, 0), Require(args, 1), Require(args, 2)),
        "importShot" => ImportShot(Require(args, 0), Require(args, 1), Require(args, 2), Require(args, 3), Require(args, 4)),
        "status" => new Reports(_database).Summary(),
        "dashboard" => new Reports(_database).Dashboard(),
        "dataVersion" => DataVersion(),
        "현황" => new Pclm.Core.Storage.Status(_database).Report(),
        "sheet" => ReadSheet(Require(args, 0)),
        "outline" => ReadOutline(),
        "userColumns" => UserColumns(args.ElementAtOrDefault(0)),
        "columnContext" => _store.ColumnContext(Require(args, 0)),
        "traceComputedColumn" => _store.TraceComputedColumn(Require(args, 0), Require(args, 1)),
        "compileComputedColumn" => _store.CompileComputedColumn(Require(args, 0), Require(args, 1)),
        "validateComputedSql" => _store.ValidateComputedSql(Require(args, 0), Require(args, 1)),
        "planFreezeColumn" => _store.PlanFreezeColumn(Require(args, 0), Require(args, 1)),
        "freezeColumn" => FreezeColumn(Require(args, 0), Require(args, 1), args.ElementAtOrDefault(2)),
        // 맞춤 뷰(docs/user-views.md §8). 화면의 types.ts 「맞춤 뷰 다리 약속」 과 이름·자리·꼴이 같다. 기준 인자는 없다 — 줄은 조달 건이다.
        "userViews" => _store.UserViews(),
        "userViewContext" => _store.UserViewContext(),
        "traceUserViewColumn" => _store.TraceUserViewColumn(Require(args, 0), SampleKeys(args.ElementAtOrDefault(1))),
        "userViewSamples" => _store.UserViewSamples(Require(args, 0), args.ElementAtOrDefault(1), args.ElementAtOrDefault(2)),
        "previewUserView" => _store.PreviewUserView(Require(args, 0), args.ElementAtOrDefault(1)),
        "saveUserView" => _store.SaveUserView(Require(args, 0), args.ElementAtOrDefault(1)),
        "exportViewPackage" => ExportViewPackage(Require(args, 0)),
        "pickViewPackage" => PickViewPackage(),
        "builtinViewPackage" => BuiltinViewPackage(),
        "previewViewPackage" => _store.PreviewViewPackage(Require(args, 0)),
        "importViewPackage" => _store.ImportViewPackage(Require(args, 0), Require(args, 1)),
        "deleteUserView" => _store.DeleteUserView(Require(args, 0)),
        "saveComputedColumn" => SaveComputedColumn(Require(args, 0), Require(args, 1), Require(args, 2), args.ElementAtOrDefault(3)),
        "addColumn" => AddColumn(Require(args, 0), Require(args, 1), Require(args, 2), args.ElementAtOrDefault(3)),
        "updateColumn" => UpdateColumn(Require(args, 0), Require(args, 1), Require(args, 2), Require(args, 3), args.ElementAtOrDefault(4)),
        "removeColumn" => RemoveColumn(Require(args, 0), Require(args, 1)),
        "moveColumn" => MoveColumn(Require(args, 0), Require(args, 1), Require(args, 2)),
        "setField" => SetField(Require(args, 0), Require(args, 1), args.ElementAtOrDefault(2)),
        "setOverride" => SetOverride(Require(args, 0), Require(args, 1), args.ElementAtOrDefault(2)),
        "clearOverride" => ClearOverride(Require(args, 0), Require(args, 1)),
        "deletionPlan" => Plan(Require(args, 0), Require(args, 1)),
        "deleteEntity" => Delete(Require(args, 0), Require(args, 1)),
        "batchEdit" => new { changed = _store.BatchEdit(Require(args, 0), ReadBatch<CellChange[]>(Require(args, 1))) },
        "batchDeletionPlan" => _store.PlanBatchDeletion(ReadBatch<string[]>(Require(args, 0)), Whole(Require(args, 1))),
        "batchDelete" => _store.BatchDelete(ReadBatch<string[]>(Require(args, 0)), Whole(Require(args, 1)), Require(args, 2)),
        "settings" => ReadSettings(),
        "saveSettings" => SaveSettings(args.ElementAtOrDefault(0)),
        "export" => Export(),
        "exportSheet" => ExportSheet(Require(args, 0), ReadBatch<SheetSnapshot>(Require(args, 1))),
        "submit" => Submit(),
        "merge" => MergeSubmissions(),
        "pickPlanExcel" => PickPlanFile(),
        "importPlan" => ImportPlan(),
        "dataLocation" => DataLocationInfo(),
        "pickWorkfile" => PickWorkfile(Require(args, 0)),
        "moveWorkfile" => Switching(() => _home.MoveTo(_database, Require(args, 0))),
        "switchWorkfile" => Switching(() => args.ElementAtOrDefault(1) is { } target
            ? _home.CreateFromSnapshot(_database, Require(args, 0), target)
            : _home.UseInPlace(_database, Require(args, 0))),
        "newWorkfile" => Switching(() => _home.CreateNewWhileRunning(_database, Require(args, 0))),
        "backupWorkfile" => BackupWorkfile(),
        "revealDataFolder" => RevealDataFolder(),
        "about" => About(),
        "windowPrefs" => ReadWindowPrefs(),
        "saveWindowPrefs" => SaveWindowPrefs(Flag(Require(args, 0)), Flag(Require(args, 1))),
        "setUnsaved" => _unsaved = Flag(Require(args, 0)),
        "closeWindow" => CloseAfterReply(),
        _ => throw new InvalidOperationException($"모르는 요청입니다: {method}"),
    };

    /// <summary>
    /// 지금 어느 작업자료를 열고 있는지와, 그것을 가리키는 홈. 창이 자기 자리를 말하지 않으면 사람은 알 길이 없다.
    /// 옮기는 길은 여기 없다 — 자리를 바꾸는 일은 전환 절차를 거쳐야 한다(ADR-032).
    /// </summary>
    private object DataLocationInfo()
    {
        // 열람 중이면 사람이 고른 원본을 보인다. 임시 사본의 자리는 사람이 알 까닭이 없다.
        var path = ShownPath;
        var file = new FileInfo(path);
        return new
        {
            path,
            folder = System.IO.Path.GetDirectoryName(path),
            sizeBytes = file.Exists ? file.Length : 0L,
            home = _home.Directory,
            configPath = _home.ConfigPath,
        };
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
        // 호스트는 홈의 쪽지를 읽으므로 따로 적어 줄 연결이 없다. 확장과 레지스트리만 준비한다.
        return ExtensionSetup.Prepare();
    }

    private object ReadExtensionStatus()
    {
        RequireProductionExtension();
        // 지금 붙은 브라우저와 오늘의 연결 기록을 함께 낸다 — 사이드바·통로·상태 보드가 한 번 읽은 것으로 같은 말을 한다.
        // 읽기만 한다(끝난 호스트의 연결 파일을 거두는 것 말고). 열람 창도 같은 홈을 보므로 그대로 동작한다.
        return ExtensionStatus.Read(_home.Directory, BundledExtension.EmbeddedVersion(), DateTime.Now);
    }

    private void RequireProductionExtension()
    {
        if (!IsDefaultHome)
            throw new InvalidOperationException("내장 확장 연결은 업무 홈에서만 준비합니다. --home 이나 --erp-dev 없이 앱을 다시 실행하세요.");
    }

    /// <summary>
    /// 지금 보고 있는 파일. 열람 중이면 원본이다 — 임시 사본은 홈 안의 앱 자리라 사람에게 보일 것이 아니다.
    /// </summary>
    private string ShownPath => _viewing?.SourcePath ?? _database.Path;

    /// <summary>
    /// 이 창이 무엇을 열고 있는가. 화면은 이것으로 띠를 띄우고 편집 칸을 잠근다(ADR-031 넷째 겹).
    /// 작업자료를 연 창은 언제나 <c>work</c> 다 — <see cref="PclmFile.OpenWork"/> 가 그것만 받는다.
    /// </summary>
    private object Session()
    {
        var role = _viewing?.Role ?? PclmRole.Work;
        return new
        {
            readOnly = _viewing is not null,
            role,
            roleName = PclmRole.Name(role),
            path = ShownPath,
        };
    }

    /// <summary>
    /// 다른 자료를 <b>새 창(새 프로세스)</b>으로 열어 본다(ADR-031 「다른 자료 열어 보기」).
    ///
    /// <para>이 창의 손잡이를 갈아 끼우지 않는다. 그러면 열람이 활성 작업자료·확장 호스트에 닿을 길이
    /// 구조적으로 없다 — 갈아 끼우는 창은 갈아 끼운 사이에 무엇이 어디로 쓰였는지를 따져야 한다.</para>
    ///
    /// <para>홈을 함께 넘긴다. 열람 창도 사본·오류 기록·WebView2 프로필을 같은 홈에 둔다 — 시험 홈의
    /// 창이 연 열람이 업무 홈에 사본을 흘리면 격리가 아니다.</para>
    /// </summary>
    private object? OpenOther()
    {
        var picked = PickOpenFile?.Invoke("열어 볼 자료 고르기");
        if (string.IsNullOrEmpty(picked)) return null;

        var full = System.IO.Path.GetFullPath(picked);

        // 내 작업자료는 이미 이 창이 쓰고 있다. 열람 창을 띄우면 같은 파일을 두 창이 보며 한쪽은 낡는다.
        if (_viewing is null && string.Equals(full, _database.Path, StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException($"지금 이 창에서 쓰고 있는 작업자료입니다: {full}");

        // 열람 창에서 내 작업자료를 골랐으면 새 창이 서지 않는다 — 새 프로세스는 그것을 열람이 아니라 보통으로 열려다
        // 홈 잠금에 막혀 본 창만 앞으로 부르고 끝난다. 「새 창에서 열어 봅니다」 라고 말하지 않게 여기서 거절하고 본 창을 부른다.
        // 열람 창의 손잡이는 임시 사본이라 그것과 견주지 않고 쪽지가 가리키는 파일과 견준다.
        if (_viewing is not null && _home.IsActiveWorkfile(full))
            throw new InvalidOperationException(Activation.Signal(_home)
                ? $"지금 작업자료입니다 — 본 창을 앞으로 불렀습니다. 본 창에서 보세요: {full}"
                : $"지금 작업자료입니다 — 본 창에서 보세요: {full}");

        var start = new System.Diagnostics.ProcessStartInfo(
            Environment.ProcessPath ?? throw new InvalidOperationException("이 프로그램의 자리를 알 수 없습니다."))
        {
            UseShellExecute = false,
        };
        start.ArgumentList.Add("--view");
        start.ArgumentList.Add(full);
        if (!_home.IsDefault)
        {
            start.ArgumentList.Add("--home");
            start.ArgumentList.Add(_home.Directory);
        }
        if (UiOrigin is not null)
        {
            start.ArgumentList.Add("--ui");
            start.ArgumentList.Add(UiOrigin);
        }

        System.Diagnostics.Process.Start(start)?.Dispose();
        return new { path = full };
    }

    // ── 작업자료 바꾸기 ──────────────────────────────────────
    // 고르기와 바꾸기를 두 요청으로 가른다. 화면은 고른 자리를 받아 무엇이 일어나는지 보이고 확인을 받은 뒤에야
    // 바꾸기를 부른다 — 바꾸는 일은 창을 다시 띄우고, 옮기기는 옛 파일을 지운다.

    private const string WorkfileFilter = "계약 목록 자료 (*.pclm)|*.pclm";

    /// <summary>
    /// 무엇으로 바꿀지 고르게 한다. <b>아무것도 바꾸지 않는다</b> — 고른 것과 그 뜻을 돌려줄 뿐이다.
    /// 그만두면 <c>null</c>. 쓸 수 없는 자리는 여기서 거절한다: 확인을 받은 뒤에 거절하면 사람은 한 번 더 고른다.
    /// </summary>
    /// <param name="kind"><c>move</c>(옮기기) · <c>switch</c>(바꾸기) · <c>new</c>(새 계약자료).</param>
    private object? PickWorkfile(string kind)
    {
        string? Save(string title, string fileName) =>
            PickSavePath?.Invoke(new SavePrompt(title, WorkfileFilter, ".pclm", fileName, AllowOverwrite: false));

        object Plan(string action, string target, string? source, string? sourceRoleName) => new
        {
            action,
            path = Home.CheckNewTarget(_database, target),
            source,
            sourceRoleName,
            current = _database.Path,
        };

        switch (kind)
        {
            case "move":
            {
                var target = Save("작업자료 옮기기", System.IO.Path.GetFileName(_database.Path));
                return string.IsNullOrEmpty(target) ? null : Plan("move", target, _database.Path, null);
            }

            case "new":
            {
                var target = Save("새 계약자료", Home.DefaultWorkfileName);
                return string.IsNullOrEmpty(target) ? null : Plan("new", target, null, null);
            }

            case "switch":
            {
                var picked = PickOpenFile?.Invoke("작업자료로 쓸 파일 고르기");
                if (string.IsNullOrEmpty(picked)) return null;

                var full = System.IO.Path.GetFullPath(picked);
                if (string.Equals(full, _database.Path, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidOperationException($"지금 이 창에서 쓰고 있는 작업자료입니다: {full}");

                var info = PclmFile.Inspect(full);
                if (info.Kind != PclmKind.Ok)
                    throw new PclmFileException(full, info.Kind, info.Role, PclmFile.Why(info, full));

                // 작업자료면 그 자리를 쓴다. 제출본·취합본·백업은 그때의 기록이라 고치지 않는다 — 사본을 뜰 자리를 한 번 더 묻는다.
                if (info.Role == PclmRole.Work)
                {
                    Home.RequireLocal(full);
                    return new { action = "use", path = full, source = (string?)null, sourceRoleName = (string?)null, current = _database.Path };
                }

                var target = Save("새 계약자료로 만들 자리", Home.DefaultWorkfileName);
                return string.IsNullOrEmpty(target) ? null : Plan("snapshot", target, full, PclmRole.Name(info.Role));
            }

            default:
                throw new InvalidOperationException($"모르는 바꾸기입니다: {kind}");
        }
    }

    /// <summary>
    /// 작업자료를 바꾸고, 답을 보낸 뒤 창을 다시 띄우게 한다.
    ///
    /// <para>실패해도 쪽지가 이미 다른 파일을 가리키면 다시 띄운다. 전환은 쪽지를 쓴 뒤로는 되돌리지 않으므로
    /// (<see cref="Home"/> 의 전환 절차), 그 뒤에 무엇이 실패했든 이 창이 쥔 것은 더 이상 작업자료가 아니다 —
    /// 그대로 두면 사람의 편집이 다음 시작에 열리지 않을 파일에 쌓인다.</para>
    /// </summary>
    private object Switching(Func<string> work)
    {
        try
        {
            return new { restart = true, path = work() };
        }
        finally
        {
            if (!_home.IsActiveWorkfile(_database.Path)) Interlocked.Exchange(ref _restart, 1);
        }
    }

    /// <summary>
    /// 지금 시점을 백업으로 뜬다. 자리를 사람이 고르게 한다 — 제출본과 같은 자세다. 쪽지는 그대로다.
    /// </summary>
    private object? BackupWorkfile()
    {
        var stamp = DateTime.Now.ToString("yyyyMMdd");
        var target = PickSavePath?.Invoke(new SavePrompt(
            "백업 만들기", "백업 (*.pclm)|*.pclm", ".pclm", $"계약자료_백업_{stamp}.pclm"));
        if (string.IsNullOrEmpty(target)) return null;

        return new { path = Writing(target, () => Home.BackupTo(_database, target)) };
    }

    /// <summary>
    /// 사람이 고른 자리에 파일을 쓴다. 쓰지 못하면 .NET 의 영어 원문 대신 할 일을 말한다 — 엑셀 내보내기와 같은 자세다.
    /// 앱이 스스로 거절한 것(작업자료를 덮지 않는다 따위)은 이미 사람 말이라 그대로 지나간다.
    /// </summary>
    private static T Writing<T>(string target, Func<T> write)
    {
        try { return write(); }
        catch (UnauthorizedAccessException)
        {
            throw new InvalidOperationException($"그 자리에 쓸 권한이 없습니다 — 다른 폴더를 골라 다시 해 주세요.  {target}");
        }
        catch (IOException)
        {
            throw new InvalidOperationException($"파일을 쓰지 못했습니다 — 다른 프로그램에서 열려 있다면 닫고 다시 하거나, 다른 자리를 골라 주세요.  {target}");
        }
    }

    private object RevealDataFolder()
    {
        var folder = System.IO.Path.GetDirectoryName(ShownPath)!;
        Directory.CreateDirectory(folder);
        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(folder) { UseShellExecute = true });
        return new { folder };
    }

    // ── 지금 보는 화면(ADR-036) ───────────────────────────────

    private readonly object _mirrorGate = new();
    private readonly object _autoGate = new();
    private readonly AutoCapture _autoCapture = new(database, home);
    private System.Threading.Timer? _autoTimer;
    private volatile bool _autoStopped;
    private AutoCaptureResult? _autoResult;

    /// <summary>창 생성 때 시작한다. 처음부터 트레이에서 켠 창도 UI 로드와 무관하게 수집한다.</summary>
    public void StartAutoCapture()
    {
        if (ReadOnly) return;
        lock (_autoGate)
            if (!_autoStopped && _autoTimer is null)
                _autoTimer = new System.Threading.Timer(_ => AutoCaptureTick(), null, TimeSpan.FromSeconds(1), TimeSpan.FromSeconds(1));
    }

    private void AutoCaptureTick()
    {
        if (!System.Threading.Monitor.TryEnter(_autoGate)) return;
        try
        {
            if (_autoStopped) return;
            var preferences = AutoCapturePreferences.Read(_home);
            if (!preferences.Enabled) { _autoResult = null; return; }
            var report = ExtensionRelay.ReadTabs(_home.Directory).OrderByDescending(r =>
                r.Report.TryGetProperty("frontAt", out var at) && at.TryGetInt64(out var n) ? n : 0).FirstOrDefault();
            if (report is null || !report.Report.TryGetProperty("front", out var front) || front.ValueKind != JsonValueKind.Number || !front.TryGetInt32(out var id) ||
                !report.Report.TryGetProperty("tabs", out var tabs)) return;
            var tab = tabs.EnumerateArray().FirstOrDefault(t => t.TryGetProperty("tabId", out var v) && v.TryGetInt32(out var n) && n == id);
            if (tab.ValueKind != JsonValueKind.Object) return;
            _autoResult = _autoCapture.TrySave(tab, preferences, DateTimeOffset.UtcNow, () => !_autoStopped);
        }
        catch (Exception e) when (e is InvalidOperationException or JsonException or IOException or UnauthorizedAccessException or
            Microsoft.Data.Sqlite.SqliteException or KeyNotFoundException or FormatException)
        { _autoResult = new("hold", e.Message); }
        finally { System.Threading.Monitor.Exit(_autoGate); }
    }

    private object ReadAutoCaptureSettings()
    {
        lock (_autoGate)
        {
            var preferences = AutoCapturePreferences.Read(_home);
            return new { preferences.Enabled, preferences.Name, lastResult = _autoResult };
        }
    }

    private object SaveAutoCaptureSettings(string json)
    {
        lock (_autoGate)
        {
            var preferences = JsonSerializer.Deserialize<AutoCapturePreferences>(json, Mapping.Json)
                ?? throw new InvalidOperationException("자동 수집 설정이 비었습니다.");
            preferences.Write(_home);
            _autoResult = null;
            return ReadAutoCaptureSettings();
        }
    }
    /// <summary>탭마다 마지막 투영. 같은 스냅샷·같은 dataVersion 이면 다시 계산하지 않는다.</summary>
    private Dictionary<string, (string Key, MirrorShot? Shot, string? Error)> _shots = new(StringComparer.Ordinal);
    /// <summary>
    /// 결과를 받기 전의 가져오기. 같은 수집 ID 로 다시 부르면 처음 고른 그대로 다시 보낸다 — 그 사이 자료가 바뀌어
    /// 고를 것이 달라져도 같은 ID 에 다른 내용을 싣지 않는다(<see cref="ErpCapture.Save"/> 가 거절한다).
    /// </summary>
    private readonly Dictionary<string, (CaptureInput Input, string BaseToken, Dictionary<string, string> Choices)> _imports = new(StringComparer.Ordinal);

    /// <summary>
    /// Chrome 의 나라장터 탭들과, 수집할 수 있는 탭마다 작업자료에 비춘 것. 열람 창은 읽지 않는다 — 비출 작업자료가
    /// 아니고, 탭 보고에는 실제 조달 자료가 들어 있다.
    /// </summary>
    private object ReadMirror()
    {
        if (_viewing is not null)
            return new { readOnly = true, reporting = false, front = (string?)null, tabs = Array.Empty<object>(), settings = (object?)null,
                screens = Array.Empty<MirrorScreen>() };
        var reports = ExtensionRelay.ReadTabs(_home.Directory);
        var version = DataVersion();
        string? front = null;
        long frontAt = long.MinValue;
        // 확장 설정을 바꿀 브라우저 — 앞 탭의 브라우저, 없으면 처음 붙은 것.
        ExtensionRelay.TabReport? owner = null;
        var tabs = new List<(string Id, int Pid, int TabId, string Browser, JsonElement Tab)>();
        foreach (var report in reports)
        {
            var root = report.Report;
            // 브라우저가 둘이면 가장 늦게 앞 탭을 바꾼 쪽이 「앞」 이다.
            if (root.TryGetProperty("front", out var f) && f.ValueKind == JsonValueKind.Number &&
                root.TryGetProperty("frontAt", out var at) && at.TryGetInt64(out var when) && when >= frontAt)
            {
                front = $"{report.Pid}:{f.GetInt32()}";
                frontAt = when;
                owner = report;
            }
            if (!root.TryGetProperty("tabs", out var list) || list.ValueKind != JsonValueKind.Array) continue;
            foreach (var tab in list.EnumerateArray())
                if (tab.TryGetProperty("tabId", out var id) && id.TryGetInt32(out var tabId))
                    tabs.Add(($"{report.Pid}:{tabId}", report.Pid, tabId, report.Browser, tab));
        }
        lock (_mirrorGate)
        {
            var next = new Dictionary<string, (string Key, MirrorShot? Shot, string? Error)>(StringComparer.Ordinal);
            var shown = tabs.Select(t =>
            {
                MirrorShot? shot = null;
                string? error = null;
                var state = Text(t.Tab, "state");
                // 다시 읽는 동안에도 앞서 읽은 화면을 그대로 비춘다 — 읽는 중이라고 몸이 비지 않게.
                if (state is "supported" or "reading" && t.Tab.TryGetProperty("snapshot", out var snapshot) && snapshot.ValueKind == JsonValueKind.Object)
                {
                    var key = version.ToString(System.Globalization.CultureInfo.InvariantCulture) + ":" + Mapping.Hash(snapshot.GetRawText()) +
                        ":" + Mapping.Hash(t.Tab.TryGetProperty("screenRows", out var rows) ? rows.GetRawText() : "");
                    if (_shots.TryGetValue(t.Id, out var cached) && cached.Key == key) (shot, error) = (cached.Shot, cached.Error);
                    else
                    {
                        try { shot = Pclm.Core.Erp.Mirror.Project(_database, Pclm.Core.Erp.Mirror.Input(snapshot), Pclm.Core.Erp.Mirror.ScreenRows(t.Tab)); }
                        catch (Exception e) when (e is InvalidOperationException or JsonException or KeyNotFoundException or FormatException or
                            Microsoft.Data.Sqlite.SqliteException) { error = e.Message; }
                    }
                    next[t.Id] = (key, shot, error);
                }
                return new
                {
                    id = t.Id, pid = t.Pid, tabId = t.TabId, browser = t.Browser,
                    // menu: 화면의 메뉴 번호(ADR-037). 모르거나 옛 확장이면 빈 글.
                    title = Text(t.Tab, "title"), screen = Text(t.Tab, "screen"), menu = Text(t.Tab, "menu"), state,
                    readAt = Text(t.Tab, "readAt"), message = Text(t.Tab, "message"),
                    front = t.Id == front, shot, error,
                    // 화면 그대로 — 이름표와 보이는 글. 펼친 보기의 왼쪽이 그린다.
                    screenRows = Pclm.Core.Erp.Mirror.ScreenRows(t.Tab),
                    // 앱이 부른 내보내기의 마지막 결과. 옛 확장은 싣지 않아 null 이다.
                    export = Pclm.Core.Erp.Mirror.Export(t.Tab),
                };
            }).ToList();
            _shots = next;
            owner ??= reports.OrderBy(r => r.ConnectedAt, StringComparer.Ordinal).FirstOrDefault();
            object? settings = null;
            if (owner is { } o && o.Report.TryGetProperty("settings", out var given) && given.ValueKind == JsonValueKind.Object)
                settings = new
                {
                    pid = o.Pid, browser = o.Browser,
                    siteAccess = given.TryGetProperty("siteAccess", out var access) && access.ValueKind == JsonValueKind.True,
                };
            // screens: 수집하는 화면(ADR-037) — 「수집 안 함」 의 안내가 매핑에서 그대로 읽는다.
            return new { readOnly = false, reporting = reports.Count > 0, front, tabs = shown, settings,
                screens = Pclm.Core.Erp.Mirror.SupportedScreens(_database) };
        }

        static string Text(JsonElement e, string name) =>
            e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
    }

    /// <summary>그 탭을 앞으로(<c>focusTab</c>) · 다시 읽기(<c>read</c>) · 현재 화면 엑셀로 내보내기(<c>export</c>).</summary>
    private object MirrorCommand(string pid, string tabId, string command)
    {
        if (!ExtensionRelay.Send(_home.Directory, int.Parse(pid, System.Globalization.CultureInfo.InvariantCulture),
                command, int.Parse(tabId, System.Globalization.CultureInfo.InvariantCulture)))
            throw new InvalidOperationException("브라우저와의 연결이 끊겨 그 탭에 닿지 못했습니다.");
        return new { sent = true };
    }

    /// <summary>
    /// 확장 폴더나 브라우저의 확장 관리 화면을 연다. 브라우저는 명령줄로 받은 chrome://·edge:// 주소를 막고 새 탭만 열므로,
    /// 그 브라우저에 확장이 붙어 있으면 확장에 맡기고(<c>openExtensions</c>) 아니면 — 맡겼는데 창이 서지 않았어도 — 새 창의
    /// 주소창에 적어 넣는다.
    /// </summary>
    private object? OpenExtensionSetup(string target)
    {
        var browser = target switch { "chrome" => "Chrome", "edge" => "Edge", _ => null };
        if (browser is not null)
            foreach (var (pid, contact) in ExtensionPresence.LiveProcesses(_home.Directory))
                if (contact.Browser == browser && OpenThroughExtension(pid, target, "openExtensions"))
                    return null;
        return ExtensionSetup.Open(target);
    }

    /// <summary>
    /// 브라우저 안의 화면을 확장에게 새 창으로 열게 하고, 그 창을 이 앱이 앞으로 가져온다. 새 창이 서지 않으면 false.
    ///
    /// <para><b>보냈다는 것만으로 끝내지 않는다.</b> 예전에는 보내고 곧바로 돌아섰다 — 그 명령을 모르는 옛 확장은 조용히 버리고,
    /// 아는 확장이 연 탭은 앱 창 뒤의 브라우저 창에 섰다. 어느 쪽이든 누른 사람 눈에는 아무 일도 없었다. 판 번호가 같은 채
    /// 확장 코드만 바뀐 동안에는 옛 확장인지도 가려낼 수 없어, 창이 서는 것을 보고서야 됐다고 친다.</para>
    /// </summary>
    private bool OpenThroughExtension(int pid, string target, string command)
    {
        var before = ExtensionSetup.Windows(target);
        return ExtensionRelay.SendSetting(_home.Directory, pid, command) && ExtensionSetup.Raise(target, before, TimeSpan.FromSeconds(3));
    }

    /// <summary>
    /// 창에서 가져오기. 수집의 저장(<see cref="ErpCapture.Save"/>)과 <b>같은 절차</b>를 탄다(ADR-032·036) — 쓰기 잠금 안에서 역할 work·쪽지가 이 파일을
    /// 가리키는지·baseToken 을 다시 보고 쓴다. 더하고 바꾸는 것은 기본, 지우는 것은 사람이 고를 때만이다
    /// (<see cref="Pclm.Core.Erp.Mirror.DefaultChoices"/>). 짝 없는 화면의 줄은 새 줄로 둔다 — 투영과 같은 입력을 지어야
    /// baseToken 이 맞는다(<see cref="Pclm.Core.Erp.Mirror.Prepare"/>). baseToken 이 낡았으면 쓰지 않고 그 탭을 다시 읽게 한다.
    /// </summary>
    /// <param name="picks">사람이 고른 자리(JSON 문자열 배열) — 수집 원값으로 복원할 덮개, 비울 칸, 지울 행.</param>
    private object ImportShot(string pidText, string tabText, string baseToken, string picks, string captureId)
    {
        if (!Guid.TryParse(captureId, out _)) throw new InvalidOperationException("올바른 수집 ID가 필요합니다.");
        var pid = int.Parse(pidText, System.Globalization.CultureInfo.InvariantCulture);
        var tabId = int.Parse(tabText, System.Globalization.CultureInfo.InvariantCulture);
        (CaptureInput Input, string BaseToken, Dictionary<string, string> Choices) request;
        lock (_mirrorGate)
        {
            if (!_imports.TryGetValue(captureId, out request))
            {
                var snapshot = ExtensionRelay.ReadTabs(_home.Directory).Where(r => r.Pid == pid)
                    .SelectMany(r => r.Report.TryGetProperty("tabs", out var list) ? list.EnumerateArray().ToList() : [])
                    .Where(t => t.TryGetProperty("tabId", out var id) && id.TryGetInt32(out var n) && n == tabId &&
                        t.TryGetProperty("snapshot", out var s) && s.ValueKind == JsonValueKind.Object)
                    .Select(t => (JsonElement?)t.GetProperty("snapshot")).FirstOrDefault();
                // 보고에 그 탭의 화면이 없다 — 닫혔거나 브라우저가 끊겼다. 다시 읽을 탭이 없으니 읽으라고 보내지도, 다시 읽는다고
                // 말하지도 않는다.
                if (snapshot is null) return Gone;
                var (input, preview) = Pclm.Core.Erp.Mirror.Prepare(_database, Pclm.Core.Erp.Mirror.Input(snapshot.Value));
                if (preview.BaseToken != baseToken) return Stale(pid, tabId);
                var picked = JsonSerializer.Deserialize<string[]>(picks) ?? [];
                request = (input, baseToken, Pclm.Core.Erp.Mirror.DefaultChoices(preview, picked.ToHashSet(StringComparer.Ordinal)));
                _imports[captureId] = request;
            }
        }
        JsonElement stored;
        try
        {
            stored = JsonSerializer.SerializeToElement(
                new ErpCapture(_database, home: _home).Save(request.Input, request.BaseToken, captureId, request.Choices));
        }
        catch (InvalidOperationException e)
        {
            // 거절은 끝난 답이다 — 같은 ID 로 다시 보낼 것이 없다. 그 밖의 실패(디스크·잠금)는 남겨 같은 ID 로 다시 보낸다.
            lock (_mirrorGate) _imports.Remove(captureId);
            if (e.Message.StartsWith("미리보기 이후", StringComparison.Ordinal)) return Stale(pid, tabId);
            throw;
        }
        lock (_mirrorGate) _imports.Remove(captureId);
        // 그 탭을 다시 읽혀 지금 보는 화면이 저장된 뒤의 상태로 비추게 한다. 닿지 못해도 저장은 끝났다.
        try { ExtensionRelay.Send(_home.Directory, pid, "read", tabId); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
        return new
        {
            status = "stored",
            entity = stored.TryGetProperty("entity", out var entity) ? entity.GetString() : null,
            changed = stored.TryGetProperty("changed", out var changed) && changed.ValueKind == JsonValueKind.True,
            at = DateTime.Now.ToString("HH:mm", System.Globalization.CultureInfo.InvariantCulture),
        };
    }

    private static readonly object Gone = new
    {
        status = "gone",
        message = "이 탭이 닫혔거나 브라우저 연결이 끊겨 가져오지 않았습니다. 나라장터에서 화면을 다시 여세요.",
    };

    private object Stale(int pid, int tabId)
    {
        try { ExtensionRelay.Send(_home.Directory, pid, "read", tabId); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
        return new { status = "stale", message = "화면이나 작업자료가 바뀌었습니다. 다시 읽습니다." };
    }

    private static string Require(string?[] args, int index) =>
        args.ElementAtOrDefault(index) ?? throw new ArgumentException($"{index + 1}번째 값이 없습니다.");

    /// <summary>보표의 표본 열쇠 — JSON 글 배열. 없으면 <c>null</c>(최신 문서 셋).</summary>
    private static IReadOnlyList<string>? SampleKeys(string? json)
    {
        if (string.IsNullOrEmpty(json)) return null;
        try { return JsonSerializer.Deserialize<string[]>(json); }
        catch (JsonException) { throw new InvalidOperationException("표본 열쇠 형식이 올바르지 않습니다."); }
    }

    /// <summary>
    /// 계약면 또는 저장한 맞춤 뷰 하나를 통째로 읽어 준다. 등록된 이름만 읽으므로 아무 표나 열 수 없다.
    /// </summary>
    private object ReadSheet(string view)
    {
        var userView = view.StartsWith(Views.UserViewPrefix, StringComparison.Ordinal)
            && _store.UserViews().Any(v => Views.UserViewPrefix + v.Name == view);
        if (!Views.Names.Contains(view) && !userView)
            throw new InvalidOperationException($"내보이지 않는 표입니다: {view}");

        // 이름으로 종류를 가리는 규칙은 Views 한 군데에 둔다. 여기서 다시 짜면
        // 접수가 들어오는 순간 v_접수가 계약으로 읽혀 고친 값이 엉뚱한 표에 담긴다.
        var entityType = userView ? "contract" : Views.EntityTypeOf(view);

        // 고칠 수 없는 표인가. 목록은 Views 한 군데에 둔다 — 여기서 조건을 다시 짜면
        // 목록이 두 벌이 되고, 갈리는 순간 고칠 수 없어야 할 표가 조용히 열린다.
        //
        // 계획은 plan 이 개체가 아니라서고(field_override·user_column 의 entity_type 에 자리가
        // 없다), 차수 뷰 둘과 통합차수는 Store.FaceView 가 종류당 뷰를 하나만 알기 때문이다 —
        // 옛 차수의 칸을 고치면 덮개의 original 을 그 뷰에서 찾지 못해 빈 문자열로 박히고,
        // "무엇을 무엇으로 고쳤나" 가 오류 없이 거짓이 된다. 고치는 자리는 공고 탭·계약 탭이다.
        //
        // 열람 중이면 모든 표가 그렇다. 다리가 이미 거절하지만, 고칠 수 있는 칸으로 그려 두고 넣는 순간
        // 거절하면 사람은 고친 줄 알았다가 되돌려진다 — 처음부터 고칠 수 있어 보이지 않게 한다.
        var 읽기전용 = userView || Views.ReadOnly.Contains(view) || _viewing is not null;

        var editable = 읽기전용
            ? new HashSet<string>()
            : _store.UserColumns(entityType).Where(c => c.Kind != "computed").Select(c => c.FieldName).ToHashSet();
        var computed = 읽기전용
            ? new HashSet<string>()
            : _store.UserColumns(entityType).Where(c => c.Kind == "computed").Select(c => c.FieldName).ToHashSet();

        // 통합의 공고·접수 열은 이어진 다른 레코드의 것이라 이 줄의 키로는 주소가 잡히지 않는다.
        // 공고·접수 시트에서 고치면 같은 뷰를 타므로 여기에도 그대로 비친다.
        //
        // <b>통합차수는 정반대다.</b> 통합의 줄은 계약이라 공고·접수를 잠갔지만, 통합차수의 줄은
        // 공고라 계약·접수 쪽이 이어진 남의 레코드다 — UnifiedNoticeColumns 를 재활용할 수 없다.
        // 지금은 통합차수가 통째로 읽기 전용이라 실효가 없지만, 두 목록이 갈린 채로 두면 나중에
        // 편집을 켜는 날 아무 오류 없이 샌다.
        HashSet<string> elsewhere = view switch
        {
            "v_통합" => [.. Views.UnifiedNoticeColumns, .. Views.UnifiedRequestColumns],
            "v_통합차수" => [.. Views.ContractColumns, .. Views.UnifiedRequestColumns],
            _ => [],
        };

        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT * FROM \"{view.Replace("\"", "\"\"")}\";";

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
                                && !computed.Contains(c)
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
    /// 내므로(<c>v_공고</c>·<c>v_계약</c>과 그 위에 얹힌 <c>v_통합</c>) 그 키를 가진 줄이
    /// 없어 화면이 찾지 못하고 지나간다 — 걸러 낼 것이 없다.</para>
    ///
    /// <para><b>"그런 줄은 없다" 가 이제 표 전체에 대한 말은 아니다.</b> 차수 뷰
    /// (<c>v_공고차수</c>·<c>v_계약차수</c>·<c>v_통합차수</c>)는 옛 차수의 줄을 세운다.
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
    private object UserColumns(string? entityType)
    {
        var broken = _store.BrokenColumns();
        // 그 열을 쓰는 맞춤 뷰 수 — 지우는 확인 문구가 「맞춤 뷰 n곳이 이 열을 씁니다」 로 센다.
        var readers = new[] { "notice", "contract", "request" }.ToDictionary(e => e, _store.UserViewReaders);
        return _store.UserColumns(entityType).Select(c =>
        {
            // 가공열의 식, 또는 값으로 굳힌 입력열에 비활성으로 남은 식. 읽지 못하는 식은 깨진 열로 알린다 —
            // 목록 하나 때문에 열 관리가 통째로 열리지 않으면 고칠 길이 없다.
            object? formula = null;
            string? error = broken.GetValueOrDefault((c.EntityType, c.FieldName));
            if (c.Kind != "choice" && c.Options is { } options && options.TrimStart().StartsWith('{'))
            {
                try
                {
                    // v2 그대로 — 빈 자리를 null 로 채우면 화면이 「적힌 필드」로 읽는다.
                    formula = ComputedColumn.ReadForEditing(options);
                }
                catch (InvalidOperationException e) when (c.Kind == "computed") { error ??= e.Message; }
                catch (InvalidOperationException) { }
            }
            return new
            {
                entityType = c.EntityType,
                fieldName = c.FieldName,
                kind = c.Kind,
                choices = c.Choices,
                formula,
                broken = c.Kind == "computed" ? error : null,
                viewReaders = readers[c.EntityType].GetValueOrDefault(c.FieldName),
            };
        }).ToList();
    }

    // ── 열 고치기 ────────────────────────────────────────
    private object SaveComputedColumn(string entityType, string name, string formula, string? previousName)
    {
        _store.SaveComputedColumn(entityType, name, formula, previousName);
        return UserColumns(null);
    }
    // 고치고 나면 뷰가 다시 지어지므로, 화면은 표를 다시 읽어야 한다.

    /// <summary>값으로 굳히기. 화면이 센 수를 함께 보내면 그 사이 자료가 바뀐 경우 쓰지 않는다.</summary>
    private object FreezeColumn(string entityType, string name, string? expected)
    {
        _store.FreezeColumn(entityType, name, long.TryParse(expected, out var count) ? count : null);
        return UserColumns(null);
    }

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

    private static T ReadBatch<T>(string json) => JsonSerializer.Deserialize<T>(json, Json)
        ?? throw new InvalidOperationException("작업할 자료가 없습니다.");

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

    /// <summary>
    /// 창의 몸가짐. 알림 영역 두기는 홈의 <c>window.json</c>(<see cref="WindowPrefs"/>)에, 자동 실행은 레지스트리에
    /// 있다 — 자동 실행을 따로 적어 두지 않는 까닭은 <see cref="Autostart"/> 에 적었다.
    /// </summary>
    private object ReadWindowPrefs()
    {
        var unavailable = Autostart.Unavailable(IsDefaultHome);
        return new
        {
            closeToTray = WindowPrefs.Read(_home.WindowPrefsPath).CloseToTray,
            autostart = Autostart.IsEnabled(),
            autostartAvailable = unavailable is null,
            autostartReason = unavailable,
            // 압축 안에서 켠 창이면 켤 때 exe 를 이 자리에 복사해 두고 그것을 적는다 — 화면이 미리 알린다.
            autostartCopyTarget = unavailable is null ? Autostart.CopyTarget() : null,
        };
    }

    /// <summary>
    /// 창의 몸가짐을 적고 곧바로 세운다. 풍선을 보였는지는 그대로 둔다 — 껐다 켜도 다시 알리지 않는다.
    /// 자동 실행을 걸 수 없는 자리에서 그것을 바꾸려 하면 <b>아무것도 적기 전에</b> 까닭을 낸다.
    /// </summary>
    private object SaveWindowPrefs(bool closeToTray, bool autostart)
    {
        var changeAutostart = autostart != Autostart.IsEnabled();
        if (changeAutostart && Autostart.Unavailable(IsDefaultHome) is { } reason)
            throw new InvalidOperationException(reason);

        (WindowPrefs.Read(_home.WindowPrefsPath) with { CloseToTray = closeToTray }).Write(_home.WindowPrefsPath);
        ApplyCloseToTray?.Invoke(closeToTray);
        if (changeAutostart) Autostart.Set(autostart);
        return ReadWindowPrefs();
    }

    /// <summary>다리의 값은 글이다. 켜고 끄는 값은 <c>true</c>·<c>false</c> 둘만 받는다 — 모르는 말을 꺼짐으로 읽지 않는다.</summary>
    private static bool Flag(string value) => value switch
    {
        "true" => true,
        "false" => false,
        _ => throw new ArgumentException($"켜짐·꺼짐이 아닙니다: {value}"),
    };

    /// <summary>화면은 후보를 세로줄로 이어 보낸다 — 저장 형식과 같아 옮겨 담을 것이 없다.</summary>
    private static IReadOnlyList<string> Split(string? choices) =>
        choices is null
            ? []
            : [.. choices.Split('|', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)];



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

    private object? ExportViewPackage(string idsJson)
    {
        var json = _store.ExportViewPackage(ReadBatch<string[]>(idsJson));
        var target = PickSavePath?.Invoke(new SavePrompt("보기 구성 내보내기", "보기 구성 (*.json)|*.json", ".json", $"보기구성_{DateTime.Now:yyyyMMdd}.json"));
        if (string.IsNullOrEmpty(target)) return null;
        Writing(target, () => { File.WriteAllText(target, json, new UTF8Encoding(false)); return target; });
        return new { path = target };
    }

    private object? PickViewPackage()
    {
        var path = PickOpenJson?.Invoke("보기 구성 가져오기");
        if (string.IsNullOrEmpty(path)) return null;
        if (new FileInfo(path).Length > 2_000_000) throw new InvalidOperationException("보기 구성 파일은 2MB까지 가져올 수 있습니다.");
        var json = File.ReadAllText(path, Encoding.UTF8);
        return new { name = System.IO.Path.GetFileName(path), json, preview = _store.PreviewViewPackage(json) };
    }

    /// <summary>서식용 예시 보기를 고른 파일처럼 내민다 — 지운 것을 다시 들일 때 같은 미리보기·이름 겹침·가져오기를 탄다.</summary>
    private object? BuiltinViewPackage()
    {
        var json = Store.BuiltinViewPackage();
        return new { name = "예시 보기", json, preview = _store.PreviewViewPackage(json) };
    }

    private object? ExportSheet(string title, SheetSnapshot snapshot)
    {
        Exporter.ValidateSnapshot(title, snapshot);
        var target = PickSavePath?.Invoke(new SavePrompt(
            "현재 표를 엑셀로 내보내기", "엑셀 통합 문서 (*.xlsx)|*.xlsx", ".xlsx", $"{title}_{DateTime.Now:yyyyMMdd}.xlsx"));
        if (string.IsNullOrEmpty(target)) return null;
        try { return Exporter.ExportSheet(target, title, snapshot); }
        catch (IOException)
        {
            throw new InvalidOperationException($"파일을 쓰지 못했습니다 — 엑셀에서 열려 있다면 닫고 다시 해 주세요.  {target}");
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

        // 제출본은 자기 역할과 새 신원을 달고 나간다 — 작업자료와 같은 신원이면 받는 쪽에서 둘을 가리지 못한다.
        return new
        {
            path = Writing(target, () => PclmFile.Snapshot(_database, target, PclmRole.Submission, overwrite: true)),
            nameMissing = name.Length == 0,
        };
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

    /// <summary>확인을 받았으니 깃발을 내리고, 답을 보낸 뒤 창을 닫는다.</summary>
    private object? CloseAfterReply()
    {
        _unsaved = false;
        Interlocked.Exchange(ref _close, 1);
        return null;
    }

    private static void Reply(CoreWebView2 core, int id, bool ok, object? result, string? error) =>
        core.PostWebMessageAsJson(JsonSerializer.Serialize(new { id, ok, result, error }, Json));
}
