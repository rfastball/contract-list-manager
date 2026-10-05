namespace Pclm.Core.Erp;

/// <summary>
/// 개발 호스트. 업무 호스트와 다른 것은 <b>이름과 홈</b>뿐이다 — 홈은 <see cref="Storage.Home.Development"/>,
/// 허용 출처 확인은 업무 호스트와 한 길(<see cref="ErpConnection.IsAllowedOrigin"/>)이다.
/// 같은 일을 두 벌로 두면 한쪽만 고쳐져 개발에서 통과한 것이 업무에서 막힌다.
/// </summary>
public static class ErpDevelopment
{
    public const string HostName = "kr.rfastball.pclm.erp.dev";
}
