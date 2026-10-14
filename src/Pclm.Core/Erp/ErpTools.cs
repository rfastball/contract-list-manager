using System.Text.Json;
using Dapper;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

/// <summary>로컬 앱 전용 고급 도구. Native Messaging은 이 관리 API를 노출하지 않는다.</summary>
public static class ErpTools
{
    /// <param name="home">이 파일을 가리켜야 하는 쪽지의 홈. 창이 넘기면 「검토한 자료 저장」 이 쓰기 잠금 안에서 쪽지가 아직
    /// 이 파일을 가리키는지 다시 본다(ADR-032) — 「지금 보는 화면」 의 가져오기와 같은 확인이다.</param>
    public static object? Invoke(Database db, string operation, string json, Home? home = null)
    {
        var p = JsonDocument.Parse(json).RootElement;
        var mappings = new MappingStore(db);
        switch (operation)
        {
            case "list": return mappings.List();
            case "saveMapping": return mappings.Save(p.GetProperty("json").GetString()!);
            case "activateMapping": mappings.Activate(p.GetProperty("revision").GetString()!); return mappings.List();
            case "sample": return mappings.ValidateSample(p.GetProperty("json").GetString()!, p.GetProperty("profile").GetString()!, p.GetProperty("data"));
            case "inspect": return new ErpCapture(db, home: home).Inspect(p.Deserialize<CaptureInput>(Mapping.Json)!);
            case "capture": return new ErpCapture(db, home: home).Save(p.GetProperty("snapshot").Deserialize<CaptureInput>(Mapping.Json)!,
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
