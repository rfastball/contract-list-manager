using System.Text.Json;
using Dapper;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

/// <summary>로컬 앱 전용 고급 도구. Native Messaging은 이 관리 API를 노출하지 않는다.</summary>
public static class ErpTools
{
    public static object? Invoke(Database db, string operation, string json)
    {
        var p = JsonDocument.Parse(json).RootElement;
        var mappings = new MappingStore(db);
        switch (operation)
        {
            case "list": return mappings.List();
            case "saveMapping": return mappings.Save(p.GetProperty("json").GetString()!);
            case "activateMapping": mappings.Activate(p.GetProperty("revision").GetString()!); return mappings.List();
            case "sample": return mappings.ValidateSample(p.GetProperty("json").GetString()!, p.GetProperty("profile").GetString()!, p.GetProperty("data"));
            case "inspect": return new ErpCapture(db).Inspect(p.Deserialize<CaptureInput>(Mapping.Json)!);
            case "capture": return new ErpCapture(db).Save(p.GetProperty("snapshot").Deserialize<CaptureInput>(Mapping.Json)!,
                p.GetProperty("baseToken").GetString()!, p.GetProperty("captureId").GetString()!,
                p.GetProperty("choices").Deserialize<Dictionary<string, string>>(Mapping.Json)!);
            case "references":
                using (var c = db.Open())
                using (var tx = c.BeginTransaction())
                { var result = ExplicitLinks.Resolve(c, tx); tx.Commit(); return result; }
            case "history":
                using (var c = db.OpenReadOnly())
                    return c.Query("SELECT capture_id,profile,entity_base,entity_seq,captured_at,snapshot_json FROM erp_capture ORDER BY captured_at DESC LIMIT 100");
            default: throw new InvalidOperationException("지원하지 않는 고급 도구입니다.");
        }
    }
}
