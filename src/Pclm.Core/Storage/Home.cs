using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Pclm.Core.Storage;

/// <summary>
/// 홈의 쪽지(<c>config.json</c>). <b>작업자료 하나를 가리키는 유일한 포인터다</b> — 창·명령줄·확장 호스트·
/// 밖의 소비자가 모두 이것을 본다(ADR-030).
///
/// <para>폴더가 아니라 <b>파일</b>을 가리킨다. 폴더 + 고정 이름이면 그 폴더에 무엇이 놓이든 그것이 자료가
/// 되고, <see cref="DatasetId"/> 가 없으면 자리에 다른 작업자료가 놓인 것을 알아챌 길이 없다.</para>
///
/// <para><see cref="Retired"/> 는 옮기기(<see cref="Home.MoveTo"/>)가 남긴 옛 자리다. 옮기는 그 순간에는 옛 파일을
/// 지울 수 없다 — 창이 아직 쥐고 있다. 다음 시작이 그 파일을 지우고 이 칸을 걷는다.</para>
/// </summary>
public sealed record HomeConfig(
    [property: JsonPropertyName("version")] int Version,
    [property: JsonPropertyName("workfile")] string Workfile,
    [property: JsonPropertyName("datasetId")] string? DatasetId,
    [property: JsonPropertyName("retired"), JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? Retired = null);

/// <summary>쪽지를 읽은 결과의 갈래.</summary>
public enum ConfigState
{
    /// <summary>쪽지가 없다 — 처음 실행이다.</summary>
    Missing,

    /// <summary>있는데 읽을 수 없다. <b>기본 자리로 물러나지 않는다</b>(ADR-030).</summary>
    Corrupt,

    Ok,
}

/// <summary><see cref="Home.ReadConfig"/> 의 결과. <paramref name="Config"/> 는 <see cref="ConfigState.Ok"/> 일 때만 찬다.</summary>
public sealed record ConfigRead(ConfigState State, HomeConfig? Config, string? Error);

/// <summary>시작할 수 없는 까닭의 갈래. 시작 화면이 무엇을 권할지 고르는 데 쓴다.</summary>
public enum ProblemKind
{
    /// <summary>쪽지가 깨졌다.</summary>
    ConfigCorrupt,

    /// <summary>쪽지가 가리키는 파일이 없다.</summary>
    WorkfileMissing,

    /// <summary>PCLM 파일이 아니다.</summary>
    NotPclm,

    /// <summary>기준선보다 옛 시험판이다.</summary>
    TooOld,

    /// <summary>이 프로그램보다 새 판이다.</summary>
    TooNew,

    /// <summary>작업자료가 아니다(제출본·취합본·백업·옮겨진 옛 자료).</summary>
    NotWork,
}

/// <summary><see cref="Home.Resolve"/> 의 결과 — 열었거나, 열지 않은 까닭이거나.</summary>
public abstract record HomeState
{
    private HomeState() { }

    /// <summary>
    /// 작업자료를 열었다. 판올림까지 마친 쓰기 손잡이다.
    /// </summary>
    /// <param name="NoticeGroupText">판올림이 건을 다시 지으며 밀어낸 것을 사람에게 알릴 글. 없으면 <c>null</c>.</param>
    /// <param name="Notes">오는 길에 있었던 일(옛 자료 옮겨 오기·백업). 화면이 열기 전에 알린다.</param>
    public sealed record Ready(Database Database, string? NoticeGroupText, IReadOnlyList<string> Notes) : HomeState;

    /// <summary>열지 않았다. <b>아무 파일도 만들지 않았다.</b> 사람이 시작 화면에서 고른다.</summary>
    public sealed record Problem(ProblemKind Kind, string? Path, string Message) : HomeState;
}

/// <summary>
/// 홈 — 앱 상태가 사는 폴더 하나. 쪽지·백업·WebView2 프로필·오류 기록·확장 출처가 여기 있다.
///
/// <para><b>홈 자체를 바꿀 수 있다</b>(<c>--home</c>). 개발·시험 격리를 이것 하나로 푼다 — 예전에는
/// <c>--db</c>(아무 파일)·<c>--erp-dev</c>(고정 DB)·업무 셋이 같은 문제를 따로 풀었고, 그 사이로
/// 확장이 엉뚱한 자료에 묶이는 길이 열려 있었다(ADR-0300).</para>
///
/// <para><b>쪽지는 창과 명령줄만 쓴다.</b> 확장 호스트는 읽기만 한다 — 브라우저가 자료 자리를 바꾸는
/// 통로가 있으면 수집 통로가 저장 구조를 고치는 통로를 겸하게 된다(ADR-032).</para>
/// </summary>
public sealed partial class Home
{
    /// <summary>처음 실행에 작업자료를 짓거나 옛 자료를 옮겨 올 이름.</summary>
    public const string DefaultWorkfileName = "계약자료.pclm";

    /// <summary>쪽지 형식의 판. 다른 판이면 깨진 것으로 본다 — 모르는 형식을 짐작해 읽지 않는다.</summary>
    public const int ConfigVersion = 1;

    /// <summary>판올림 전 백업을 몇 벌 남기나. 판올림은 드물어 열이면 충분히 거슬러 올라간다.</summary>
    public const int BackupsKept = 10;

    /// <summary>옛 자료의 파일 이름. 처음 실행에 한 번 옮겨 올 때만 본다.</summary>
    private const string LegacyFileName = "pclm.db";

    // 한글 경로를 \uXXXX 로 감추지 않는다. 쪽지는 사람이 열어 보는 파일이기도 하다 — 어느 파일을 가리키는지
    // 바로 읽혀야 한다. HTML 에 넣을 글이 아니라 덜 이스케이프해도 위험이 없다.
    private static readonly JsonSerializerOptions Json = new()
    {
        WriteIndented = true,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.Create(System.Text.Unicode.UnicodeRanges.All),
    };

    private readonly string? _legacyRoamingConfig;

    /// <param name="directory">홈 폴더. 온전한 경로로 바꿔 쥔다.</param>
    /// <param name="legacyRoamingConfig">
    /// 옛 <c>%APPDATA%\Pclm\config.json</c>. 처음 실행에 그 <c>dataDir</c> 의 자료를 옮겨 오려고 본다.
    /// 업무 홈만 넘긴다 — 개발·시험 홈이 사람의 업무 자료를 끌어오면 격리가 아니다.
    /// </param>
    public Home(string directory, string? legacyRoamingConfig = null)
    {
        Directory = Path.TrimEndingDirectorySeparator(Path.GetFullPath(directory));
        _legacyRoamingConfig = legacyRoamingConfig;
    }

    /// <summary>업무 홈. <c>%LOCALAPPDATA%\Pclm</c>.</summary>
    public static Home Default { get; } = new(
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Pclm"),
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Pclm", "config.json"));

    /// <summary>개발 홈. 개발 호스트(<c>*-erp-dev.exe</c>)와 <c>--erp-dev</c> 창이 본다. 옛 쪽지를 보지 않는다.</summary>
    public static Home Development { get; } = new(
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Pclm.Erp.Dev"));

    public string Directory { get; }

    public string ConfigPath => Path.Combine(Directory, "config.json");
    public string BackupsDirectory => Path.Combine(Directory, "backups");
    public string WebView2Directory => Path.Combine(Directory, "WebView2");
    public string ErrorLogPath => Path.Combine(Directory, "error.log");

    /// <summary>창의 몸가짐(<see cref="WindowPrefs"/>). 자료가 아니라 이 컴퓨터의 것이라 작업자료 밖, 홈에 둔다.</summary>
    public string WindowPrefsPath => Path.Combine(Directory, "window.json");

    /// <summary>확장의 출처. 내장 확장을 준비할 때 적히고, 호스트가 부른 쪽을 가린다.</summary>
    public string ExtensionOriginPath => Path.Combine(Directory, "extension-origin.txt");

    public string DefaultWorkfile => Path.Combine(Directory, DefaultWorkfileName);

    /// <summary>열어 본 파일의 임시 사본이 놓이는 자리(<see cref="PclmFile.OpenView"/>). 이 폴더는 앱의 것이다.</summary>
    public string ViewDirectory => Path.Combine(Directory, "view");

    /// <summary>열람 사본을 얼마나 오래 두나. 열린 채인 사본은 지워지지 않으므로 이 값은 넉넉하기만 하면 된다.</summary>
    public static readonly TimeSpan ViewCopiesKept = TimeSpan.FromDays(1);

    /// <summary>업무 홈인가. 확장 연결은 업무 홈에서만 준비한다 — 시험 홈이 사람의 브라우저 연결을 덮으면 안 된다.</summary>
    public bool IsDefault => SamePath(Directory, Default.Directory);

    // ── 쪽지 ─────────────────────────────────────────────────────────

    /// <summary>쪽지를 읽는다. 예외를 내지 않는다 — 깨졌으면 깨졌다고 답한다.</summary>
    public ConfigRead ReadConfig()
    {
        if (!File.Exists(ConfigPath)) return new(ConfigState.Missing, null, null);

        try
        {
            var config = JsonSerializer.Deserialize<HomeConfig>(File.ReadAllText(ConfigPath));
            if (config is null) return new(ConfigState.Corrupt, null, "비어 있습니다.");
            if (config.Version != ConfigVersion)
                return new(ConfigState.Corrupt, null, $"모르는 형식(version {config.Version})입니다.");
            if (string.IsNullOrWhiteSpace(config.Workfile))
                return new(ConfigState.Corrupt, null, "작업자료 경로(workfile)가 없습니다.");
            // 상대 경로는 받지 않는다. 창과 명령줄의 작업 폴더가 달라 같은 쪽지가 서로 다른 파일을 가리킨다.
            if (!Path.IsPathFullyQualified(config.Workfile))
                return new(ConfigState.Corrupt, null, $"작업자료 경로가 전체 경로가 아닙니다: {config.Workfile}");
            return new(ConfigState.Ok, config, null);
        }
        catch (Exception e) when (e is JsonException or IOException or UnauthorizedAccessException or NotSupportedException)
        {
            return new(ConfigState.Corrupt, null, e.Message);
        }
    }

    /// <summary>
    /// 쪽지가 지금 바로 이 파일, 이 신원을 가리키는가. 쪽지가 없거나 깨졌으면 아니다.
    /// 확장 호스트가 쓰기 잠금을 쥔 뒤에 묻는다(<see cref="Erp.ErpCapture"/>).
    /// </summary>
    public bool PointsAt(string workfile, string datasetId)
    {
        var read = ReadConfig();
        return read.State == ConfigState.Ok &&
               SamePath(read.Config!.Workfile, Path.GetFullPath(workfile)) &&
               read.Config.DatasetId == datasetId;
    }

    /// <summary>
    /// 쪽지를 쓴다. <b>같은 폴더의 임시 파일에 쓴 뒤 이름을 바꾼다</b> — 쓰다 끊기면 반쯤 쓴 쪽지가 남아,
    /// 다음 실행이 그것을 깨진 쪽지로 보고 멈춘다. 이름 바꾸기는 같은 볼륨 안에서 한 번에 일어난다.
    /// </summary>
    /// <param name="retired">옮기기가 남긴 옛 자리. 다음 시작이 지운다(<see cref="Resolve"/>).</param>
    public void WriteConfig(string workfile, string datasetId, string? retired = null)
    {
        System.IO.Directory.CreateDirectory(Directory);
        var config = new HomeConfig(ConfigVersion, Path.GetFullPath(workfile), datasetId,
            retired is null ? null : Path.GetFullPath(retired));
        var temporary = ConfigPath + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllText(temporary, JsonSerializer.Serialize(config, Json));
            File.Move(temporary, ConfigPath, overwrite: true);
        }
        finally
        {
            if (File.Exists(temporary)) File.Delete(temporary);
        }
    }

    // ── 시작 ─────────────────────────────────────────────────────────

    /// <summary>
    /// 지금 쓸 작업자료를 정해 연다(ADR-030). <b>어떤 연결보다 먼저 부른다.</b>
    ///
    /// <para>빈 자료가 서는 것은 <b>쪽지가 없는 처음 실행</b>뿐이다. 쪽지가 깨졌거나 가리키는 파일이 없거나
    /// 작업자료가 아니면 아무것도 만들지 않고 <see cref="HomeState.Problem"/> 을 낸다 — 거기서 빈 자료를
    /// 세우면 경로 하나 어긋난 것만으로 사람의 자료가 사라진 것처럼 보이고, 확장까지 그 빈 자료에 쌓인다.</para>
    ///
    /// <para>예상한 상태에서는 예외를 내지 않는다. 판올림 실패 같은 뜻밖의 일만 그대로 올라간다.</para>
    /// </summary>
    public HomeState Resolve()
    {
        var read = ReadConfig();
        if (read.State == ConfigState.Missing) return FirstRun();
        if (read.State == ConfigState.Corrupt)
            return new HomeState.Problem(ProblemKind.ConfigCorrupt, ConfigPath,
                $"작업자료를 가리키는 설정 파일을 읽을 수 없습니다({read.Error}). " +
                "다른 자료를 대신 열지 않습니다 — 작업자료 파일을 찾아 주거나 새로 만드세요.");

        var notes = new List<string>();
        var state = Open(read.Config!.Workfile, read.Config.DatasetId, notes, backup: true);

        // 옮기기가 남긴 옛 파일은 새 자리가 열린 뒤에만 지운다. 새 자리가 열리지 않으면 옛 파일이 남은
        // 한 벌일 수 있다 — 그때는 쪽지에 그대로 두어 다음 시작이 다시 본다.
        if (state is HomeState.Ready ready && read.Config.Retired is { } retired)
        {
            DiscardRetired(retired, notes);
            WriteConfig(ready.Database.Path, DatasetIdOf(ready.Database.Path));
        }
        return state;
    }

    /// <summary>
    /// 옮기기 전 자리의 옛 파일을 지운다. <b>역할이 <c>retired</c> 인 PCLM 일 때만</b> — 그 사이 사람이
    /// 그 자리에 다른 자료를 놓았으면 그것은 남의 것이다. 못 지우면 둔다: 열어도 열람뿐이라 해가 없다.
    /// </summary>
    private static void DiscardRetired(string path, List<string> notes)
    {
        var info = PclmFile.Inspect(path);
        if (info is not { Kind: PclmKind.Ok, Role: PclmRole.Retired }) return;

        try
        {
            // 본체를 먼저 지운다. 본체가 남았는데 -wal 만 사라지면 커밋된 retired 표시를 잃을 수 있다.
            File.Delete(path);
            foreach (var suffix in new[] { "-wal", "-shm" })
                File.Delete(path + suffix);
            notes.Add($"옮기기 전 자리의 옛 파일을 지웠습니다: {path}");
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            // 다른 프로그램이 쥐고 있다. 옮긴 뒤의 옛 파일이라 그대로 두어도 아무도 거기에 쓰지 않는다.
        }
    }

    /// <summary>
    /// 처음 실행. 옛 자료가 있으면 사본을 떠 오고, 홈에 이미 작업자료가 있으면 그것을 쓰고, 둘 다 없으면 짓는다.
    /// </summary>
    private HomeState FirstRun()
    {
        var notes = new List<string>();

        // 임시 규칙: 표지가 생기기 전의 자료(pclm.db)를 한 번 건네받는다. 1.0 전에 걷는다(ADR-031).
        // 원본은 그대로 둔다 — 옮긴 사본이 잘못됐을 때 돌아갈 자리이고, 옛 판 프로그램은 여전히 그것을 연다.
        // 홈에 이미 작업자료가 있으면 덮지 않고 그것을 쓴다(아래).
        if (!File.Exists(DefaultWorkfile))
        {
            foreach (var candidate in LegacyCandidates())
            {
                var info = PclmFile.Inspect(candidate);
                if (info.Kind == PclmKind.NotFound) continue;

                // 옛 자료가 있는데 받을 수 없다. 빈 자료를 새로 세우면 사람은 자료가 사라진 줄 안다 —
                // 멈추고 무엇이 왜 안 되는지 보인다.
                if (info is not { Kind: PclmKind.Ok, Role: PclmRole.Work })
                    return Refuse(info, candidate);

                CopyLegacy(candidate);
                notes.Add($"옛 자료를 옮겨 왔습니다.\n{candidate}\n→ {DefaultWorkfile}\n(원본은 그대로 남아 있습니다)");

                // 원본이 백업 노릇을 하므로 사본을 다시 백업하지 않는다.
                return Open(DefaultWorkfile, null, notes, backup: false);
            }
        }

        if (File.Exists(DefaultWorkfile))
            return Open(DefaultWorkfile, null, notes, backup: true);

        // 빈 자료가 서는 두 길 중 하나다(다른 하나는 시작 화면의 「새로 만들기」). ADR-031.
        var created = PclmFile.Create(DefaultWorkfile, PclmRole.Work);
        WriteConfig(created.Path, DatasetIdOf(created.Path));
        return new HomeState.Ready(created, null, notes);
    }

    /// <summary>옛 자료가 있을 수 있는 자리. 먼저 오는 것이 이긴다.</summary>
    private IEnumerable<string> LegacyCandidates()
    {
        if (_legacyRoamingConfig is not null)
            foreach (var folder in ReadLegacyFolders(_legacyRoamingConfig))
                yield return Path.Combine(folder, LegacyFileName);

        // 업무 홈의 기본 자리이자 옛 개발 DB(Pclm.Erp.Dev\pclm.db)의 자리다.
        yield return Path.Combine(Directory, LegacyFileName);
    }

    /// <summary>
    /// 옛 쪽지가 가리키는 폴더. 이사가 밀려 있었으면(<c>pendingMoveFrom</c>) 자료는 아직 옛 자리에 있으므로
    /// 그것이 먼저다 — <c>dataDir</c> 만 보면 빈 새 자리를 보고 빈 자료를 세운다.
    /// 없거나 깨졌으면 아무것도 내지 않는다 — 옛 쪽지 때문에 멈출 까닭은 없다.
    /// </summary>
    private static IEnumerable<string> ReadLegacyFolders(string path)
    {
        var folders = new List<string>();
        try
        {
            if (!File.Exists(path)) return folders;
            using var document = JsonDocument.Parse(File.ReadAllText(path));
            if (document.RootElement.ValueKind != JsonValueKind.Object) return folders;

            foreach (var key in new[] { "pendingMoveFrom", "dataDir" })
                if (document.RootElement.TryGetProperty(key, out var value) &&
                    value.ValueKind == JsonValueKind.String &&
                    Path.IsPathFullyQualified(value.GetString()!))
                    folders.Add(value.GetString()!);
        }
        catch (Exception e) when (e is JsonException or IOException or UnauthorizedAccessException or ArgumentException)
        {
        }
        return folders;
    }

    /// <summary>
    /// 옛 자료를 홈의 작업자료 자리에 한 벌 뜬다. <b>임시 이름으로 뜬 뒤 제자리 이름으로 바꾼다</b>(ADR-031) —
    /// 뜨다 끊기면 반쪽 파일이 작업자료 자리에 서서 다음 실행을 막는다.
    ///
    /// <para><see cref="PclmFile.Snapshot"/> 이 아니라 <see cref="Database.Snapshot"/> 이다. 옛 v19 파일에는 아직
    /// <c>pclm_file</c> 이 없고, 이것은 사본이 아니라 <b>옮겨 오기</b>라 신원(<c>dataset_id</c>)을 이어 간다 —
    /// 그래야 확장의 열린 검토 화면이 끊기지 않는다.</para>
    /// </summary>
    private void CopyLegacy(string source)
    {
        var temporary = DefaultWorkfile + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            new Database(source, Access.Read).Snapshot(temporary);
            File.Move(temporary, DefaultWorkfile, overwrite: false);
        }
        finally
        {
            if (File.Exists(temporary)) File.Delete(temporary);
        }
    }

    /// <summary>
    /// 작업자료로 연다. 판이 낮으면 백업을 뜨고 올린다. 쪽지의 신원이 파일과 다르면 쪽지를 고친다.
    /// </summary>
    private HomeState Open(string path, string? configDatasetId, List<string> notes, bool backup)
    {
        var full = Path.GetFullPath(path);
        var info = PclmFile.Inspect(full);
        if (info is not { Kind: PclmKind.Ok, Role: PclmRole.Work }) return Refuse(info, full);

        if (backup && info.Version < Schema.Version)
            notes.Add($"판을 올리기 전에 백업을 떴습니다(v{info.Version} → v{Schema.Version}).\n{Backup(full, info.Version)}");

        var database = PclmFile.OpenWork(full);
        var change = database.Migrate();
        var datasetId = DatasetIdOf(full);

        // 쪽지의 신원과 다르면 그 자리에 다른 작업자료가 놓인 것이다. 쪽지를 파일의 것으로 고친다 —
        // 확장 호스트는 쪽지의 신원으로 대상을 확인하므로, 이로써 옛 자료를 보고 연 검토 화면의 저장이 거절된다.
        if (configDatasetId != datasetId || !SamePath(ReadConfig().Config?.Workfile, full))
            WriteConfig(full, datasetId);

        return new HomeState.Ready(database, NoticeGroupText.Render(change), notes);
    }

    /// <summary>
    /// 판올림 전 백업. 판올림은 되돌릴 수 없어서 뜬다(ADR-033). 최근 <see cref="BackupsKept"/> 벌만 남긴다.
    /// </summary>
    /// <returns>뜬 자리.</returns>
    private string Backup(string path, int version)
    {
        System.IO.Directory.CreateDirectory(BackupsDirectory);

        var stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss", System.Globalization.CultureInfo.InvariantCulture);
        var target = Path.Combine(BackupsDirectory, $"계약자료_v{version}_{stamp}.pclm");
        for (var n = 2; File.Exists(target); n++)
            target = Path.Combine(BackupsDirectory, $"계약자료_v{version}_{stamp}-{n}.pclm");

        // 표지가 생기기 전의 v19 에는 pclm_file 이 없어 역할을 적을 자리가 없다 — 날것 그대로 뜬다.
        // 이 갈래는 v19 임시 규칙과 함께 걷는다.
        var source = new Database(path, Access.Read);
        var saved = version == Schema.BaselineVersion
            ? source.Snapshot(target)
            : PclmFile.Snapshot(source, target, PclmRole.Backup, overwrite: false);

        Prune();
        return saved;
    }

    /// <summary>오래된 백업을 지운다. 우리가 지은 이름의 것만 — 사람이 거기 둔 다른 파일은 건드리지 않는다.</summary>
    private void Prune()
    {
        var old = new DirectoryInfo(BackupsDirectory)
            .GetFiles("계약자료_v*_*.pclm")
            .OrderByDescending(f => f.LastWriteTimeUtc)
            .ThenByDescending(f => f.Name, StringComparer.Ordinal)
            .Skip(BackupsKept);

        foreach (var file in old)
        {
            try { file.Delete(); }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { /* 다음 판올림에서 다시 지운다 */ }
        }
    }

    // ── 열람 ─────────────────────────────────────────────────────────

    /// <summary>
    /// 쪽지가 지금 가리키는 작업자료가 이 파일인가. 쪽지가 없거나 깨졌으면 아니다.
    /// 창이 <c>--view</c> 로 받은 파일이 내 작업자료면 열람이 아니라 보통으로 연다.
    /// </summary>
    public bool IsActiveWorkfile(string path)
    {
        var read = ReadConfig();
        return read.State == ConfigState.Ok && SamePath(read.Config!.Workfile, path);
    }

    /// <summary>
    /// 남은 열람 사본을 걷는다. 창이 죽어 <see cref="ViewedFile.Dispose"/> 를 못 지난 것들이다.
    ///
    /// <para><see cref="ViewCopiesKept"/> 보다 오래된 것만 지운다 — 지금 떠 있는 다른 열람 창의 사본을
    /// 걷으면 그 창이 읽던 자리가 사라진다. 열린 채인 파일은 지워지지 않아 그대로 넘어간다.</para>
    /// </summary>
    public void PruneViews()
    {
        if (!System.IO.Directory.Exists(ViewDirectory)) return;

        var cutoff = DateTime.UtcNow - ViewCopiesKept;
        foreach (var file in new DirectoryInfo(ViewDirectory).GetFiles())
        {
            if (file.LastWriteTimeUtc >= cutoff) continue;
            try { file.Delete(); }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { /* 열린 채다. 다음에 다시 본다 */ }
        }
    }

    // ── 시작 화면의 두 동작 ───────────────────────────────────────────

    /// <summary>
    /// 있는 작업자료를 고른다(시작 화면 「파일 찾기」). <b>PCLM 이고 역할이 <c>work</c> 인 것만</b> 받는다 —
    /// 제출본·취합본·백업을 그 자리에서 쓰면 보낸 것과 받은 것이 조용히 갈린다.
    /// </summary>
    /// <exception cref="PclmFileException">받을 수 없는 파일.</exception>
    /// <exception cref="InvalidOperationException">네트워크 자리.</exception>
    public void Adopt(string path)
    {
        var full = Path.GetFullPath(path);
        RequireLocal(full);

        var info = RequireWork(full);
        WriteConfig(full, info.DatasetId!);
    }

    /// <summary>
    /// 새 작업자료를 짓는다(시작 화면 「새로 만들기」). 그 자리에 파일이 있으면 덮지 않고 실패한다.
    /// </summary>
    /// <returns>지은 파일의 온전한 경로.</returns>
    public string CreateNew(string path)
    {
        var full = Path.GetFullPath(path);
        RequireLocal(full);

        var database = PclmFile.Create(full, PclmRole.Work);
        WriteConfig(database.Path, DatasetIdOf(database.Path));
        return database.Path;
    }

    /// <summary>
    /// 작업자료는 로컬 디스크에만 둔다. WAL 은 네트워크 파일 시스템에서 동작하지 않는다(ADR-006) —
    /// 공유 메모리(<c>-shm</c>)를 여러 기계가 나눠 쓸 수 없어, 열리기는 해도 잠금이 조용히 어긋난다.
    /// </summary>
    /// <exception cref="InvalidOperationException">네트워크 자리.</exception>
    public static void RequireLocal(string path)
    {
        var full = Path.GetFullPath(path);
        const string why = "작업자료는 이 컴퓨터의 디스크에 두어야 합니다 — 네트워크 자리에서는 SQLite 의 WAL 이 동작하지 않습니다";

        if (full.StartsWith(@"\\", StringComparison.Ordinal) && !full.StartsWith(@"\\?\", StringComparison.Ordinal) ||
            full.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException($"{why}: {full}");

        var root = Path.GetPathRoot(full);
        if (string.IsNullOrEmpty(root)) return;
        try
        {
            if (new DriveInfo(root).DriveType == DriveType.Network)
                throw new InvalidOperationException($"{why}: {full}");
        }
        catch (ArgumentException)
        {
            // 드라이브 문자로 볼 수 없는 뿌리. 위의 UNC 거르기가 맡는다.
        }
    }

    // ── 홈 잠금 ──────────────────────────────────────────────────────

    /// <summary>
    /// 이 홈을 한 창만 쥐게 한다(ADR-030). 이미 누가 쥐었으면 <c>null</c>.
    ///
    /// <para>이름 붙은 뮤텍스가 <b>있는가</b>만 본다 — 소유는 따지지 않는다. 소유는 실마리에 매여 같은 실마리가
    /// 다시 얻으면 그냥 통과하고, 놓는 것도 얻은 실마리여야 한다. 손잡이가 열려 있는 동안 이름이 살아 있고,
    /// 프로세스가 죽으면 운영체제가 손잡이를 닫아 잠금도 풀린다.</para>
    ///
    /// <para><paramref name="wait"/> 동안은 다시 본다. 작업자료를 바꾼 창은 스스로를 다시 띄우는데(ADR-032 의 5),
    /// 나가는 창이 잠금을 놓고 끝나는 사이에 새 창이 먼저 물으면 "이미 열려 있다" 로 멈춘다.</para>
    /// </summary>
    public IDisposable? TryLock(TimeSpan wait = default)
    {
        var until = DateTime.UtcNow + wait;
        while (true)
        {
            var mutex = new Mutex(initiallyOwned: false, LockName, out var createdNew);
            if (createdNew) return mutex;

            mutex.Dispose();
            if (DateTime.UtcNow >= until) return null;
            Thread.Sleep(100);
        }
    }

    /// <summary>
    /// 경로에서 뽑은 고정 이름. 경로를 그대로 쓰지 않는 까닭은 뮤텍스 이름에 <c>\</c> 를 쓸 수 없어서고,
    /// 대소문자를 접는 까닭은 Windows 경로가 대소문자를 가리지 않아서다.
    /// </summary>
    private string LockName => @"Local\Pclm-" + DirectoryKey;

    /// <summary>
    /// 이 홈의 창을 앞으로 불러내는 신호의 이름. 창이 알림 영역에 숨어 있으면 두 번째로 띄운 사람이 「이미 열려 있다」
    /// 만 듣고 창을 찾을 길이 없다 — 그래서 잠금을 얻지 못한 쪽이 이 신호를 울려 떠 있는 창을 앞으로 부른다.
    /// 잠금과 같은 열쇠를 쓰므로 같은 홈끼리만 닿는다.
    /// </summary>
    public string ActivationName => @"Local\Pclm.Show." + DirectoryKey;

    private string DirectoryKey =>
        Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(Directory.ToUpperInvariant())), 0, 16);

    // ── 도움 ─────────────────────────────────────────────────────────

    private static HomeState.Problem Refuse(PclmInfo info, string path)
    {
        var (kind, message) = info.Kind switch
        {
            PclmKind.NotFound => (ProblemKind.WorkfileMissing,
                $"작업자료 파일이 없습니다. 지워졌거나 옮겨졌거나 이름이 바뀌었을 수 있습니다: {path}"),
            PclmKind.NotPclm => (ProblemKind.NotPclm, $"계약 목록 자료(PCLM) 파일이 아닙니다: {path}"),
            PclmKind.TooOld => (ProblemKind.TooOld,
                $"정식판 이전의 옛 시험판(v{info.Version})으로 지은 자료라 이 프로그램이 올리지 못합니다. " +
                $"0.7.0 으로 한 번 열어 v{Schema.BaselineVersion} 로 올린 뒤 다시 여세요: {path}"),
            PclmKind.TooNew => (ProblemKind.TooNew,
                $"이 프로그램보다 새 판(v{info.Version})으로 지은 자료입니다. 프로그램을 새로 받아 여세요: {path}"),
            _ => (ProblemKind.NotWork,
                $"작업자료가 아니라 {PclmRole.Name(info.Role)}입니다 — 그 자리에서 고치지 않습니다: {path}"),
        };
        return new HomeState.Problem(kind, path, message);
    }

    private static string DatasetIdOf(string path) =>
        PclmFile.Inspect(path).DatasetId
        ?? throw new InvalidOperationException($"작업자료의 신원(dataset_id)을 읽지 못했습니다: {path}");

    private static bool SamePath(string? a, string? b) =>
        a is not null && b is not null &&
        string.Equals(
            Path.TrimEndingDirectorySeparator(Path.GetFullPath(a)),
            Path.TrimEndingDirectorySeparator(Path.GetFullPath(b)),
            StringComparison.OrdinalIgnoreCase);
}
