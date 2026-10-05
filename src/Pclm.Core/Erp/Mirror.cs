using System.Globalization;
using System.Text.Json;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

/// <summary>
/// 칸 하나. <paramref name="Kind"/> 는 <c>id</c>(번호 열) · <c>same</c> · <c>changed</c> · <c>new</c>(견줄 차수가 없다) ·
/// <c>override</c>(사람이 고친 칸의 아래 값이 이번 화면으로 바뀐다).
/// </summary>
/// <param name="Value">계약면에 찍힐 표기 그대로. 덮개 칸이면 화면의 값(덮개 아래).</param>
/// <param name="Old">견준 차수의 표기. <c>changed</c> 에만.</param>
/// <param name="Raw">변환 전 화면의 원값이 표기와 다르면(날짜·금액) 그 원값.</param>
/// <param name="Human">사람이 고친 값. <c>override</c> 에만.</param>
/// <param name="ChoiceId">가져올 때 고를 자리(<see cref="CaptureChange.Id"/>). <c>override</c> 에만.</param>
/// <param name="Sources">이 열을 채우는 화면의 자리 — 수집 규칙의 source 키, 표에서 오면 <c>table:&lt;대상 표&gt;</c>.
/// 계약면의 열 정의에서 끌어낸다(<see cref="Views.FaceReferences"/>). 끌어낼 수 없으면 비었다.</param>
/// <param name="From">그 자리에 화면이 적어 둔 이름표. 화면이 이름표를 보내지 않았으면 빈 문자열.</param>
public sealed record MirrorField(string Column, string Kind, string Value, string? Old, string? Raw, string? Human, string? ChoiceId,
    List<string> Sources, string From);

/// <summary>
/// 확장이 화면에서 읽어 보낸 이름표와 보이는 글 한 줄(ADR-036). 값은 화면에 보인 그대로다 — 변환하지 않는다.
/// </summary>
/// <param name="Group">가장 가까운 앞의 구역 제목. 찾지 못했으면 빈 문자열.</param>
/// <param name="Source">수집 규칙의 source 키. 품목 줄은 <c>table:&lt;대상 표&gt;:&lt;화면의 몇째 줄&gt;</c>, 수집 규칙에 없는
/// 화면은 빈 문자열.</param>
public sealed record ScreenRow(string Group, string Label, string Text, string Source);

/// <summary>견준 차수의 같은 순번 품목. 수량·단가·금액 중 하나라도 다를 때만 온다.</summary>
public sealed record MirrorItemBefore(string Quantity, string Unit, string Price, string Amount);

/// <param name="Added">견준 차수에 없던 순번.</param>
/// <param name="Source">화면의 품목 줄 — <c>table:&lt;대상 표&gt;:&lt;화면의 몇째 줄&gt;</c>. 이번 화면에 없던 줄이면 빈 문자열.</param>
public sealed record MirrorItem(string Line, string Name, string Spec, string Quantity, string Unit, string Price, string Amount,
    MirrorItemBefore? Before, bool Added, string Source = "");

/// <summary>
/// 접수 → 공고 → 계약 의 한 칸. <paramref name="State"/> 는 <c>here</c>(이 화면) · <c>linked</c>(이어져 있다) ·
/// <c>ref</c>(원천 참조만 있다) · <c>missing</c>(아직 수집되지 않음).
/// </summary>
/// <param name="Number">번호-차수. <c>ref</c> 면 참조한 본번호.</param>
/// <param name="Collected"><c>ref</c> 일 때 그 본번호가 작업자료에 있는가(있는데 이어지지 않았다).</param>
/// <param name="Source">이 칸을 잇는 화면의 자리 — 그 종류를 참조하는 번호 칸. <c>ref</c>·<c>linked</c> 에만.</param>
public sealed record MirrorPlace(string Kind, string State, string Number, bool Collected, string? Source = null);

/// <param name="State"><c>stored</c> · <c>current</c>(이 화면과 같은 차수) · <c>ghost</c>(가져오면 설 차수).</param>
/// <param name="SavedOn">마지막으로 고친 날(로컬 <c>MM/dd</c>).</param>
public sealed record MirrorRound(string Seq, string Amount, string SavedOn, string State);

/// <summary>
/// 지금 보는 화면을 작업자료에 비춘 것(ADR-036). <b>칸은 계약면의 표기 그대로다</b> — 쓰기 잠금 안의 savepoint 에 그
/// 화면을 적용하고 면 뷰에서 읽은 뒤 되돌린다. 표기를 아는 것은 뷰뿐이라(<c>Views.cs</c> 의 날짜·금액·덮개) 화면이
/// 따로 흉내 내면 두 벌이 된다.
/// </summary>
/// <param name="Status"><c>새 자료</c> · <c>새 차수</c> · <c>검토 대기</c> · <c>바뀐 칸</c> · <c>저장됨</c>.</param>
/// <param name="Changes">견준 차수와 다른 칸 수 + 다른 품목 행 수.</param>
/// <param name="CompareSeq">견준 차수. 같은 차수가 있으면 그것, 없으면 그 본번호의 가장 늦은 차수. 본번호가 없으면 null.</param>
/// <param name="LatestSeq">작업자료에 있는 가장 늦은 차수.</param>
/// <param name="Rest">이 화면에서 읽지 않아 빈 열의 수.</param>
/// <param name="ItemRows">스냅샷의 품목 표 행 수.</param>
/// <param name="ItemsAllRead">미리보기의 품목 수가 스냅샷의 행 수와 같다.</param>
/// <param name="Blocked">창에서 가져올 수 없는 까닭(대응을 골라야 하는 품목, 지울지 골라야 하는 행). 없으면 null.</param>
public sealed record MirrorShot(string EntityType, string Kind, string Number, string Base, string Seq, string Title,
    string Status, int Changes, string? CompareSeq, string? LatestSeq, string View, int ViewColumns,
    List<MirrorField> Fields, int Rest, List<MirrorItem> Items, int ItemChanges, int ItemRows, bool ItemsAllRead,
    List<MirrorPlace> Place, List<MirrorRound> Rounds, string BaseToken, string? Blocked);

/// <summary>수집하는 화면 하나(ADR-037) — 「수집 안 함」 이 무엇을 열면 되는지 안내할 때 쓴다.</summary>
/// <param name="Kind">사람이 읽을 종류 이름(접수·공고·계약).</param>
public sealed record MirrorScreen(string Code, string EntityType, string Kind, string Name);

/// <summary>
/// 지금 보는 화면의 투영. 확장이 읽어 보낸 스냅샷을 <b>창 안에서</b> inspect 하고, 사람이 읽을 모양으로 편다. 쓰지 않는다 —
/// 투영의 쓰기는 모두 되돌린다. 링크를 짓는 <see cref="ExplicitLinks.Resolve(SqliteConnection, SqliteTransaction)"/> 는 부르지 않는다.
/// </summary>
public static class Mirror
{
    private const string Point = "pclm_mirror";

    public static string KindName(string entityType) => entityType switch
    {
        "request" => "접수",
        "notice" => "공고",
        _ => "계약",
    };

    /// <summary>
    /// 수집하는 화면들 — 지금 매핑에서 화면을 단 프로필(<see cref="Screens.Supported"/>). 매핑을 읽지 못하면 기본 매핑으로
    /// 물러선다: 화면과 고유키는 기본의 것으로 고정이라 같다.
    /// </summary>
    public static List<MirrorScreen> SupportedScreens(Database database)
    {
        MappingSet mapping;
        try { mapping = new MappingStore(database).Active(); }
        catch (Exception e) when (e is InvalidOperationException or JsonException or SqliteException) { mapping = Mapping.Defaults(); }
        return Screens.Supported(mapping).Select(p => new MirrorScreen(p.Screen!.Code, p.EntityType, KindName(p.EntityType), p.Screen.Name)).ToList();
    }

    private static string TitleColumn(string entityType) => entityType switch
    {
        "request" => "요청명",
        "notice" => "공고명",
        _ => "계약건명",
    };

    private static string AmountColumn(string entityType) => entityType switch
    {
        "request" => "품대",
        "notice" => "추정가격",
        _ => "계약금액",
    };

    private sealed record ItemView(string View, string Key, string Name, string Price, string? Amount);

    private static ItemView ItemsOf(string entityType) => entityType switch
    {
        "request" => new("v_접수품목", "접수번호", "세부품명", "단가", "금액"),
        "notice" => new("v_공고품목", "입찰공고번호", "세부품명", "추정단가", null),
        _ => new("v_계약품목", "계약번호", "품명", "단가", "금액"),
    };

    /// <summary>있던 값을 빈 값으로 지우는 변경 — 늦게 그려진 탭을 빈 칸으로 읽은 것이 대부분이라 유지가 기본이다(확장의 wipes 와 같다).</summary>
    public static bool Wipes(CaptureChange change) =>
        change.Conflict && change.Override is null && change.Field != "__row" &&
        (change.After ?? "") == "" && (change.Before ?? "") != "";

    /// <summary>
    /// 확장과 같은 기본 규칙으로 고른 것. 덮어쓰기는 적용하고, 있던 값이 비게 되는 것은 유지하고, 사람이 고친 칸은
    /// 사람이 고른 것(<paramref name="restore"/> 에 있으면 복원, 아니면 유지)을 따른다. 행 삭제는 고르지 않는다 —
    /// 그것이 남으면 창에서는 가져오지 않는다(<see cref="MirrorShot.Blocked"/>).
    /// </summary>
    public static Dictionary<string, string> DefaultChoices(CapturePreview preview, IReadOnlySet<string> restore)
    {
        var choices = new Dictionary<string, string>();
        foreach (var change in preview.Changes.Where(ch => ch.Conflict))
        {
            if (change.Field == "__row") continue;
            choices[change.Id] = change.Field == "__override" ? (restore.Contains(change.Id) ? "apply" : "keep")
                : Wipes(change) ? "keep" : "apply";
        }
        return choices;
    }

    /// <summary>창에서 가져올 수 없는 까닭. 없으면 null.</summary>
    public static string? BlockedBy(CapturePreview preview) =>
        preview.Unmatched.Count > 0 ? "기존 품목과 화면의 품목을 짝지어야 합니다."
        : preview.Changes.Any(ch => ch.Field == "__row") ? "화면에 없는 품목 행을 지울지 골라야 합니다."
        : null;

    /// <summary>
    /// 스냅샷 하나를 투영한다. 쓰기 잠금을 짧게 쥐고(BEGIN IMMEDIATE) 그 안의 savepoint 에서만 쓴 뒤 모두 되돌린다.
    /// </summary>
    /// <param name="screen">확장이 함께 보낸 화면의 이름표와 보이는 글. 칸마다 「화면 · 이름표」 와 화면 표기를 거기서 읽는다.</param>
    public static MirrorShot Project(Database database, CaptureInput input, IReadOnlyList<ScreenRow>? screen = null)
    {
        var mapping = new MappingStore(database).Active();
        var capture = new ErpCapture(database);
        using var c = capture.Open();
        using var tx = c.BeginTransaction(deferred: false);
        try
        {
            var preview = capture.Inspect(c, tx, input);
            return Project(c, tx, mapping, input, preview, screen ?? []);
        }
        finally { tx.Rollback(); }
    }

    /// <summary>탭 보고 한 탭의 <c>screenRows</c>. 없거나 모양이 틀린 줄은 건너뛴다(호스트가 이미 모양을 보았다).</summary>
    public static List<ScreenRow> ScreenRows(JsonElement tab)
    {
        var rows = new List<ScreenRow>();
        if (tab.ValueKind != JsonValueKind.Object || !tab.TryGetProperty("screenRows", out var list) || list.ValueKind != JsonValueKind.Array) return rows;
        foreach (var row in list.EnumerateArray())
        {
            string? Text(string name) => row.ValueKind == JsonValueKind.Object && row.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
            if (Text("group") is { } g && Text("label") is { } l && Text("text") is { } t && Text("source") is { } s) rows.Add(new(g, l, t, s));
        }
        return rows;
    }

    /// <summary>
    /// 화면 줄의 자리가 그 source 를 가리키는가. 표 자리(<c>table:x</c>)는 그 표의 모든 줄(<c>table:x:n</c>)을 가리킨다.
    /// 화면과 창이 같은 규칙으로 잇는다(<c>Mirror.tsx</c> 의 <c>잇는가</c>).
    /// </summary>
    public static bool Points(string source, string row) =>
        source != "" && (row == source || source.StartsWith("table:", StringComparison.Ordinal) && row.StartsWith(source + ":", StringComparison.Ordinal));

    /// <summary>
    /// 면 열 하나를 채우는 화면의 자리. 열 정의가 가리키는 본 표의 열(<see cref="Views.FaceReferences"/>)을 수집 규칙의
    /// 대상과 맞춘다 — 머리 칸이면 그 source 키, 딸린 표면 <c>table:&lt;대상 표&gt;</c>. 번호 열은 문서를 가리는 칸이다.
    /// </summary>
    private static List<string> SourcesOf(MappingProfile? profile, string type, string column,
        IReadOnlyDictionary<string, IReadOnlyList<string>> references)
    {
        if (profile is null) return [];
        if (column == Store.FaceKey(type) || column.EndsWith("본번호", StringComparison.Ordinal) || column == "차수")
            return profile.IdentitySource == "pointInfo" ? new[] { profile.BaseField, profile.SeqField }.Distinct().ToList() : [];
        var sources = new List<string>();
        foreach (var reference in references.GetValueOrDefault(column) ?? [])
        {
            var dot = reference.IndexOf('.');
            var (table, target) = (reference[..dot], reference[(dot + 1)..]);
            if (table == type)
                sources.AddRange(profile.Fields.Where(f => f.Target == target).Select(f => f.Source));
            else if (profile.Tables.FirstOrDefault(t => t.Target == table) is { } t && t.Fields.Any(f => f.Target == target))
                sources.Add("table:" + table);
        }
        return sources.Distinct(StringComparer.Ordinal).ToList();
    }

    /// <summary>그 종류(<c>request</c>·<c>notice</c>)를 참조하는 번호 칸의 자리 — <c>ref_&lt;종류&gt;_base</c> 를 채우는 것.</summary>
    private static string? RefSource(MappingProfile? profile, string kind)
    {
        var target = "ref_" + kind + "_base";
        if (profile?.Fields.FirstOrDefault(f => f.Target == target) is { } field) return field.Source;
        return profile?.Tables.FirstOrDefault(t => t.Fields.Any(f => f.Target == target)) is { } table ? "table:" + table.Target : null;
    }

    private static MirrorShot Project(SqliteConnection c, SqliteTransaction tx, MappingSet mapping, CaptureInput input, CapturePreview preview,
        IReadOnlyList<ScreenRow> screen)
    {
        var doc = preview.Document;
        var type = doc.EntityType;
        var profile = mapping.Profiles.SingleOrDefault(p => p.Id == input.Profile);
        var references = Views.FaceReferences(type);
        var face = Store.FaceView(type);
        var key = Store.FaceKey(type);
        var items = ItemsOf(type);
        string Display(string seq) => new EntityRef(type, doc.Base, seq).Display;
        static int Number(string seq) => int.Parse(seq, CultureInfo.InvariantCulture);

        // 쌓인 차수. 고치기 전에 읽는다.
        var stored = c.Query<(string Seq, string? UpdatedAt)>(
            $"SELECT seq, updated_at FROM {type} WHERE {type}_base=@Base ORDER BY CAST(seq AS INTEGER)", doc, tx).ToList();
        var exists = stored.Any(r => r.Seq == doc.Seq);
        var latest = stored.Count > 0 ? stored[^1].Seq : null;
        var compare = exists ? doc.Seq : latest;
        var rounds = stored.Select(r => new MirrorRound(r.Seq,
            Scalar(c, tx, ErpCapture.RevisionView(type), AmountColumn(type), key, Display(r.Seq)) ?? "",
            LocalDay(r.UpdatedAt), r.Seq == doc.Seq ? "current" : "stored")).ToList();
        var users = c.Query<string>("SELECT field_name FROM user_column WHERE entity_type=@type", new { type }, tx).ToHashSet(StringComparer.Ordinal);

        Dictionary<string, string>? before = null;
        Dictionary<string, Dictionary<string, string>> beforeItems = [];
        Dictionary<string, string> after;
        List<Dictionary<string, string>> afterItems;
        List<MirrorPlace> place;
        var itemSources = new Dictionary<string, string>(StringComparer.Ordinal);
        var columns = 0;
        tx.Save(Point);
        try
        {
            // 견줄 차수를 계약면에서 읽는다. 면 뷰는 본번호마다 가장 늦은 차수만 내므로, 그보다 늦은 것을 잠시 걷어
            // 그 차수가 면에 서게 한다 — 모두 되돌린다.
            if (compare is not null)
            {
                DropLater(c, tx, type, doc.Base, compare);
                before = Row(c, tx, face, key, Display(compare));
                beforeItems = Rows(c, tx, items.View, items.Key, Display(compare))
                    .GroupBy(r => r.GetValueOrDefault("순번", "")).ToDictionary(g => g.Key, g => g.First());
                tx.Rollback(Point);
            }
            // 이 화면을 기본 규칙으로 적용하고 그 차수를 면에서 읽는다.
            DropLater(c, tx, type, doc.Base, doc.Seq);
            var defaults = DefaultChoices(preview, new HashSet<string>());
            ErpCapture.Write(c, tx, input, doc, preview.Changes, ch => ch.Conflict ? defaults.GetValueOrDefault(ch.Id, "keep") : "apply",
                DateTime.UtcNow.ToString("O"));
            after = Row(c, tx, face, key, Display(doc.Seq)) ?? [];
            columns = after.Count;
            afterItems = Rows(c, tx, items.View, items.Key, Display(doc.Seq));
            // 품목 줄마다 화면의 몇째 줄인가 — 확장이 화면의 품목 줄을 같은 차례로 센다(table:<대상 표>:<n>).
            var order = doc.Rows.Where(r => r.Table == type + "_item").Select((r, i) => (r.SourceKey, N: i + 1))
                .GroupBy(r => r.SourceKey).ToDictionary(g => g.Key, g => g.First().N, StringComparer.Ordinal);
            foreach (var (key2, line) in c.Query<(string SourceKey, long Line)>(
                "SELECT source_key, line_no FROM erp_row WHERE entity_type=@type AND entity_base=@Base AND entity_seq=@Seq AND table_name=@table",
                new { type, doc.Base, doc.Seq, table = type + "_item" }, tx))
                if (order.TryGetValue(key2, out var n)) itemSources[line.ToString(CultureInfo.InvariantCulture)] = $"table:{type}_item:{n}";
            place = Place(c, tx, type, doc.Base, doc.Seq, Display(doc.Seq));
        }
        finally { tx.Rollback(Point); tx.Release(Point); }
        place = place.Select(p => p.State is "ref" or "linked"
            ? p with { Source = RefSource(profile, p.Kind switch { "접수" => "request", "공고" => "notice", _ => "contract" }) }
            : p).ToList();

        // 칸. 화면의 이름표와 보이는 글은 그 칸을 채우는 자리에서 읽는다.
        var overrides = preview.Changes.Where(ch => ch.Field == "__override" && ch.Override is not null)
            .GroupBy(ch => ch.Override!).ToDictionary(g => g.Key, g => g.First());
        var fields = new List<MirrorField>();
        var rest = 0;
        foreach (var (column, value) in after)
        {
            if (users.Contains(column)) continue; // 사람이 채우는 열은 화면에서 오지 않는다.
            var sources = SourcesOf(profile, type, column, references);
            var from = From(sources, screen);
            if (Views.KeyColumns.Contains(column))
            {
                if (value != "") fields.Add(new(column, "id", value, null, null, null, null, sources, from));
                continue;
            }
            if (overrides.TryGetValue(column, out var o))
            {
                fields.Add(new(column, "override", o.After ?? "", null, Shown(o.After ?? "", sources, screen), o.Before ?? "", o.Id, sources, from));
                continue;
            }
            var old = before?.GetValueOrDefault(column);
            if (value == "" && (old ?? "") == "") { rest++; continue; }
            var kind = before is null ? "new" : value == old ? "same" : "changed";
            fields.Add(new(column, kind, value, kind == "changed" ? old ?? "" : null, Shown(value, sources, screen), null, null, sources, from));
        }

        // 품목.
        var list = new List<MirrorItem>();
        foreach (var row in afterItems)
        {
            var line = row.GetValueOrDefault("순번", "");
            var item = Item(row, items, null, false) with { Source = itemSources.GetValueOrDefault(line, "") };
            if (before is not null)
            {
                if (!beforeItems.TryGetValue(line, out var earlier)) item = item with { Added = true };
                else
                {
                    var was = new MirrorItemBefore(earlier.GetValueOrDefault("수량", ""), earlier.GetValueOrDefault("단위", ""),
                        earlier.GetValueOrDefault(items.Price, ""), items.Amount is null ? "" : earlier.GetValueOrDefault(items.Amount, ""));
                    if (was.Quantity != item.Quantity || was.Unit != item.Unit || was.Price != item.Price || was.Amount != item.Amount)
                        item = item with { Before = was };
                }
            }
            list.Add(item);
        }
        var itemChanges = list.Count(i => i.Before is not null || i.Added);
        var itemRows = ItemRows(mapping, input, type);

        var changes = fields.Count(f => f.Kind == "changed") + itemChanges;
        var status = stored.Count == 0 ? "새 자료"
            : !exists ? "새 차수"
            : overrides.Count > 0 ? "검토 대기"
            : changes > 0 ? "바뀐 칸"
            : "저장됨";
        if (!exists)
            rounds.Add(new(doc.Seq, after.GetValueOrDefault(AmountColumn(type), ""), "", "ghost"));
        rounds = rounds.OrderBy(r => Number(r.Seq)).ToList();

        return new(type, KindName(type), Display(doc.Seq), doc.Base, doc.Seq, after.GetValueOrDefault(TitleColumn(type), ""),
            status, changes, compare, latest, face, columns - users.Count(after.ContainsKey),
            fields, rest, list, itemChanges, itemRows, itemRows > 0 && preview.ItemCount == itemRows,
            place, rounds, preview.BaseToken, BlockedBy(preview));
    }

    private static MirrorItem Item(Dictionary<string, string> row, ItemView view, MirrorItemBefore? before, bool added) =>
        new(row.GetValueOrDefault("순번", ""), row.GetValueOrDefault(view.Name, ""), row.GetValueOrDefault("규격", ""),
            row.GetValueOrDefault("수량", ""), row.GetValueOrDefault("단위", ""), row.GetValueOrDefault(view.Price, ""),
            view.Amount is null ? "" : row.GetValueOrDefault(view.Amount, ""), before, added);

    /// <summary>그 차수보다 늦은 차수를 걷는다. savepoint 안에서만 부른다 — 딸린 표는 FK 가 함께 걷는다.</summary>
    private static void DropLater(SqliteConnection c, SqliteTransaction tx, string type, string @base, string seq) =>
        c.Execute($"DELETE FROM {type} WHERE {type}_base=@base AND CAST(seq AS INTEGER) > CAST(@seq AS INTEGER)", new { @base, seq }, tx);

    private static Dictionary<string, string>? Row(SqliteConnection c, SqliteTransaction tx, string view, string key, string value) =>
        Rows(c, tx, view, key, value).FirstOrDefault();

    /// <summary>뷰의 줄들. 열 차례를 지키고, 값은 문자열로(계약면은 빈 값도 빈 문자열이다).</summary>
    private static List<Dictionary<string, string>> Rows(SqliteConnection c, SqliteTransaction tx, string view, string key, string value)
    {
        using var command = c.CreateCommand();
        command.Transaction = tx;
        command.CommandText = $"SELECT * FROM \"{view}\" WHERE \"{key}\"=$key";
        command.Parameters.AddWithValue("$key", value);
        using var reader = command.ExecuteReader();
        var rows = new List<Dictionary<string, string>>();
        while (reader.Read())
        {
            var row = new Dictionary<string, string>(StringComparer.Ordinal);
            for (var i = 0; i < reader.FieldCount; i++)
                row[reader.GetName(i)] = reader.IsDBNull(i) ? "" : Convert.ToString(reader.GetValue(i), CultureInfo.InvariantCulture) ?? "";
            rows.Add(row);
        }
        return rows;
    }

    private static string? Scalar(SqliteConnection c, SqliteTransaction tx, string view, string column, string key, string value) =>
        Row(c, tx, view, key, value)?.GetValueOrDefault(column);

    private static string LocalDay(string? utc) =>
        DateTime.TryParse(utc, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind, out var at)
            ? at.ToLocalTime().ToString("MM/dd", CultureInfo.InvariantCulture) : "";

    /// <summary>
    /// 접수 → 공고 → 계약 의 자리. <b>지금 저장된 연결로만</b> 읽는다 — 링크를 짓는 <c>ExplicitLinks.Resolve</c> 를 부르지
    /// 않는다. 이어지지 않았으면 원천 참조를 보이고, 그것도 없으면 아직 수집되지 않은 것으로 둔다.
    /// </summary>
    private static List<MirrorPlace> Place(SqliteConnection c, SqliteTransaction tx, string type, string @base, string seq, string display)
    {
        string? Latest(string kind, string b) => c.QuerySingleOrDefault<string>(
            $"SELECT seq FROM {kind} WHERE {kind}_base=@b ORDER BY CAST(seq AS INTEGER) DESC LIMIT 1", new { b }, tx) is { } s
            ? new EntityRef(kind, b, s).Display : null;
        bool Has(string kind, string b) =>
            c.ExecuteScalar<long>($"SELECT count(*) FROM {kind}_series WHERE {kind}_base=@b", new { b }, tx) > 0;

        var group = type switch
        {
            "notice" => c.QuerySingleOrDefault<string>("SELECT group_base FROM notice_series WHERE notice_base=@base", new { @base }, tx),
            "contract" => c.QuerySingleOrDefault<string>("SELECT notice_group FROM project_link WHERE contract_base=@base", new { @base }, tx),
            _ => c.QuerySingleOrDefault<string>("SELECT notice_group FROM request_link WHERE request_base=@base", new { @base }, tx),
        };
        MirrorPlace? request = type == "request" ? new("접수", "here", display, true) : null;
        MirrorPlace? notice = type == "notice" ? new("공고", "here", display, true) : null;
        MirrorPlace? contract = type == "contract" ? new("계약", "here", display, true) : null;
        if (group is not null)
        {
            if (notice is null)
            {
                var current = c.QuerySingleOrDefault<string>(
                    "SELECT 현행공고 FROM v_공고 WHERE 공고건=@group AND 현행공고<>'' LIMIT 1", new { group }, tx)
                    ?? c.Query<string>("SELECT notice_base FROM notice_series WHERE group_base=@group ORDER BY notice_base DESC", new { group }, tx)
                        .Select(b => Latest("notice", b)).FirstOrDefault(n => n is not null);
                if (current is not null) notice = new("공고", "linked", current, true);
            }
            if (request is null && c.QuerySingleOrDefault<string>("SELECT request_base FROM request_link WHERE notice_group=@group", new { group }, tx) is { } r
                && Latest("request", r) is { } shownRequest)
                request = new("접수", "linked", shownRequest, true);
            if (contract is null)
            {
                var linked = c.Query<string>("SELECT contract_base FROM project_link WHERE notice_group=@group ORDER BY contract_base", new { group }, tx)
                    .Select(b => Latest("contract", b)).OfType<string>().ToList();
                if (linked.Count > 0) contract = new("계약", "linked", string.Join(" · ", linked), true);
            }
        }
        // 원천 참조. 계약은 공고·접수 번호를, 공고는 품목마다 접수 번호를 싣는다.
        if (type == "contract")
        {
            var refs = c.QuerySingleOrDefault<(string? Notice, string? Request)>(
                "SELECT ref_notice_base, ref_request_base FROM contract WHERE contract_base=@base AND seq=@seq", new { @base, seq }, tx);
            if (notice is null && !string.IsNullOrEmpty(refs.Notice)) notice = new("공고", "ref", refs.Notice, Has("notice", refs.Notice));
            if (request is null && !string.IsNullOrEmpty(refs.Request)) request = new("접수", "ref", refs.Request, Has("request", refs.Request));
        }
        if (type == "notice" && request is null)
        {
            var refs = c.Query<string>("SELECT DISTINCT ref_request_base FROM notice_item WHERE notice_base=@base AND seq=@seq AND coalesce(ref_request_base,'')<>''",
                new { @base, seq }, tx).ToList();
            if (refs.Count == 1) request = new("접수", "ref", refs[0], Has("request", refs[0]));
        }
        return
        [
            request ?? new("접수", "missing", "", false),
            notice ?? new("공고", "missing", "", false),
            contract ?? new("계약", "missing", "", false),
        ];
    }

    /// <summary>
    /// 「화면 · 이름표」 — 그 칸을 채우는 자리에 화면이 적어 둔 이름표. 머리 칸이면 그 칸의 이름표, 표에서 오면 그 표가 선
    /// 구역의 제목. 화면이 이름표를 보내지 않았으면 빈 문자열.
    /// </summary>
    private static string From(List<string> sources, IReadOnlyList<ScreenRow> screen)
    {
        foreach (var source in sources.Where(s => !s.StartsWith("table:", StringComparison.Ordinal)))
            if (screen.FirstOrDefault(r => r.Source == source && r.Label != "") is { } row) return row.Label;
        foreach (var source in sources.Where(s => s.StartsWith("table:", StringComparison.Ordinal)))
            if (screen.FirstOrDefault(r => Points(source, r.Source)) is { } row) return row.Group;
        return "";
    }

    /// <summary>
    /// 화면 표기 — 그 칸이 화면에 보인 글이 계약면의 표기와 다를 때만(날짜의 줄표·금액의 「원」). 화면이 보낸 보이는 글을
    /// 그대로 쓴다: 값을 맞춰 보는 어림은 두지 않는다. 머리 칸 하나에서 오는 열만 — 표나 여러 칸을 엮은 열은 대응할 글이 없다.
    /// </summary>
    private static string? Shown(string value, List<string> sources, IReadOnlyList<ScreenRow> screen)
    {
        var header = sources.Where(s => !s.StartsWith("table:", StringComparison.Ordinal)).ToList();
        if (header.Count != 1 || header.Count != sources.Count) return null;
        var text = screen.FirstOrDefault(r => r.Source == header[0])?.Text;
        return string.IsNullOrEmpty(text) || text == value ? null : text;
    }

    /// <summary>스냅샷의 품목 표 행 수. 표가 없거나 하나로 정해지지 않으면 0.</summary>
    private static int ItemRows(MappingSet mapping, CaptureInput input, string type)
    {
        var profile = mapping.Profiles.SingleOrDefault(p => p.Id == input.Profile);
        var table = profile?.Tables.FirstOrDefault(t => t.Target == type + "_item");
        if (table is null) return 0;
        try { return Mapping.Table(input.Data, table.Source).Length; }
        catch (InvalidOperationException) { return 0; }
    }

    /// <summary>탭 보고의 스냅샷을 수집 입력으로. 확장의 <c>inspect</c> 와 같은 모양이다.</summary>
    public static CaptureInput Input(JsonElement snapshot) =>
        snapshot.Deserialize<CaptureInput>(Mapping.Json) ?? throw new InvalidOperationException("화면 자료가 비었습니다.");
}
