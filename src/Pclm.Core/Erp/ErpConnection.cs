using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

/// <summary>
/// 확장 호스트가 저장 대상을 찾는 길. <b>홈의 쪽지 하나를 읽기만 한다</b> — 창·명령줄과 같은 쪽지다.
///
/// <para>한동안 호스트는 따로 <c>erp-connection.json</c> 을 봤고, 둘을 맞추는 것은 "앱이 시작할 때 적어
/// 준다" 는 관례뿐이었다(ADR-030). 쪽지가 둘이면 어느 하나가 늙은 채로 남아도 아무도 모른다.</para>
///
/// <para>호스트는 <b>쪽지를 쓰지 않고 판을 올리지도 않는다.</b> 그 둘은 창의 일이다 — 브라우저가 띄운
/// 프로세스가 저장 구조를 고치면 사람이 모르는 사이에 자료가 바뀐다.</para>
/// </summary>
public static class ErpConnection
{
    public const string HostName = "kr.rfastball.pclm.erp";

    /// <summary>
    /// 쪽지가 가리키는 작업자료를 연다. 쪽지가 멀쩡하고, 파일이 지금 판의 작업자료이고, 쪽지에 적힌 신원과
    /// 같아야만 연다.
    /// </summary>
    /// <exception cref="InvalidOperationException">그 밖의 모든 경우. 메인 프로그램을 다시 띄우라고 말한다.</exception>
    public static Database OpenBound(Home home)
    {
        var read = home.ReadConfig();
        if (read.State != ConfigState.Ok)
            throw new InvalidOperationException("메인 프로그램에서 저장 대상을 준비하세요.");
        var config = read.Config!;

        // 판별은 PclmFile 한 곳이 낸다. 작업자료가 아니면 여기서 PclmFileException 으로 멈춘다.
        var database = PclmFile.OpenWork(config.Workfile);

        // 판이 낮으면 창이 아직 올리지 않은 것이다. 호스트는 올리지 않는다 — 올리는 것은 백업과 함께 창이 한다.
        // 쪽지의 신원과 다르면 그 자리에 다른 작업자료가 놓였다. 옛 검토 화면이 엉뚱한 파일에 저장하지 않게 막는다.
        var info = PclmFile.Inspect(database.Path);
        if (info.Version != Schema.Version || info.DatasetId != config.DatasetId)
            throw new InvalidOperationException("연결된 자료가 바뀌었습니다. 메인 프로그램을 다시 실행하세요.");

        return database;
    }

    /// <summary>
    /// 호스트를 부른 확장이 우리가 풀어 둔 그 확장인가. 출처는 내장 확장을 준비할 때 홈에 적힌다 —
    /// 업무·개발 호스트가 같은 길을 쓰고 홈만 다르다.
    /// </summary>
    public static bool IsAllowedOrigin(Home home, string origin) =>
        System.Text.RegularExpressions.Regex.IsMatch(origin, @"\Achrome-extension://[a-p]{32}/\z") &&
        File.Exists(home.ExtensionOriginPath) &&
        File.ReadAllText(home.ExtensionOriginPath).Trim() == origin;
}
