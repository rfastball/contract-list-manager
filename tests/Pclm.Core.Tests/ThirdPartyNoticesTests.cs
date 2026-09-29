using System.Text.Json;
using System.Text.RegularExpressions;
using System.Xml.Linq;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 배포 exe 에 실리는 남의 것과 <c>THIRD-PARTY-NOTICES.txt</c> 를 대조한다.
///
/// <para>고지는 손으로 쓴다. 그래서 의존을 더하거나 판을 올리면 고지만 옛것으로 남아도 아무것도
/// 실패하지 않았다 — exe 에는 새 라이브러리가 실려 나가는데 그 라이선스는 어디에도 없다.
/// 여기서는 <b>실제 해석 결과를 원천으로</b> 삼아 양쪽을 맞춘다: 실린 것이 고지에 없어도,
/// 고지에 적힌 것이 더는 실리지 않아도 붉어진다. 수를 박아 두지 않는다 — 판이 오르면
/// 붉어지는 것이 의도다.</para>
///
/// <para>원천: NuGet 은 <c>src/Pclm.App/obj/project.assets.json</c>(전이 포함), npm 은
/// <c>package-lock.json</c> 의 운영 의존, 런타임 팩은 assets 를 지은 SDK 의
/// <c>Microsoft.NETCoreSdk.BundledVersions.props</c> — 자체 포함 publish 가 싣는 판이 거기 적혀 있다.</para>
/// </summary>
public class ThirdPartyNoticesTests
{
    /// <summary>
    /// 빌드에만 쓰이고 exe 에 실리지 않는 패키지. ILLink.Tasks 는 SDK 가 자동으로 걸어 두는
    /// 트리밍 도구다(suppressParent=All) — 고지에 적을 까닭이 없다.
    /// </summary>
    private static readonly HashSet<string> BuildOnly = ["Microsoft.NET.ILLink.Tasks"];

    /// <summary>publish.ps1 의 기본 런타임.</summary>
    private const string Rid = "win-x64";

    private static readonly Regex Head = new(@"^== (runtime|nuget|npm|font|native) (\S+) (\S+) — .+$", RegexOptions.Multiline);

    private static readonly string Root = FindRoot();

    private static string FindRoot()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory); directory is not null; directory = directory.Parent)
            if (File.Exists(Path.Combine(directory.FullName, "Pclm.sln"))) return directory.FullName;
        throw new InvalidOperationException("저장소 뿌리(Pclm.sln)를 찾지 못했습니다.");
    }

    /// <summary>고지에 적힌 항목. 갈래 → "이름 판".</summary>
    private static ILookup<string, string> Notices()
    {
        var text = File.ReadAllText(Path.Combine(Root, "THIRD-PARTY-NOTICES.txt"));
        return Head.Matches(text).ToLookup(m => m.Groups[1].Value, m => $"{m.Groups[2].Value} {m.Groups[3].Value}");
    }

    private static JsonElement Assets()
    {
        var path = Path.Combine(Root, "src", "Pclm.App", "obj", "project.assets.json");
        Assert.True(File.Exists(path),
            $"{path} 가 없습니다. 먼저 `dotnet restore src/Pclm.App` 를 돌리세요 — 실제로 실리는 NuGet 목록이 거기서 나옵니다.");
        return JsonDocument.Parse(File.ReadAllText(path)).RootElement;
    }

    private static void Same(IEnumerable<string> shipped, IEnumerable<string> written, string kind)
    {
        var missing = shipped.Except(written).Order().ToList();
        var stale = written.Except(shipped).Order().ToList();
        Assert.True(missing.Count == 0 && stale.Count == 0,
            $"THIRD-PARTY-NOTICES.txt 의 {kind} 항목이 실제와 어긋납니다.\n" +
            $"  고지에 없음: {string.Join(", ", missing)}\n" +
            $"  더는 실리지 않음: {string.Join(", ", stale)}\n" +
            "고지 파일을 함께 고치세요(이름·판·라이선스·저작권 줄).");
    }

    [Fact]
    public void NuGet_패키지가_이름과_판으로_모두_적혀_있고_적힌_것은_모두_실린다()
    {
        var shipped = Assets().GetProperty("libraries").EnumerateObject()
            .Where(p => p.Value.GetProperty("type").GetString() == "package")
            .Select(p => p.Name.Split('/'))
            .Where(p => !BuildOnly.Contains(p[0]))
            .Select(p => $"{p[0]} {p[1]}");

        Same(shipped, Notices()["nuget"], "nuget");
    }

    /// <summary>
    /// assets 의 <c>downloadDependencies</c> 는 RID 를 주고 복원했을 때만 런타임 팩을 적는다 —
    /// 그냥 <c>dotnet build</c> 한 뒤에는 비어 시험이 복원 방식에 따라 갈린다. 그래서 복원한 SDK 의
    /// 판 목록에서, 앱이 실제로 거는 프레임워크 참조마다 publish 가 고를 판을 찾는다.
    /// </summary>
    [Fact]
    public void 런타임_팩의_판이_고지와_같다()
    {
        var framework = Assets().GetProperty("project").GetProperty("frameworks").EnumerateObject().Single();
        var tfm = framework.Name.Split('-')[0];
        var sdk = Path.GetDirectoryName(framework.Value.GetProperty("runtimeIdentifierGraphPath").GetString())!;
        var props = Path.Combine(sdk, "Microsoft.NETCoreSdk.BundledVersions.props");
        Assert.True(File.Exists(props), $"{props} 가 없습니다. 이 기계의 SDK 로 `dotnet restore src/Pclm.App` 를 다시 돌리세요.");

        var known = XDocument.Load(props).Descendants()
            .Where(e => e.Name.LocalName == "KnownFrameworkReference" && (string?)e.Attribute("TargetFramework") == tfm)
            .ToDictionary(e => (string)e.Attribute("Include")!);
        var shipped = framework.Value.GetProperty("frameworkReferences").EnumerateObject()
            .Select(r => known[r.Name])
            .Select(k => $"{((string)k.Attribute("RuntimePackNamePatterns")!).Replace("**RID**", Rid)} {(string)k.Attribute("LatestRuntimeFrameworkVersion")!}")
            .Distinct();

        Same(shipped, Notices()["runtime"], "runtime");
    }

    [Fact]
    public void npm_운영_의존이_이름과_판으로_모두_적혀_있고_적힌_것은_모두_실린다()
    {
        using var lockFile = JsonDocument.Parse(File.ReadAllText(Path.Combine(Root, "package-lock.json")));
        const string prefix = "node_modules/";
        var shipped = lockFile.RootElement.GetProperty("packages").EnumerateObject()
            .Where(p => p.Name.StartsWith(prefix, StringComparison.Ordinal))
            .Where(p => !Flag(p.Value, "dev") && !Flag(p.Value, "devOptional"))
            .Select(p => $"{p.Name[(p.Name.LastIndexOf(prefix, StringComparison.Ordinal) + prefix.Length)..]} {p.Value.GetProperty("version").GetString()}");

        Same(shipped, Notices()["npm"], "npm");

        static bool Flag(JsonElement e, string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.True;
    }

    [Fact]
    public void 화면에_싣는_글꼴마다_고지가_있다()
    {
        var fonts = Path.Combine(Root, "frontend", "fonts");
        var shipped = Directory.Exists(fonts)
            ? Directory.GetFiles(fonts, "*.woff2").Select(f => Path.GetFileName(f)).ToList()
            : [];
        var written = Notices()["font"].Select(e => e.Split(' ')[0]).ToList();

        Assert.All(shipped, f => Assert.Contains(f, written));
        Assert.All(written, f => Assert.Contains(f, shipped));
    }
}
