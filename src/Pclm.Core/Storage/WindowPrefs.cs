using System.Text.Json;
using System.Text.Json.Serialization;

namespace Pclm.Core.Storage;

/// <summary>
/// 창의 몸가짐 — 이 컴퓨터에서 창을 어떻게 다룰지. 홈의 <c>window.json</c>(<see cref="Home.WindowPrefsPath"/>)에 적는다.
///
/// <para><b>자료가 아니라 자리의 것이다.</b> 작업자료의 설정(<c>app_setting</c>)에 두면 제출본·작업자료 바꾸기를 따라
/// 남의 컴퓨터로 건너가고, 쪽지(<c>config.json</c>)에 두면 밖에 내놓은 약속(dataset-contract §1)이 커진다.
/// 그래서 홈에 따로 둔다.</para>
///
/// <para>읽기는 <b>너그럽다</b> — 없거나 깨졌으면 기본값(모두 꺼짐)으로 선다. 창을 막을 일이 아니고, 깨진 몸가짐을
/// 사람에게 물을 까닭도 없다. 쓰기는 쪽지처럼 임시 파일에 쓴 뒤 바꿔치기한다.</para>
/// </summary>
/// <param name="CloseToTray">닫기(X)를 눌러도 끝내지 않고 알림 영역에 둔다.</param>
/// <param name="TrayNoticeShown">알림 영역에 처음 둘 때의 풍선을 이미 보였다. 한 번만 보인다.</param>
public sealed record WindowPrefs(
    [property: JsonPropertyName("closeToTray")] bool CloseToTray = false,
    [property: JsonPropertyName("trayNoticeShown")] bool TrayNoticeShown = false)
{
    private static readonly JsonSerializerOptions Json = new() { WriteIndented = true };

    /// <summary>읽는다. 예외를 내지 않는다 — 없거나 깨졌거나 못 읽으면 기본값이다.</summary>
    public static WindowPrefs Read(string path)
    {
        try
        {
            if (!File.Exists(path)) return new();
            return JsonSerializer.Deserialize<WindowPrefs>(File.ReadAllText(path), Json) ?? new();
        }
        catch (Exception e) when (e is JsonException or IOException or UnauthorizedAccessException or NotSupportedException)
        {
            return new();
        }
    }

    /// <summary>
    /// 쓴다. 같은 폴더의 임시 파일에 쓴 뒤 이름을 바꾼다 — 쓰다 끊겨 반쯤 쓴 파일이 남으면 다음에 기본값으로 물러나
    /// 사람이 켜 둔 것이 조용히 꺼진다.
    /// </summary>
    public void Write(string path)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!);
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllText(temporary, JsonSerializer.Serialize(this, Json));
            File.Move(temporary, path, overwrite: true);
        }
        finally
        {
            if (File.Exists(temporary)) File.Delete(temporary);
        }
    }
}
