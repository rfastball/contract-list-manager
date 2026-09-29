namespace Pclm.Core.Domain;

/// <summary>계약 한 건. 계약번호 끝 두 자리가 차수이며 변경계약마다 오른다.</summary>
public sealed record ContractRecord
{
    /// <summary>본번호. <c>R26TA09110507</c></summary>
    public required string ContractBase { get; init; }

    /// <summary>차수. 원계약이 <c>00</c>.</summary>
    public required string Seq { get; init; }

    public string Key => $"{ContractBase}{Seq}";

    // ── 계약 ─────────────────────────────────────────────
    public string? Title { get; init; }
    public DateTime? ContractedOn { get; init; }
    public string? ContractMethod { get; init; }
    public string? LawClause { get; init; }
    public string? GoodsOrService { get; init; }
    public string? PurchaseManagementNumber { get; init; }
    public string? AgencyManagementNumber { get; init; }
    public string? ContractKind { get; init; }
    public string? ContractCharacter { get; init; }
    public bool? Terminated { get; init; }
    public DateTime? TerminatedOn { get; init; }

    // ── 품목·금액 ────────────────────────────────────────
    public string? ItemName { get; init; }
    public int? Quantity { get; init; }
    public string? Unit { get; init; }
    public decimal? Amount { get; init; }
    public decimal? Fee { get; init; }
    public decimal? DelayPenaltyRate { get; init; }
    public decimal? WarrantyBondRate { get; init; }
    public string? WarrantyPeriod { get; init; }

    // ── 이행 ─────────────────────────────────────────────
    public string? ContractPeriod { get; init; }
    public DateTime? DeliveryDue { get; init; }
    public string? DeliveryTerms { get; init; }
    public string? DeliveryPlace { get; init; }
    public bool? PartialDeliveryAllowed { get; init; }
    public string? PaymentMethod { get; init; }

    // ── 기관 ─────────────────────────────────────────────
    public string? DemandAgency { get; init; }
    public string? InspectionAgency { get; init; }
    public string? AcceptanceAgency { get; init; }

    /// <summary>계약상대자. 주민등록번호는 담지 않는다 — 파싱 단계에서 버린다(ADR-005).</summary>
    public CounterpartyRecord? Counterparty { get; init; }

    public IReadOnlyList<ContractItemRecord> Items { get; init; } = [];
    public IReadOnlyList<ContractAttachmentRecord> Attachments { get; init; } = [];
}

/// <summary>
/// 계약상대자. 상호·사업자등록번호·대표자는 계약 문서에 인쇄되는 사업체 정보라 담고,
/// 주민등록번호는 담지 않는다.
/// </summary>
public sealed record CounterpartyRecord
{
    public string? Name { get; init; }
    public string? Address { get; init; }
    public string? Representative { get; init; }
    public string? BusinessNumber { get; init; }
    public string? Phone { get; init; }
    public string? Fax { get; init; }
}

/// <summary>계약물품명세서 한 줄.</summary>
public sealed record ContractItemRecord
{
    public int LineNo { get; init; }
    public string? ClassificationNumber { get; init; }
    public string? ItemIdNumber { get; init; }
    public string? ItemName { get; init; }
    public string? Specification { get; init; }
    public string? Region { get; init; }
    public string? Unit { get; init; }
    public string? DeliveryTerms { get; init; }
    public int? Quantity { get; init; }
    public decimal? UnitPrice { get; init; }
    public decimal? Amount { get; init; }
    public string? DemandAgency { get; init; }
    public DateTime? DeliveryDue { get; init; }
}

/// <summary>첨부문서 목록 한 줄. 파일은 다루지 않고 목록만 남긴다.</summary>
public sealed record ContractAttachmentRecord
{
    public int LineNo { get; init; }
    public string? DocumentType { get; init; }
    public string? FileName { get; init; }
}
