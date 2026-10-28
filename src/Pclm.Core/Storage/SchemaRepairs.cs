using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;

namespace Pclm.Core.Storage;

/// <summary>배포된 자료의 입증된 원천값과 열 이름을 같은 판올림 트랜잭션에서 복구한다.</summary>
internal static class SchemaRepairs
{
    internal static void Upgrade32(SqliteConnection c, SqliteTransaction tx)
    {
        var defaults = Mapping.Defaults();
        var defaultRevision = Mapping.Hash(defaults);
        foreach (var row in c.Query<(string Base, string Seq, string Value, string Document)>("""
            SELECT n.notice_base,n.seq,n.notice_kind,s.document_json FROM notice n JOIN erp_source s
              ON s.entity_type='notice' AND s.entity_base=n.notice_base AND s.entity_seq=n.seq
            WHERE n.notice_kind IS NOT NULL
            """, transaction: tx).ToList())
        {
            try
            {
                var document = JsonNode.Parse(row.Document);
                var head = document?["rows"]?.AsArray().FirstOrDefault(r => (string?)r?["table"] == "notice");
                if ((string?)head?["values"]?["notice_kind"] != row.Value ||
                    (string?)head?["origins"]?["notice_kind"] is not ("g2b-notice-a-v1" or "g2b-notice-a-service-v1")) continue;
                var profile = (string)head!["origins"]!["notice_kind"]!;
                foreach (var snapshot in c.Query<string>("""
                    SELECT snapshot_json FROM erp_capture WHERE profile=@profile AND entity_base=@Base AND entity_seq=@Seq
                    ORDER BY captured_at DESC, capture_id DESC
                    """, new { profile, row.Base, row.Seq }, tx))
                {
                    var receipt = JsonNode.Parse(snapshot);
                    var revision = (string?)receipt?["mappingRevision"];
                    var json = revision == defaultRevision ? JsonSerializer.Serialize(defaults, Mapping.Json)
                        : c.QuerySingleOrDefault<string>("SELECT json FROM erp_mapping WHERE revision=@revision", new { revision }, tx);
                    if (json is null) break; // 당시 매핑을 모르면 원천 칸을 추정하지 않는다.
                    var field = JsonNode.Parse(json)?["profiles"]?.AsArray().SingleOrDefault(p => (string?)p?["id"] == profile)?["fields"]?
                        .AsArray().SingleOrDefault(f => (string?)f?["target"] == "notice_kind")?.Deserialize<FieldMap>(Mapping.Json);
                    if (field is null || string.IsNullOrEmpty(field.Source) || field.SecondarySource is not null || field.Source.StartsWith("tables.")) break;
                    if (receipt?["data"]?["pointInfo"] is not { } point) break;
                    using var raw = JsonDocument.Parse(point.ToJsonString());
                    var observed = Mapping.At(raw.RootElement, field.Source);
                    if (observed is null) continue;
                    var kind = Mapping.ConvertValue(Mapping.Text(observed), field);
                    if (string.IsNullOrEmpty(kind) || !new[] { "일반공고", "변경공고", "취소공고", "재공고" }
                        .Any(status => row.Value == kind + "(" + status + ")")) break;
                    // 정정 덮개와 영수증은 그대로 두고, 입증된 원천 종류만 정규화한다.
                    head!["values"]!["notice_kind"] = kind;
                    c.Execute("UPDATE notice SET notice_kind=@kind WHERE notice_base=@Base AND seq=@Seq", new { kind, row.Base, row.Seq }, tx);
                    c.Execute("UPDATE erp_source SET document_json=@json WHERE entity_type='notice' AND entity_base=@Base AND entity_seq=@Seq",
                        new { json = document!.ToJsonString(), row.Base, row.Seq }, tx);
                    break;
                }
            }
            catch (Exception e) when (e is JsonException or InvalidOperationException or FormatException) { }
        }
    }

    internal static void Upgrade30(SqliteConnection connection, SqliteTransaction transaction, int previousVersion)
    {
        RepairFinanceOfficer(connection, transaction);
        RepairColumns(connection, transaction, previousVersion);
    }

    private static void RepairFinanceOfficer(SqliteConnection c, SqliteTransaction tx)
    {
        var rows = c.Query<(string Base, string Seq, string Value, string? Document)>("""
            SELECT n.notice_base,n.seq,n.finance_officer,s.document_json
            FROM notice n LEFT JOIN erp_source s
              ON s.entity_type='notice' AND s.entity_base=n.notice_base AND s.entity_seq=n.seq
            WHERE n.finance_officer IS NOT NULL
            """, transaction: tx).ToList();
        foreach (var row in rows)
        {
            var keep = false;
            try
            {
                var document = JsonNode.Parse(row.Document ?? "null");
                var head = document?["rows"]?.AsArray().FirstOrDefault(r => (string?)r?["table"] == "notice");
                // v29 이후 새 대상으로 실제 수집한 값은 옛 creditor 판올림과 관계없다.
                keep = head?["values"]?["finance_officer"] is JsonValue fresh && fresh.TryGetValue<string>(out var value) && value == row.Value;
                if (!keep && (string?)head?["values"]?["creditor"] == row.Value &&
                    (string?)head?["origins"]?["creditor"] is { } profile &&
                    profile is "g2b-notice-a-v1" or "g2b-notice-a-service-v1")
                    keep = ProvesLegacyOfficer(c, tx, row.Base, row.Seq, profile, row.Value);
            }
            catch (Exception e) when (e is JsonException or InvalidOperationException or FormatException) { }
            if (!keep)
                c.Execute("UPDATE notice SET finance_officer=NULL WHERE notice_base=@Base AND seq=@Seq", new { row.Base, row.Seq }, tx);
        }
    }

    private static bool ProvesLegacyOfficer(SqliteConnection c, SqliteTransaction tx, string @base, string seq, string profile, string value)
    {
        // erp_source.mapping_revision 은 다른 칸을 수집해도 바뀐다. 실제 수집 영수증의 판과 원값을 함께 확인한다.
        foreach (var snapshot in c.Query<string>("""
            SELECT snapshot_json FROM erp_capture WHERE profile=@profile AND entity_base=@base AND entity_seq=@seq
            ORDER BY captured_at DESC, capture_id DESC
            """, new { @base, seq, profile }, tx))
        {
            var receipt = JsonNode.Parse(snapshot);
            var revision = (string?)receipt?["mappingRevision"];
            var json = c.QuerySingleOrDefault<string>("SELECT json FROM erp_mapping WHERE revision=@revision", new { revision }, tx);
            if (json is null) return false; // 당시 기본 매핑 사본이 없으면 출처를 추정하지 않는다.
            var historical = JsonNode.Parse(json)?["profiles"]?.AsArray().SingleOrDefault(p => (string?)p?["id"] == profile);
            var field = historical?["fields"]?.AsArray().SingleOrDefault(f => (string?)f?["target"] == "creditor");
            if (field is null) continue;
            var source = (string?)field["source"];
            var raw = receipt?["data"]?["pointInfo"];
            if (raw is null || source is null) return false;
            using var document = JsonDocument.Parse(raw.ToJsonString());
            var sourceValue = Mapping.At(document.RootElement, source);
            if (sourceValue is null) continue; // 이 수집에서는 그 칸을 읽지 않았다.
            return source == "fncrNm" && ((string?)field["transform"] ?? "text") == "text" &&
                field["secondarySource"] is null && Mapping.Text(sourceValue) == value;
        }
        return false;
    }

    private static void RepairColumns(SqliteConnection c, SqliteTransaction tx, int previousVersion)
    {
        foreach (var entity in new[] { "notice", "contract", "request" })
        {
            var table = entity + "_user_field";
            var names = c.Query<string>($"""
                SELECT field_name FROM user_column WHERE entity_type=@entity
                UNION SELECT field_name FROM {table}
                """, new { entity }, tx).ToList();
            var occupied = names.ToHashSet(StringComparer.OrdinalIgnoreCase);
            // v28·v29 에 더한 머리 열만 충돌할 수 있다. 품목 열에는 사용자 열이 붙지 않는다.
            var builtins = Views.HiddenByDefault[Store.FaceView(entity)];
            foreach (var from in names.Where(n => builtins.Contains(n, StringComparer.OrdinalIgnoreCase)).Order(StringComparer.Ordinal).ToList())
            {
                string Candidate(int number)
                {
                    var suffix = "_사용자" + (number == 1 ? "" : number.ToString(System.Globalization.CultureInfo.InvariantCulture));
                    return from[..Math.Min(from.Length, 20 - suffix.Length)] + suffix;
                }
                var number = 1;
                var to = Candidate(number);
                while (occupied.Contains(to) || Views.BaseColumns(entity).Contains(to)) to = Candidate(++number);
                occupied.Add(to);
                c.Execute("UPDATE user_column SET field_name=@to WHERE entity_type=@entity AND field_name=@from", new { entity, from, to }, tx);
                c.Execute($"UPDATE {table} SET field_name=@to WHERE field_name=@from", new { from, to }, tx);
                var message = $"표의 새 열과 겹친 사용자 열 「{from}」을 「{to}」로 옮겼습니다. 이 식이 읽을 열을 확인하고 식을 고쳐 저장하세요.";
                var introduced = entity == "notice" && from == "재무관" ? 29 : 28;
                RepairFormulaReaders(c, tx, entity, from, message, previousVersion >= introduced);
                if (previousVersion < introduced) UserViewBuilder.RenameField(c, tx, entity, from, to);
            }
        }
    }

    private static void RepairFormulaReaders(SqliteConnection c, SqliteTransaction tx, string entity, string from,
        string message, bool ambiguousFlow)
    {
        bool Mark(JsonNode? formula)
        {
            if (formula is not JsonObject root) return false;
            var needsReview = false;
            if (root["source"] is JsonValue source && source.TryGetValue<string>(out var type) && type == "sql" &&
                root["sql"] is JsonValue sql && sql.TryGetValue<string>(out var text))
            {
                // shortcut: 다른 뷰의 동명 참조도 수동 확인한다. SQL 자동 이관이 필요해지면 파서로 참조를 해소한다.
                try { needsReview = SqlColumn.CheckShape(text).Contains(from); }
                catch (InvalidOperationException) { } // 이미 깨진 SQL 원문도 그대로 둔다.
            }
            else if (ambiguousFlow)
                needsReview = UserViewBuilder.Touch(root, entity, from, null);
            if (!needsReview) return false;
            if (root["migrationError"] is JsonObject previous)
            {
                var existing = (string?)previous["message"] ?? "";
                if (existing.Contains(message, StringComparison.Ordinal)) return false;
                previous["message"] = existing + "\n" + message;
                return true;
            }
            root["migrationError"] = new JsonObject { ["message"] = message, ["original"] = root.DeepClone() };
            return true;
        }
        foreach (var column in c.Query<(string Entity, string Name, string Options)>("""
            SELECT entity_type,field_name,options FROM user_column WHERE options IS NOT NULL AND options LIKE '{%'
            """, transaction: tx).ToList())
        {
            JsonNode? formula;
            try { formula = JsonNode.Parse(column.Options); }
            catch (JsonException) { continue; }
            if (Mark(formula))
                c.Execute("UPDATE user_column SET options=@options WHERE entity_type=@Entity AND field_name=@Name",
                    new { column.Entity, column.Name, options = formula!.ToJsonString(Flow.Json) }, tx);
        }
        foreach (var view in UserViewBuilder.Read(c, tx))
        {
            JsonNode? definition;
            try { definition = JsonNode.Parse(view.Definition); }
            catch (JsonException) { continue; }
            if (definition?["columns"] is not JsonArray columns) continue;
            var changed = false;
            foreach (var column in columns.OfType<JsonObject>()) changed |= Mark(column["formula"]);
            if (changed) c.Execute("UPDATE user_view SET definition=@definition WHERE id=@Id",
                new { view.Id, definition = definition.ToJsonString(Flow.Json) }, tx);
        }
    }
}
