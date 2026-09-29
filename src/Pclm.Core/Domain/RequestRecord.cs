namespace Pclm.Core.Domain;

/// <summary>
/// 국방계약요청접수서 한 건 — 조달의 <b>맨 앞칸</b>이다.
///
/// <para>수요기관이 조달요구번호마다 요청을 내고, 조달청이 그것을 모아 하나의 접수서로 받은 뒤
/// 하나의 공고를 낸다. 그래서 접수서 한 장은 <b>요청 여럿을 묶은 것</b>이고, 줄 하나가
/// 조달요구 하나다(<see cref="RequestItemRecord.RequestNumber"/>).</para>
///
/// <para><b>접수의 키는 접수번호·접수차수다</b>(ADR-027).</para>
/// </summary>
public sealed record RequestRecord
{
    /// <summary>접수번호.</summary>
    public required string RequestBase { get; init; }

    /// <summary>접수차수. 원접수가 <c>000</c>.</summary>
    public required string Seq { get; init; }

    public string Key => $"{RequestBase}-{Seq}";

    // ── 요청기본정보 ─────────────────────────────────────
    public string? Title { get; init; }
    public DateTime? ReceivedOn { get; init; }
    public string? BusinessKind { get; init; }
    public string? ContractLaw { get; init; }
    public string? ContractMethod { get; init; }
    public string? ContractKind { get; init; }
    public string? AwardMethod { get; init; }
    public string? AccountingKind { get; init; }
    public string? PaymentMethod { get; init; }

    // ── 금액 ─────────────────────────────────────────────
    /// <summary>품대. 공고의 사업금액에 대응한다.</summary>
    public decimal? GoodsAmount { get; init; }
    public decimal? Fee { get; init; }
    public decimal? Vat { get; init; }

    /// <summary>예산금액. 공고의 배정예산에 대응한다.</summary>
    public decimal? BudgetAmount { get; init; }

    // ── 구분 ─────────────────────────────────────────────
    public bool? ForeignAllowed { get; init; }
    public string? RequestKind { get; init; }
    public string? Disclosure { get; init; }
    public string? ExecutiveOfficer { get; init; }
    public string? Department { get; init; }
    public string? Officer { get; init; }
    public bool? AdvanceNotice { get; init; }
    public bool? AdvancePayment { get; init; }
    public string? Remarks { get; init; }

    // ── 기관담당자정보 ───────────────────────────────────
    public string? DemandAgency { get; init; }
    public string? DemandAgencyCode { get; init; }
    public string? AgencyOfficer { get; init; }
    public string? AgencyPhone { get; init; }
    public string? AgencyFax { get; init; }

    public IReadOnlyList<RequestItemRecord> Items { get; init; } = [];
}

/// <summary>
/// 요청물품목록 한 줄 = <b>조달요구 하나</b>.
///
/// <para><see cref="RequestNumber"/> 가 그 조달요구번호다. 여기 실린 <b>수량과 단가</b>가
/// 공고와 접수를 잇는 열쇠가 된다 — 두 문서 어디에도 서로를 가리키는 키가 없어서다
/// (ADR-021). <see cref="DetailItemNumber"/> 는 협의·요청 누락으로 공고와 다를 수 있어
/// 잇기의 근거로 쓰지 않는다.</para>
/// </summary>
public sealed record RequestItemRecord
{
    public int LineNo { get; init; }

    /// <summary>요청번호 = 조달요구번호. 이 줄의 정체다.</summary>
    public string? RequestNumber { get; init; }

    public string? PlanYear { get; init; }
    public string? ChangeSeq { get; init; }
    public string? DetailItemNumber { get; init; }
    public string? ItemIdNumber { get; init; }
    public string? ItemName { get; init; }
    public bool? Cancelled { get; init; }
    public string? Specification { get; init; }
    public string? InspectionKind { get; init; }

    public decimal? UnitPrice { get; init; }
    public int? Quantity { get; init; }
    public decimal? Amount { get; init; }
    public string? Unit { get; init; }
    public int? Pages { get; init; }

    /// <summary>
    /// 납품일수. <b>문자열이다</b> — 접수서는 이 칸에 <c>계약후 90일 이내</c> 처럼 문장을 적는다.
    /// 숫자로 억지로 바꾸면 적힌 것이 사라진다(ADR-016).
    /// </summary>
    public string? DeliveryDays { get; init; }

    public string? DeliveryTerms { get; init; }
    public DateTime? DeliveryDue { get; init; }
    public string? DeliveryPlace { get; init; }
    public string? StockNumber { get; init; }
    public string? SpecNumber { get; init; }
    public string? ExpenseRequestNumber { get; init; }
}
