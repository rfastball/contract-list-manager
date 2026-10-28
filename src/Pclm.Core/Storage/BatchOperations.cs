using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Dapper;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

public sealed record CellChange(string RowKey, string Field, string Value, string Original, string Operation);
public sealed record BatchDeletionPlan(string Fingerprint, IReadOnlyList<DeletionPlan> Targets,
    int DocumentCount, int SeriesCount, int ChildRows, int UserFields, int Overrides, int Links);

public sealed partial class Store
{
    /// <summary>모든 칸을 검증한 뒤 같은 쓰기 트랜잭션으로 넣는다. Original은 화면에서 본 현재 값이다.</summary>
    public int BatchEdit(string view, IReadOnlyList<CellChange> changes)
    {
        if (view is not ("v_통합" or "v_접수" or "v_공고" or "v_계약"))
            throw new InvalidOperationException("이 표에서는 일괄 편집할 수 없습니다.");
        if (changes.Count is < 1 or > 10000)
            throw new InvalidOperationException("한 번에 1~10,000셀을 변경할 수 있습니다.");
        if (_database.Access == Access.Read) throw new InvalidOperationException("열람 중에는 자료를 고칠 수 없습니다.");

        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();
        var type = Views.EntityTypeOf(view);
        var columns = ViewColumns(connection, view).ToHashSet();
        var definitions = connection.Query<UserColumnDefinition>(
            "SELECT entity_type AS EntityType, field_name AS FieldName, kind AS Kind, options AS Options, sort_order AS SortOrder FROM user_column WHERE entity_type = @type;",
            new { type }).ToDictionary(c => c.FieldName);
        var pending = new List<(EntityRef Entity, CellChange Change, bool User)>();
        var seen = new HashSet<(string Base, string Seq, string Field)>();
        foreach (var change in changes)
        {
            if (change is null || string.IsNullOrEmpty(change.RowKey) || string.IsNullOrEmpty(change.Field)
                || change.Value is null || change.Original is null || change.Value.Length > 32767
                || change.Operation is not ("set" or "revert"))
                throw new InvalidOperationException("변경할 셀 정보가 올바르지 않습니다.");
            var entity = Resolve(connection, change.RowKey)
                ?? throw new InvalidOperationException($"자료가 없어졌습니다: {change.RowKey}");
            if (entity.EntityType != type || !columns.Contains(change.Field) || Views.KeyColumns.Contains(change.Field)
                || (view == "v_통합" && (Views.UnifiedNoticeColumns.Contains(change.Field) || Views.UnifiedRequestColumns.Contains(change.Field))))
                throw new InvalidOperationException($"고칠 수 없는 셀입니다: {change.RowKey} · {change.Field}");
            // 반드시 현재 표에 있는 정확한 차수여야 한다. Resolve의 별칭/옛 차수로 우회하지 않는다.
            if (connection.ExecuteScalar<long>($"SELECT COUNT(*) FROM \"{view}\" WHERE \"{FaceKey(type)}\" = @key;",
                    new { key = entity.Display }) == 0 || entity.Display != change.RowKey)
                throw new InvalidOperationException("자료의 차수가 바뀌었습니다. 표를 새로 읽고 다시 해 주세요.");

            var user = definitions.TryGetValue(change.Field, out var definition);
            if (user && (definition!.Kind == "computed" || change.Operation == "revert"))
                throw new InvalidOperationException($"원래 값 복원 또는 직접 입력을 할 수 없는 열입니다: {change.Field}");
            if (!user) CheckCorrectable(connection, entity, change.Field);
            if (!seen.Add((entity.Base, user ? "" : entity.Seq, change.Field)))
                throw new InvalidOperationException($"같은 셀이 두 번 포함되어 있습니다: {change.Field}");
            if (FaceValue(connection, entity, change.Field) != change.Original)
                throw new InvalidOperationException($"다른 작업에서 값이 바뀌었습니다: {change.RowKey} · {change.Field}. 표를 새로 읽고 다시 해 주세요.");
            if (user && change.Value.Length > 0)
            {
                var valid = definition!.Kind switch
                {
                    "choice" => definition.Choices.Contains(change.Value),
                    "number" => decimal.TryParse(change.Value, NumberStyles.Number, CultureInfo.InvariantCulture, out _),
                    "date" => DateOnly.TryParseExact(change.Value, ["yyyy-MM-dd", "yyyy/MM/dd"], CultureInfo.InvariantCulture, DateTimeStyles.None, out _),
                    _ => true,
                };
                if (!valid) throw new InvalidOperationException($"열 형식에 맞지 않는 값입니다: {change.Field} · {change.Value}");
            }
            pending.Add((entity, change, user));
        }
        foreach (var (entity, change, user) in pending)
        {
            if (user) SetUserField(connection, entity, change.Field, change.Value);
            else if (change.Operation == "revert") ClearOverride(connection, entity, change.Field);
            else SetOverride(connection, entity, change.Field, change.Value, change.Original);
        }
        if (pending.Any(p => !p.User && p.Entity.EntityType == "notice" && p.Change.Field == "공고종류"))
            Pclm.Core.Erp.ExplicitLinks.Resolve(connection, transaction);
        transaction.Commit();
        return pending.Count;
    }

    public BatchDeletionPlan PlanBatchDeletion(IReadOnlyList<string> keys, bool wholeSeries)
    {
        RefuseBatchReadOnly();
        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();
        return PlanBatchDeletion(connection, transaction, BatchTargets(connection, keys, wholeSeries), wholeSeries);
    }

    public BatchDeletionPlan BatchDelete(IReadOnlyList<string> keys, bool wholeSeries, string fingerprint)
    {
        RefuseBatchReadOnly();
        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();
        var targets = BatchTargets(connection, keys, wholeSeries);
        var plan = PlanBatchDeletion(connection, transaction, targets, wholeSeries);
        if (string.IsNullOrEmpty(fingerprint) || plan.Fingerprint != fingerprint)
            throw new InvalidOperationException("삭제 대상이나 연결이 바뀌었습니다. 삭제 내용을 다시 확인해 주세요.");
        DeleteBatchRows(connection, transaction, targets, wholeSeries);
        transaction.Commit();
        return plan;
    }

    private void RefuseBatchReadOnly()
    {
        if (_database.Access == Access.Read) throw new InvalidOperationException("열람 중에는 자료를 삭제할 수 없습니다.");
    }

    private static List<EntityRef> BatchTargets(SqliteConnection connection, IReadOnlyList<string> keys, bool wholeSeries)
    {
        if (keys.Count is < 1 or > 2000 || keys.Any(string.IsNullOrWhiteSpace))
            throw new InvalidOperationException("한 번에 1~2,000건을 선택해 주세요.");
        var targets = keys.Select(key => Resolve(connection, key)
            ?? throw new InvalidOperationException($"자료가 없어졌습니다: {key}"));
        return targets.OrderBy(e => e.EntityType, StringComparer.Ordinal).ThenBy(e => e.Base, StringComparer.Ordinal)
            .ThenBy(e => e.Seq, StringComparer.Ordinal)
            .DistinctBy(e => (e.EntityType, e.Base, Seq: wholeSeries ? "" : e.Seq)).ToList();
    }

    private static BatchDeletionPlan PlanBatchDeletion(SqliteConnection connection, SqliteTransaction transaction,
        List<EntityRef> entities, bool wholeSeries)
    {
        var plans = entities.Select(e => PlanDeletion(connection, e, wholeSeries)).ToList();
        var snapshot = new List<string> { wholeSeries.ToString() };
        void Remember(string sql, object? args = null)
        {
            snapshot.Add(sql);
            snapshot.AddRange(connection.Query(sql, args).Select(row => JsonSerializer.Serialize((IDictionary<string, object>)row))
                .Order(StringComparer.Ordinal));
        }
        Remember("SELECT * FROM pclm_file;");
        // 접수/계약뿐 아니라 공고 건을 다시 나누는 관계도 삭제 승인에 포함한다.
        foreach (var table in new[] { "project_link", "request_link", "notice_series", "notice_relation" })
            Remember($"SELECT * FROM {table};");
        foreach (var group in entities.GroupBy(e => (e.EntityType, e.Base)))
        {
            var entity = group.First();
            snapshot.Add(JsonSerializer.Serialize(group));
            var (head, key, _, user, children) = Shape(entity.EntityType);
            foreach (var table in new[] { head, user }.Concat(children))
                Remember($"SELECT * FROM {table} WHERE {key} = @Base;", new { entity.Base });
            Remember("SELECT * FROM field_override WHERE entity_type = @EntityType AND base = @Base;", entity);
        }
        var fingerprint = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(JsonSerializer.Serialize(snapshot))));
        var before = DeletionCounts(connection);
        var links = LinkIdentities(connection);
        // 기존 삭제 경로를 그대로 시험하고 되감아, 여러 차수·계열의 겹치는 영향까지 실제로 센다.
        transaction.Save("deletion_preview");
        int[] after;
        int removedLinks;
        try
        {
            DeleteBatchRows(connection, transaction, entities, wholeSeries);
            after = DeletionCounts(connection);
            removedLinks = links.Except(LinkIdentities(connection)).Count();
        }
        finally
        {
            transaction.Rollback("deletion_preview");
            transaction.Release("deletion_preview");
        }
        var selectedBySeries = entities.GroupBy(e => (e.EntityType, e.Base)).ToDictionary(g => g.Key, g => g.Count());
        for (var i = 0; i < plans.Count; i++)
        {
            var entity = entities[i];
            if (wholeSeries || selectedBySeries[(entity.EntityType, entity.Base)] == plans[i].Revisions.Count)
                plans[i] = plans[i] with { SeriesGoes = true };
        }
        return new(fingerprint, plans, before[0] - after[0], before[1] - after[1], before[2] - after[2],
            before[3] - after[3], before[4] - after[4], removedLinks);
    }

    private static int[] DeletionCounts(SqliteConnection connection)
    {
        var shapes = new[] { "contract", "notice", "request" }.Select(Shape).ToArray();
        int Count(IEnumerable<string> tables) => tables.Sum(t => connection.ExecuteScalar<int>($"SELECT COUNT(*) FROM {t};"));
        return [Count(shapes.Select(s => s.Head)), Count(shapes.Select(s => s.Series)),
            Count(shapes.SelectMany(s => s.Children)), Count(shapes.Select(s => s.UserFields)), Count(["field_override"])];
    }

    private static void DeleteBatchRows(SqliteConnection connection, SqliteTransaction transaction,
        IEnumerable<EntityRef> entities, bool wholeSeries)
    {
        foreach (var entity in entities) Delete(connection, transaction, entity, wholeSeries, rebuild: false);
        Pclm.Core.Erp.ExplicitLinks.Resolve(connection, transaction);
    }

    private static HashSet<string> LinkIdentities(SqliteConnection connection) =>
        // 공고 건을 다시 만들며 끝점이 옮겨져도 연결 자체는 살아 있다. 불변 기본키로 소멸만 센다.
        connection.Query<string>("SELECT 'contract:' || contract_base FROM project_link UNION ALL SELECT 'request:' || request_base FROM request_link;")
            .ToHashSet(StringComparer.Ordinal);
}
