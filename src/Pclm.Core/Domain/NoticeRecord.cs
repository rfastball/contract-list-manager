namespace Pclm.Core.Domain;

/// <summary>입찰공고 한 건. 차수까지가 자연키다 — 변경공고는 별개의 행으로 쌓인다.</summary>
public sealed record NoticeRecord
{
    /// <summary>본번호. <c>R26BK09017075</c></summary>
    public required string NoticeBase { get; init; }

    /// <summary>차수. 원공고가 <c>000</c>, 변경될 때마다 오른다.</summary>
    public required string Seq { get; init; }

    public string Key => $"{NoticeBase}-{Seq}";

    // ── 일반 ─────────────────────────────────────────────
    public string? Title { get; init; }
    public string? NoticeKind { get; init; }
    public DateTime? PostedAt { get; init; }
    public string? BidMethod { get; init; }
    public string? AwardMethod { get; init; }
    public string? AwardCriteria { get; init; }
    public decimal? LowestBidRatio { get; init; }
    public string? ContractMethod { get; init; }
    public string? ContractKind { get; init; }
    public string? InternationalKind { get; init; }
    public string? Creditor { get; init; }
    public bool? RebidAllowed { get; init; }
    public bool? PartialDeliveryAllowed { get; init; }
    public int? WarrantyMonths { get; init; }

    // ── 금액 ─────────────────────────────────────────────
    public string? PriceEstimationMethod { get; init; }
    public decimal? ProjectAmount { get; init; }
    public decimal? AllocatedBudget { get; init; }
    public decimal? EstimatedPrice { get; init; }
    public decimal? BasePrice { get; init; }

    // ── 기관 ─────────────────────────────────────────────
    public string? NoticeAgency { get; init; }
    public string? NoticeOfficer { get; init; }
    public string? ExecutiveOfficer { get; init; }
    public string? PriorSpecNumber { get; init; }

    // ── 자격 ─────────────────────────────────────────────
    public string? RegionRestriction { get; init; }
    public bool? PerformanceRestricted { get; init; }
    public string? ManufacturingKind { get; init; }
    public bool? ForeignAllowed { get; init; }
    public bool? ResearchItem { get; init; }

    public IReadOnlyList<NoticeItemRecord> Items { get; init; } = [];
    public IReadOnlyList<NoticeScheduleRecord> Schedule { get; init; } = [];
    public IReadOnlyList<NoticeContactRecord> Contacts { get; init; } = [];

    /// <summary>
    /// 관련공고. 이 공고가 대신하거나 이 공고를 대신하는 공고의 번호다 — 변경공고는 당초를,
    /// 당초는 (변경 뒤에 다시 내려받으면) 변경분을, 재공고는 취소공고와 그 당초를 함께 적는다.
    ///
    /// <para><b>재공고는 본번호가 갈린다.</b> <c>R26BK09011054</c> 가 취소되고
    /// <c>R26BK09012082</c> 로 다시 나가므로 차수로는 이어지지 않는다 — 이 칸이 둘을 잇는
    /// <b>문서상의 유일한 단서</b>다. 담기만 하고 계열의 생사를 여기서 정하지는 않는다.</para>
    /// </summary>
    public IReadOnlyList<string> RelatedNotices { get; init; } = [];

    /// <summary>일정에서 개찰 시각을 골라 온다. 문서 생성에서 자주 쓰는 값이라 따로 낸다.</summary>
    public DateTime? OpeningAt =>
        Schedule.FirstOrDefault(s => s.Name?.Contains("개찰") == true)?.StartsAt;

    public DateTime? BidClosesAt =>
        Schedule.FirstOrDefault(s => s.Name?.Contains("입찰서제출") == true)?.EndsAt;

    public DateTime? RegistrationClosesAt =>
        Schedule.FirstOrDefault(s => s.Name?.Contains("입찰참가자격등록") == true)?.EndsAt;
}

/// <summary>구매대상물품 한 줄.</summary>
public sealed record NoticeItemRecord
{
    public int LineNo { get; init; }
    public string? DemandAgency { get; init; }
    public string? ItemName { get; init; }
    public string? DetailItemNumber { get; init; }
    public string? ItemIdNumber { get; init; }
    public string? Specification { get; init; }
    public int? Quantity { get; init; }
    public string? Unit { get; init; }
    public decimal? UnitPrice { get; init; }
    public int? DeliveryDays { get; init; }

    /// <summary>공고 단계에서는 대개 <c>-</c> 라 비어 있다. 날짜가 박히는 것은 계약서에서다.</summary>
    public DateTime? DeliveryDue { get; init; }

    public string? DeliveryPlace { get; init; }
    public string? DeliveryTerms { get; init; }
}

/// <summary>입찰진행정보 한 줄.</summary>
public sealed record NoticeScheduleRecord
{
    public int LineNo { get; init; }
    public string? Name { get; init; }
    public string? Method { get; init; }
    public DateTime? StartsAt { get; init; }
    public DateTime? EndsAt { get; init; }
    public string? Place { get; init; }
}

/// <summary>
/// 수요기관담당자정보목록 한 줄. 공고를 낸 조달청 담당자가 아니라 <b>물건을 받는 기관</b> 쪽 사람이다.
///
/// <para>수요기관이 여럿인 공고가 있어 목록으로 받는다 — 한 줄로 눌러 담으면 둘째부터 사라진다.</para>
///
/// <para>담을 것은 기관의 <b>업무 연락처</b>뿐이다. 양식에 따라 이름이 <c>홍**</c> 처럼
/// 가려진 채 나오는데, 가려진 그대로 담는다(ADR-016).</para>
/// </summary>
public sealed record NoticeContactRecord
{
    public int LineNo { get; init; }
    public string? DemandAgency { get; init; }
    public string? Department { get; init; }
    public string? Officer { get; init; }
    public string? Fax { get; init; }
    public string? Phone { get; init; }
}
