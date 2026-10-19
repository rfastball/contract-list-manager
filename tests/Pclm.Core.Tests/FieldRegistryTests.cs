using Pclm.Core.Erp;

namespace Pclm.Core.Tests;

/// <summary>
/// 수집 필드 목록(<c>docs/erp/fields.md</c>)과 기본 매핑을 대조한다(ADR-043).
///
/// <para>필드가 지금 무엇인지는 문서 셋(전수 표 · 질문 문서의 확정 기록 · 옛 일괄 제외)에 「뒤의 기록이 앞을 이긴다」 로
/// 겹쳐 있었고, 매핑에 필드를 더하거나 빼도 어느 문서도 따라오지 않아 아무것도 실패하지 않았다. 이제 목록은 한 벌이고,
/// 매핑에 있는데 <c>수집</c> 이 아니거나 <c>수집</c> 인데 매핑에 없으면 여기서 붉어진다.</para>
/// </summary>
public class FieldRegistryTests
{
    private static readonly string[] Statuses = ["수집", "제외: 개인정보", "제외: 다른 출처", "제외: 뜻 없음", "후보"];

    /// <summary>
    /// 뜻이 확정되어 <c>수집</c> 인데 아직 매핑에 담지 않은 필드. 줄기만 한다 — 매핑에 담으면 여기서 지운다(남겨 두면 붉어진다).
    /// 새 항목을 여기 더하는 것은 「확정이지만 담지 않는다」 를 다시 여는 일이라, 목록에 <c>수집</c> 으로 적는 커밋에서 매핑도 함께 고친다.
    /// </summary>
    private static readonly (string Profile, string Source, string Id)[] NotYetMapped = [];

    private const string Header = "| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |";

    private sealed record Row(int Line, string Profile, string Source, string Id, string Status)
    {
        public (string, string, string) Key => (Profile, Source, Id);
    }

    private static readonly string Root = FindRoot();

    private static string FindRoot()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory); directory is not null; directory = directory.Parent)
            if (File.Exists(Path.Combine(directory.FullName, "Pclm.sln"))) return directory.FullName;
        throw new InvalidOperationException("저장소 뿌리(Pclm.sln)를 찾지 못했습니다.");
    }

    /// <summary>머리줄이 <see cref="Header"/> 인 표의 행만 읽는다 — 상태 설명 같은 다른 표는 건너뛴다.</summary>
    private static List<Row> Registry()
    {
        var lines = File.ReadAllLines(Path.Combine(Root, "docs", "erp", "fields.md"));
        var rows = new List<Row>();
        var inTable = false;
        for (var i = 0; i < lines.Length; i++)
        {
            var line = lines[i].Trim();
            if (line == Header) { inTable = true; i++; continue; } // 구분줄을 건너뛴다
            if (!inTable) continue;
            if (!line.StartsWith('|')) { inTable = false; continue; }
            var cells = line.Trim('|').Split('|').Select(c => c.Trim().Trim('`')).ToArray();
            Assert.True(cells.Length == 6, $"fields.md {i + 1}행: 칸이 여섯이 아닙니다 — {line}");
            rows.Add(new(i + 1, cells[0], cells[1], cells[2], cells[4]));
        }
        Assert.NotEmpty(rows);
        return rows;
    }

    /// <summary>
    /// 기본 매핑이 읽는 (프로필, 출처, ID). 머리 필드는 <c>pointInfo</c>, <c>tables.&lt;표&gt;.&lt;행&gt;.&lt;ID&gt;</c> 는 그 표다.
    /// 식별 필드(baseField·seqField·표 keys)와 secondarySource 도 넣는다 — 매핑이 읽어 문서 번호·행 키·시각으로 담는 값이라,
    /// 빼면 목록에 없는 칸이 조용히 담긴다.
    /// </summary>
    private static HashSet<(string, string, string)> Mapped()
    {
        var keys = new HashSet<(string, string, string)>();
        foreach (var p in Mapping.Defaults().Profiles)
        {
            foreach (var f in p.Fields)
            {
                keys.Add(Head(p.Id, f.Source));
                if (f.SecondarySource is not null) keys.Add(Head(p.Id, f.SecondarySource));
            }
            keys.Add((p.Id, p.IdentitySource, p.BaseField));
            keys.Add((p.Id, p.IdentitySource, p.SeqField));
            foreach (var t in p.Tables)
            {
                foreach (var f in t.Fields)
                {
                    keys.Add((p.Id, t.Source, f.Source));
                    if (f.SecondarySource is not null) keys.Add((p.Id, t.Source, f.SecondarySource));
                }
                foreach (var k in t.Keys) keys.Add((p.Id, t.Source, k));
            }
        }
        return keys;

        static (string, string, string) Head(string profile, string source)
        {
            if (!source.StartsWith("tables.")) return (profile, "pointInfo", source);
            var parts = source.Split('.');
            return (profile, parts[1], string.Join('.', parts[3..]));
        }
    }

    private static string Show((string Profile, string Source, string Id) k) => $"{k.Profile} · {k.Source} · {k.Id}";

    [Fact]
    public void 매핑이_읽는_필드는_모두_수집으로_적혀_있다()
    {
        var collected = Registry().Where(r => r.Status == "수집").Select(r => r.Key).ToHashSet();
        var missing = Mapped().Where(k => !collected.Contains(k)).Select(Show).Order().ToList();
        Assert.True(missing.Count == 0,
            "매핑이 읽는데 fields.md 에 「수집」 으로 없는 필드:\n  " + string.Join("\n  ", missing) +
            "\n목록에 「수집」 으로 적거나, 뜻이 확정되지 않았으면 매핑에서 빼세요(ADR-043).");
    }

    [Fact]
    public void 수집으로_적힌_필드는_매핑에_있거나_아직_담지_않은_목록에_있다()
    {
        var mapped = Mapped();
        var pending = NotYetMapped.ToHashSet();
        var stray = Registry().Where(r => r.Status == "수집" && !mapped.Contains(r.Key) && !pending.Contains(r.Key))
            .Select(r => $"{r.Line}행 {Show(r.Key)}").ToList();
        Assert.True(stray.Count == 0,
            "fields.md 에 「수집」 인데 매핑에 없는 필드:\n  " + string.Join("\n  ", stray) +
            "\n매핑에 담으세요. 확정이면 담는다(ADR-043).");
    }

    [Fact]
    public void 아직_담지_않은_목록은_줄기만_한다()
    {
        var mapped = Mapped();
        var collected = Registry().Where(r => r.Status == "수집").Select(r => r.Key).ToHashSet();
        var done = NotYetMapped.Where(mapped.Contains).Select(Show).ToList();
        Assert.True(done.Count == 0, "이미 매핑에 담겼으니 NotYetMapped 에서 지우세요:\n  " + string.Join("\n  ", done));
        var orphan = NotYetMapped.Where(k => !collected.Contains(k)).Select(Show).ToList();
        Assert.True(orphan.Count == 0, "fields.md 에 「수집」 으로 없는 NotYetMapped 항목:\n  " + string.Join("\n  ", orphan));
        Assert.Equal(NotYetMapped.Length, NotYetMapped.Distinct().Count());
    }

    /// <summary>
    /// 같은 문자열의 ID 는 어느 화면·표에서도 같은 뜻이다(2026-10-15 사용자 원칙, ADR-043 결정 6). 뜻은 ID 단위로 정해지고
    /// 자리마다 정하는 것은 그 자리 값이 사본인지(<c>제외: 다른 출처</c>)뿐이라, 한 자리에서 확정한 ID 가 다른 자리에서
    /// 후보이거나 뜻 없음이면 둘 중 하나가 틀렸다.
    /// </summary>
    [Fact]
    public void 한_ID_에는_뜻이_하나다()
    {
        var split = Registry().GroupBy(r => r.Id)
            .Where(g => g.Any(r => r.Status == "수집") && g.Any(r => r.Status is "후보" or "제외: 뜻 없음"))
            .Select(g => $"{g.Key} — " + string.Join(", ", g.Where(r => r.Status is "수집" or "후보" or "제외: 뜻 없음")
                .Select(r => $"{r.Line}행 {r.Profile} · {r.Source} 「{r.Status}」"))).ToList();
        Assert.True(split.Count == 0,
            "같은 ID 가 한 자리에서는 「수집」 이고 다른 자리에서는 「후보」·「제외: 뜻 없음」 입니다:\n  " + string.Join("\n  ", split) +
            "\n같은 ID 는 어느 화면·표에서도 같은 뜻이다 — 한 번 확정한 ID 는 다른 자리에서도 확정이고, 자리마다 정하는 것은" +
            " 사본인지(「제외: 다른 출처」)뿐이다(ADR-043 결정 6).");
    }

    [Fact]
    public void 상태는_다섯_가지이고_필드마다_한_줄이다()
    {
        var rows = Registry();
        var profiles = Mapping.Defaults().Profiles.Select(p => p.Id).ToHashSet();
        var bad = rows.Where(r => !Statuses.Contains(r.Status) || !profiles.Contains(r.Profile) || r.Source == "" || r.Id == "")
            .Select(r => $"{r.Line}행 {Show(r.Key)} — 「{r.Status}」").ToList();
        Assert.True(bad.Count == 0, "상태가 다섯 가지 밖이거나 프로필을 모르는 행:\n  " + string.Join("\n  ", bad));
        var twice = rows.GroupBy(r => r.Key).Where(g => g.Count() > 1)
            .Select(g => $"{Show(g.Key)} — {string.Join(", ", g.Select(r => r.Line + "행"))}").ToList();
        Assert.True(twice.Count == 0, "같은 필드가 두 번 적혔습니다:\n  " + string.Join("\n  ", twice));
    }
}
