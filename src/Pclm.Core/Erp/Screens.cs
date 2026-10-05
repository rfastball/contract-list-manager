using System.Text.Json;
using System.Text.RegularExpressions;

namespace Pclm.Core.Erp;

/// <summary>
/// 수집하는 나라장터 화면 — 화면의 메뉴 번호(다섯 자리)로 가린 허용 목록이다(ADR-037). 목록은 따로 두지 않는다: 매핑에서
/// <see cref="MappingProfile.Screen"/> 을 단 프로필들이 그것이고, 같은 프로필이 그 화면의 고유키를 들고 있다. 받는 것은 번호가
/// 목록에 있고 <b>그리고</b> 그 프로필의 고유키가 읽힐 때뿐이다 — 뒤의 것은 <see cref="Mapping.Map"/> 이 본다. 확장은 hello 의
/// 매핑에서 같은 목록을 끌어내고, 호스트는 수집 입력마다 여기서 다시 본다.
///
/// <para>번호를 읽지 못한 화면·파일도 받지 않는다. 받는 화면을 넓힐 때는 기본 <c>mapping.json</c> 의 그 프로필에 <c>screen</c>
/// 을 단다(고유키와 필드도 함께).</para>
/// </summary>
public static class Screens
{
    /// <summary>수집하는 화면들 — 매핑에서 화면을 단 프로필.</summary>
    public static IEnumerable<MappingProfile> Supported(MappingSet mapping) => mapping.Profiles.Where(p => p.Screen is not null);

    private static readonly Regex Code = new(@"\A\d{5}\z", RegexOptions.CultureInvariant);

    /// <summary>메뉴 번호의 모양(숫자 다섯)인가.</summary>
    public static bool IsCode(string? value) => value is not null && Code.IsMatch(value);

    /// <summary>그 화면을 읽는 프로필. 목록에 없으면 null.</summary>
    public static MappingProfile? ProfileFor(MappingSet mapping, string? code) =>
        IsCode(code) ? Supported(mapping).SingleOrDefault(p => p.Screen!.Code == code) : null;

    /// <summary>
    /// ERP JSON 의 <c>pointInfo</c> 가 적은 메뉴 경로에서 그 화면의 번호 — 비지 않은 가장 깊은 단(depth3 → depth2 → depth1).
    /// 없거나 모양이 틀리면 null.
    /// </summary>
    public static string? Leaf(JsonElement pointInfo)
    {
        if (pointInfo.ValueKind != JsonValueKind.Object) return null;
        foreach (var name in new[] { "depth3", "depth2", "depth1" })
        {
            if (!pointInfo.TryGetProperty(name, out var value) || value.ValueKind != JsonValueKind.String) continue;
            var text = value.GetString()?.Trim();
            if (string.IsNullOrEmpty(text)) continue;
            return IsCode(text) ? text : null;
        }
        return null;
    }

    /// <summary>
    /// 그 화면의 자료를 이 프로필로 받아도 되는가. 번호를 모르거나, 목록에 없거나, 그 화면의 프로필이 아니면 사람이 읽을 까닭을
    /// 담아 던진다.
    /// </summary>
    /// <param name="live">확장이 읽은 실제 화면인가. 파일이면 프로필을 고쳐 고르라고 말한다.</param>
    public static void Check(MappingSet mapping, string? code, string profile, bool live)
    {
        if (!IsCode(code))
            throw new InvalidOperationException(live
                ? "화면 번호를 읽지 못했습니다. 접수·공고·계약 상세 화면에서 다시 읽어 주세요."
                : "이 JSON 에는 화면 번호(depth)가 없어 받지 않습니다.");
        if (ProfileFor(mapping, code) is not { } expected)
            throw new InvalidOperationException($"메뉴 {code} 화면은 수집하지 않습니다.");
        if (expected.Id == profile) return;
        var name = Mirror.KindName(expected.EntityType);
        throw new InvalidOperationException(live
            ? $"{name} 화면(메뉴 {code})인데 다른 프로필로 읽혔습니다. 화면을 다시 읽어 주세요."
            : $"{name} 화면(메뉴 {code})의 JSON 입니다. {name} 프로필({expected.Id})로 여세요.");
    }
}
