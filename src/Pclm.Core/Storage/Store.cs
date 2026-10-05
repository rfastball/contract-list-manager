using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;

namespace Pclm.Core.Storage;

/// <summary>접수·공고·계약 하나를 가리키는 것. <b>본번호와 차수가 실제 키다.</b></summary>
public sealed record EntityRef(string EntityType, string Base, string Seq)
{
    /// <summary>
    /// 사람에게 보여줄 때 쓰는 한 덩어리 표기. 저장이나 조인에는 쓰지 않는다.
    /// 접수는 공고처럼 붙임표로 잇는다 — 계약만 끝 두 자리를 그대로 붙인다.
    /// </summary>
    public string Display => EntityType == "contract" ? $"{Base}{Seq}" : $"{Base}-{Seq}";
}

/// <summary>
/// 사람이 고쳐 둔 칸 하나. <paramref name="Original"/> 은 <b>처음 고칠 때 그 자리에 있던 값</b>이다 —
/// 화면이 무엇을 무엇으로 고쳤는지 보이는 데 쓴다.
/// </summary>
public sealed record FieldOverride(string Base, string Seq, string ColumnName, string Original);

/// <summary>
/// 지우면 무엇이 함께 사라지는가. <b>묻기 전에 세어 보여 준다</b> — 되돌릴 수 없는 일이라
/// 사람이 숫자를 보고 눌러야 한다.
/// </summary>
/// <param name="Revisions">이 본번호로 쌓인 차수 전부. 낮은 것부터.</param>
/// <param name="SeriesGoes">
/// 계열까지 걷히는가. 계열이 걷히면 <b>사람이 적은 값과 링크도 함께 간다</b>. 차수 하나만
/// 지우더라도 그것이 마지막 차수라면 계열은 남을 자리가 없으므로 함께 걷힌다.
/// </param>
public sealed record DeletionPlan(
    string EntityType,
    string Display,
    string Seq,
    string Title,
    IReadOnlyList<string> Revisions,
    bool SeriesGoes,
    int ChildRows,
    int UserFields,
    int Overrides,
    bool Linked);

/// <summary>
/// 들어온 자료 위에서 사람이 하는 일(사람 열·덮개·지우기)과 키 해석을 한 자리에 모은다.
///
/// <para>자료는 확장 수집·ERP JSON·계획 엑셀로만 들어온다(ADR-028). 들이는 쓰기는
/// <c>ErpCapture</c>·<c>PlanImport</c> 가 맡는다.</para>
///
/// <para><see cref="Schema"/> 의 <c>user_field</c> 와 <c>field_override</c> 는 자료를 들이는 길이
/// 쓰지 않는다 — 사람이 적은 값이 다시 수집해도 지워지지 않게 하는 구조적 보장이다(ADR-007·020).</para>
/// </summary>
public sealed class Store(Database database)
{
    private readonly Database _database = database;

    // ── 시험 픽스처 ──────────────────────────────────────────────
    // 아래 셋은 프로덕션 호출자가 없다 — 시험 픽스처 전용이다(ADR-028). 자료를 세우는 시험
    // 수십 개가 이것을 쓰고 있어 좁혀 남겼다. 자연키 업서트라 같은 레코드를 다시 넣어도 행이
    // 늘지 않고, 차수가 다르면 별개 행으로 쌓인다.

    internal void UpsertNotice(NoticeRecord notice)
    {
        var now = DateTime.UtcNow.ToString("O");

        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();

        // 계열 먼저. 사람이 붙인 값과 링크가 이 표를 가리키므로 차수 행보다 앞서 있어야 한다.
        //
        // 건도 여기서 세운다. <b>group_base 를 함께 넣지 않으면 NOT NULL 위반을 OR IGNORE 가
        // 삼켜</b> 공고는 들어가는데 계열이 서지 않는다 — 오류 한 줄 없이 사람 값도 링크도 붙지
        // 않는 공고가 생긴다. 값은 제 본번호다. 관련공고를 타고 합치는 것은 끝의 Rebuild 다.
        connection.Execute(
            "INSERT OR IGNORE INTO notice_group (group_base) VALUES (@b);",
            new { b = notice.NoticeBase }, transaction);

        connection.Execute(
            "INSERT OR IGNORE INTO notice_series (notice_base, group_base) VALUES (@b, @b);",
            new { b = notice.NoticeBase }, transaction);

        connection.Execute(
            """
            INSERT INTO notice (
                notice_base, seq, title, notice_kind, posted_at, bid_method, award_method,
                award_criteria, lowest_bid_ratio, contract_method, contract_kind,
                international_kind, creditor, rebid_allowed, partial_delivery_allowed,
                warranty_months, price_estimation_method, project_amount, allocated_budget,
                estimated_price, base_price, notice_agency, notice_officer, executive_officer,
                prior_spec_number, region_restriction, performance_restricted,
                manufacturing_kind, foreign_allowed, research_item, updated_at
            ) VALUES (
                @NoticeBase, @Seq, @Title, @NoticeKind, @PostedAt, @BidMethod, @AwardMethod,
                @AwardCriteria, @LowestBidRatio, @ContractMethod, @ContractKind,
                @InternationalKind, @Creditor, @RebidAllowed, @PartialDeliveryAllowed,
                @WarrantyMonths, @PriceEstimationMethod, @ProjectAmount, @AllocatedBudget,
                @EstimatedPrice, @BasePrice, @NoticeAgency, @NoticeOfficer, @ExecutiveOfficer,
                @PriorSpecNumber, @RegionRestriction, @PerformanceRestricted,
                @ManufacturingKind, @ForeignAllowed, @ResearchItem, @Now
            )
            ON CONFLICT(notice_base, seq) DO UPDATE SET
                title = excluded.title, notice_kind = excluded.notice_kind,
                posted_at = excluded.posted_at, bid_method = excluded.bid_method,
                award_method = excluded.award_method, award_criteria = excluded.award_criteria,
                lowest_bid_ratio = excluded.lowest_bid_ratio,
                contract_method = excluded.contract_method, contract_kind = excluded.contract_kind,
                international_kind = excluded.international_kind, creditor = excluded.creditor,
                rebid_allowed = excluded.rebid_allowed,
                partial_delivery_allowed = excluded.partial_delivery_allowed,
                warranty_months = excluded.warranty_months,
                price_estimation_method = excluded.price_estimation_method,
                project_amount = excluded.project_amount,
                allocated_budget = excluded.allocated_budget,
                estimated_price = excluded.estimated_price, base_price = excluded.base_price,
                notice_agency = excluded.notice_agency, notice_officer = excluded.notice_officer,
                executive_officer = excluded.executive_officer,
                prior_spec_number = excluded.prior_spec_number,
                region_restriction = excluded.region_restriction,
                performance_restricted = excluded.performance_restricted,
                manufacturing_kind = excluded.manufacturing_kind,
                foreign_allowed = excluded.foreign_allowed, research_item = excluded.research_item,
                updated_at = excluded.updated_at;
            """,
            new
            {
                notice.NoticeBase, notice.Seq, notice.Title, notice.NoticeKind, notice.PostedAt,
                notice.BidMethod, notice.AwardMethod, notice.AwardCriteria, notice.LowestBidRatio,
                notice.ContractMethod, notice.ContractKind, notice.InternationalKind, notice.Creditor,
                notice.RebidAllowed, notice.PartialDeliveryAllowed, notice.WarrantyMonths,
                notice.PriceEstimationMethod, notice.ProjectAmount, notice.AllocatedBudget,
                notice.EstimatedPrice, notice.BasePrice, notice.NoticeAgency, notice.NoticeOfficer,
                notice.ExecutiveOfficer, notice.PriorSpecNumber, notice.RegionRestriction,
                notice.PerformanceRestricted, notice.ManufacturingKind, notice.ForeignAllowed,
                notice.ResearchItem, Now = now,
            }, transaction);

        // 딸린 줄은 통째로 갈아 끼운다. 줄이 줄어든 변경공고에서 옛 줄이 남지 않게.
        ReplaceChildren(connection, transaction,
            "DELETE FROM notice_item WHERE notice_base = @b AND seq = @s;",
            new { b = notice.NoticeBase, s = notice.Seq });

        foreach (var item in notice.Items)
            connection.Execute(
                """
                INSERT INTO notice_item (
                    notice_base, seq, line_no, demand_agency, item_name, detail_item_number,
                    item_id_number, specification, quantity, unit, unit_price,
                    delivery_days, delivery_due, delivery_place, delivery_terms
                ) VALUES (
                    @Base, @Seq, @LineNo, @DemandAgency, @ItemName, @DetailItemNumber,
                    @ItemIdNumber, @Specification, @Quantity, @Unit, @UnitPrice,
                    @DeliveryDays, @DeliveryDue, @DeliveryPlace, @DeliveryTerms
                );
                """,
                new
                {
                    Base = notice.NoticeBase, notice.Seq, item.LineNo, item.DemandAgency,
                    item.ItemName, item.DetailItemNumber, item.ItemIdNumber, item.Specification,
                    item.Quantity, item.Unit, item.UnitPrice, item.DeliveryDays,
                    item.DeliveryDue, item.DeliveryPlace, item.DeliveryTerms,
                }, transaction);

        ReplaceChildren(connection, transaction,
            "DELETE FROM notice_schedule WHERE notice_base = @b AND seq = @s;",
            new { b = notice.NoticeBase, s = notice.Seq });

        foreach (var step in notice.Schedule)
            connection.Execute(
                """
                INSERT INTO notice_schedule (notice_base, seq, line_no, name, method, starts_at, ends_at, place)
                VALUES (@Base, @Seq, @LineNo, @Name, @Method, @StartsAt, @EndsAt, @Place);
                """,
                new
                {
                    Base = notice.NoticeBase, notice.Seq, step.LineNo, step.Name,
                    step.Method, step.StartsAt, step.EndsAt, step.Place,
                }, transaction);

        ReplaceChildren(connection, transaction,
            "DELETE FROM notice_officer_contact WHERE notice_base = @b AND seq = @s;",
            new { b = notice.NoticeBase, s = notice.Seq });

        foreach (var contact in notice.Contacts)
            connection.Execute(
                """
                INSERT INTO notice_officer_contact (
                    notice_base, seq, line_no, demand_agency, department, officer, fax, phone
                ) VALUES (@Base, @Seq, @LineNo, @DemandAgency, @Department, @Officer, @Fax, @Phone);
                """,
                new
                {
                    Base = notice.NoticeBase, notice.Seq, contact.LineNo, contact.DemandAgency,
                    contact.Department, contact.Officer, contact.Fax, contact.Phone,
                }, transaction);

        // 관련공고도 차수에 매단다. 변경공고마다 가리키는 것이 달라서다 —
        // 당초는 자기를 대신한 것을, 변경분은 자기가 대신한 것을 적는다.
        ReplaceChildren(connection, transaction,
            "DELETE FROM notice_relation WHERE notice_base = @b AND seq = @s;",
            new { b = notice.NoticeBase, s = notice.Seq });

        foreach (var (related, line) in notice.RelatedNotices.Select((r, i) => (r, i + 1)))
        {
            // 적힌 그대로(related)와 해소한 값을 나란히 담는다. 가르는 것은 C# 한 군데다 —
            // 뷰가 이 두 열만 견주므로 SQL 에는 문자열 산술이 없고, 규칙이 두 벌이 되지 않는다.
            var 갈린것 = ValueParser.SplitNoticeRef(related);

            connection.Execute(
                """
                INSERT INTO notice_relation (notice_base, seq, line_no, related, related_base, related_seq)
                VALUES (@Base, @Seq, @LineNo, @Related, @RelatedBase, @RelatedSeq);
                """,
                new
                {
                    Base = notice.NoticeBase, notice.Seq, LineNo = line, Related = related,
                    RelatedBase = 갈린것?.Base, RelatedSeq = 갈린것?.Seq,
                },
                transaction);
        }

        // 관련공고가 바뀌면 건이 바뀐다. 공고 한 장이 이미 선 건 둘을 합칠 수 있어
        // 증분으로는 셀 수 없다 — 커밋 전에 같은 트랜잭션에서 전부 다시 짓는다.
        NoticeGroups.Rebuild(connection, transaction);

        transaction.Commit();
    }

    internal void UpsertContract(ContractRecord contract)
    {
        var now = DateTime.UtcNow.ToString("O");

        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();

        connection.Execute(
            "INSERT OR IGNORE INTO contract_series (contract_base) VALUES (@b);",
            new { b = contract.ContractBase }, transaction);

        var party = contract.Counterparty;
        if (!string.IsNullOrWhiteSpace(party?.BusinessNumber))
            connection.Execute(
                """
                INSERT INTO counterparty (business_number, name, representative, address, phone, fax, updated_at)
                VALUES (@BusinessNumber, @Name, @Representative, @Address, @Phone, @Fax, @Now)
                ON CONFLICT(business_number) DO UPDATE SET
                    name = excluded.name, representative = excluded.representative,
                    address = excluded.address, phone = excluded.phone, fax = excluded.fax,
                    updated_at = excluded.updated_at;
                """,
                new
                {
                    party.BusinessNumber, party.Name, party.Representative,
                    party.Address, party.Phone, party.Fax, Now = now,
                }, transaction);

        connection.Execute(
            """
            INSERT INTO contract (
                contract_base, seq, title, contracted_on, contract_method, law_clause,
                goods_or_service, purchase_management_number, agency_management_number,
                contract_kind, contract_character, terminated, terminated_on,
                item_name, quantity, unit, amount, fee, delay_penalty_rate, warranty_bond_rate,
                warranty_period, contract_period, delivery_due, delivery_terms, delivery_place,
                partial_delivery_allowed, payment_method, demand_agency, inspection_agency,
                acceptance_agency, counterparty_number, updated_at
            ) VALUES (
                @ContractBase, @Seq, @Title, @ContractedOn, @ContractMethod, @LawClause,
                @GoodsOrService, @PurchaseManagementNumber, @AgencyManagementNumber,
                @ContractKind, @ContractCharacter, @Terminated, @TerminatedOn,
                @ItemName, @Quantity, @Unit, @Amount, @Fee, @DelayPenaltyRate, @WarrantyBondRate,
                @WarrantyPeriod, @ContractPeriod, @DeliveryDue, @DeliveryTerms, @DeliveryPlace,
                @PartialDeliveryAllowed, @PaymentMethod, @DemandAgency, @InspectionAgency,
                @AcceptanceAgency, @CounterpartyNumber, @Now
            )
            ON CONFLICT(contract_base, seq) DO UPDATE SET
                title = excluded.title, contracted_on = excluded.contracted_on,
                contract_method = excluded.contract_method, law_clause = excluded.law_clause,
                goods_or_service = excluded.goods_or_service,
                purchase_management_number = excluded.purchase_management_number,
                agency_management_number = excluded.agency_management_number,
                contract_kind = excluded.contract_kind,
                contract_character = excluded.contract_character,
                terminated = excluded.terminated, terminated_on = excluded.terminated_on,
                item_name = excluded.item_name, quantity = excluded.quantity, unit = excluded.unit,
                amount = excluded.amount, fee = excluded.fee,
                delay_penalty_rate = excluded.delay_penalty_rate,
                warranty_bond_rate = excluded.warranty_bond_rate,
                warranty_period = excluded.warranty_period,
                contract_period = excluded.contract_period, delivery_due = excluded.delivery_due,
                delivery_terms = excluded.delivery_terms, delivery_place = excluded.delivery_place,
                partial_delivery_allowed = excluded.partial_delivery_allowed,
                payment_method = excluded.payment_method, demand_agency = excluded.demand_agency,
                inspection_agency = excluded.inspection_agency,
                acceptance_agency = excluded.acceptance_agency,
                counterparty_number = excluded.counterparty_number,
                updated_at = excluded.updated_at;
            """,
            new
            {
                contract.ContractBase, contract.Seq, contract.Title, contract.ContractedOn,
                contract.ContractMethod, contract.LawClause, contract.GoodsOrService,
                contract.PurchaseManagementNumber, contract.AgencyManagementNumber,
                contract.ContractKind, contract.ContractCharacter, contract.Terminated,
                contract.TerminatedOn, contract.ItemName, contract.Quantity, contract.Unit,
                contract.Amount, contract.Fee, contract.DelayPenaltyRate, contract.WarrantyBondRate,
                contract.WarrantyPeriod, contract.ContractPeriod, contract.DeliveryDue,
                contract.DeliveryTerms, contract.DeliveryPlace, contract.PartialDeliveryAllowed,
                contract.PaymentMethod, contract.DemandAgency, contract.InspectionAgency,
                contract.AcceptanceAgency,
                CounterpartyNumber = party?.BusinessNumber,
                Now = now,
            }, transaction);

        ReplaceChildren(connection, transaction,
            "DELETE FROM contract_item WHERE contract_base = @b AND seq = @s;",
            new { b = contract.ContractBase, s = contract.Seq });

        foreach (var item in contract.Items)
            connection.Execute(
                """
                INSERT INTO contract_item (
                    contract_base, seq, line_no, classification_number, item_id_number, item_name,
                    specification, region, unit, delivery_terms, quantity, unit_price, amount,
                    demand_agency, delivery_due
                ) VALUES (
                    @Base, @Seq, @LineNo, @ClassificationNumber, @ItemIdNumber, @ItemName,
                    @Specification, @Region, @Unit, @DeliveryTerms, @Quantity, @UnitPrice, @Amount,
                    @DemandAgency, @DeliveryDue
                );
                """,
                new
                {
                    Base = contract.ContractBase, contract.Seq, item.LineNo,
                    item.ClassificationNumber, item.ItemIdNumber, item.ItemName, item.Specification,
                    item.Region, item.Unit, item.DeliveryTerms, item.Quantity, item.UnitPrice,
                    item.Amount, item.DemandAgency, item.DeliveryDue,
                }, transaction);

        ReplaceChildren(connection, transaction,
            "DELETE FROM contract_attachment WHERE contract_base = @b AND seq = @s;",
            new { b = contract.ContractBase, s = contract.Seq });

        foreach (var attachment in contract.Attachments)
            connection.Execute(
                """
                INSERT INTO contract_attachment (contract_base, seq, line_no, document_type, file_name)
                VALUES (@Base, @Seq, @LineNo, @DocumentType, @FileName);
                """,
                new
                {
                    Base = contract.ContractBase, contract.Seq,
                    attachment.LineNo, attachment.DocumentType, attachment.FileName,
                }, transaction);

        transaction.Commit();
    }

    internal void UpsertRequest(RequestRecord request)
    {
        var now = DateTime.UtcNow.ToString("O");

        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();

        // 계열 먼저. 사람이 붙인 값과 링크가 이 표를 가리키므로 차수 행보다 앞서 있어야 한다.
        connection.Execute(
            "INSERT OR IGNORE INTO request_series (request_base) VALUES (@b);",
            new { b = request.RequestBase }, transaction);

        connection.Execute(
            """
            INSERT INTO request (
                request_base, seq, title, received_on, business_kind, contract_law,
                contract_method, contract_kind, award_method, accounting_kind, payment_method,
                goods_amount, fee, vat, budget_amount, foreign_allowed, request_kind, disclosure,
                executive_officer, department, officer, advance_notice, advance_payment, remarks,
                demand_agency, demand_agency_code, agency_officer, agency_phone, agency_fax,
                updated_at
            ) VALUES (
                @RequestBase, @Seq, @Title, @ReceivedOn, @BusinessKind, @ContractLaw,
                @ContractMethod, @ContractKind, @AwardMethod, @AccountingKind, @PaymentMethod,
                @GoodsAmount, @Fee, @Vat, @BudgetAmount, @ForeignAllowed, @RequestKind, @Disclosure,
                @ExecutiveOfficer, @Department, @Officer, @AdvanceNotice, @AdvancePayment, @Remarks,
                @DemandAgency, @DemandAgencyCode, @AgencyOfficer, @AgencyPhone, @AgencyFax,
                @Now
            )
            ON CONFLICT(request_base, seq) DO UPDATE SET
                title = excluded.title, received_on = excluded.received_on,
                business_kind = excluded.business_kind, contract_law = excluded.contract_law,
                contract_method = excluded.contract_method, contract_kind = excluded.contract_kind,
                award_method = excluded.award_method, accounting_kind = excluded.accounting_kind,
                payment_method = excluded.payment_method, goods_amount = excluded.goods_amount,
                fee = excluded.fee, vat = excluded.vat, budget_amount = excluded.budget_amount,
                foreign_allowed = excluded.foreign_allowed, request_kind = excluded.request_kind,
                disclosure = excluded.disclosure, executive_officer = excluded.executive_officer,
                department = excluded.department, officer = excluded.officer,
                advance_notice = excluded.advance_notice, advance_payment = excluded.advance_payment,
                remarks = excluded.remarks, demand_agency = excluded.demand_agency,
                demand_agency_code = excluded.demand_agency_code,
                agency_officer = excluded.agency_officer, agency_phone = excluded.agency_phone,
                agency_fax = excluded.agency_fax,
                updated_at = excluded.updated_at;
            """,
            new
            {
                request.RequestBase, request.Seq, request.Title, request.ReceivedOn,
                request.BusinessKind, request.ContractLaw, request.ContractMethod,
                request.ContractKind, request.AwardMethod, request.AccountingKind,
                request.PaymentMethod, request.GoodsAmount, request.Fee, request.Vat,
                request.BudgetAmount, request.ForeignAllowed, request.RequestKind,
                request.Disclosure, request.ExecutiveOfficer, request.Department, request.Officer,
                request.AdvanceNotice, request.AdvancePayment, request.Remarks,
                request.DemandAgency, request.DemandAgencyCode, request.AgencyOfficer,
                request.AgencyPhone, request.AgencyFax, Now = now,
            }, transaction);

        // 딸린 줄은 통째로 갈아 끼운다. 줄이 줄어든 변경접수에서 옛 줄이 남지 않게.
        ReplaceChildren(connection, transaction,
            "DELETE FROM request_item WHERE request_base = @b AND seq = @s;",
            new { b = request.RequestBase, s = request.Seq });

        foreach (var item in request.Items)
            connection.Execute(
                """
                INSERT INTO request_item (
                    request_base, seq, line_no, request_number, plan_year, change_seq,
                    detail_item_number, item_id_number, item_name, cancelled, specification,
                    inspection_kind, unit_price, quantity, amount, unit, pages,
                    delivery_terms, delivery_days, delivery_due, delivery_place,
                    stock_number, spec_number, expense_request_number
                ) VALUES (
                    @Base, @Seq, @LineNo, @RequestNumber, @PlanYear, @ChangeSeq,
                    @DetailItemNumber, @ItemIdNumber, @ItemName, @Cancelled, @Specification,
                    @InspectionKind, @UnitPrice, @Quantity, @Amount, @Unit, @Pages,
                    @DeliveryTerms, @DeliveryDays, @DeliveryDue, @DeliveryPlace,
                    @StockNumber, @SpecNumber, @ExpenseRequestNumber
                );
                """,
                new
                {
                    Base = request.RequestBase, request.Seq, item.LineNo, item.RequestNumber,
                    item.PlanYear, item.ChangeSeq, item.DetailItemNumber, item.ItemIdNumber,
                    item.ItemName, item.Cancelled, item.Specification, item.InspectionKind,
                    item.UnitPrice, item.Quantity, item.Amount, item.Unit, item.Pages,
                    item.DeliveryTerms, item.DeliveryDays, item.DeliveryDue, item.DeliveryPlace,
                    item.StockNumber, item.SpecNumber, item.ExpenseRequestNumber,
                }, transaction);

        transaction.Commit();
    }

    /// <summary>
    /// 사람이 채우는 열의 정의를 읽는다. 편집 화면이 무엇을 띄울지 여기서 정해진다.
    /// </summary>
    public IReadOnlyList<UserColumnDefinition> UserColumns(string? entityType = null)
    {
        using var connection = _database.Open();
        return [.. connection.Query<UserColumnDefinition>(
            """
            SELECT entity_type AS EntityType, field_name AS FieldName, kind AS Kind,
                   options AS Options, sort_order AS SortOrder
            FROM user_column
            WHERE (@entityType IS NULL OR entity_type = @entityType)
            ORDER BY entity_type, sort_order;
            """, new { entityType })];
    }

    /// <summary>
    /// 사람이 채우는 열을 새로 만든다. 만들고 나면 뷰를 다시 지어 표에 곧바로 선다.
    /// </summary>
    public void AddUserColumn(
        string entityType, string fieldName, string kind, IReadOnlyList<string> choices)
    {
        var name = CleanName(fieldName);
        CheckKind(kind);

        using (var connection = _database.Open())
        {
            CheckFree(connection, entityType, name);

            connection.Execute(
                """
                INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
                VALUES (@entityType, @name, @kind, @options,
                        COALESCE((SELECT MAX(sort_order) FROM user_column
                                  WHERE entity_type = @entityType), 0) + 10);
                """,
                new { entityType, name, kind, options = Pack(kind, choices) });
        }

        _database.RefreshViews();
    }

    /// <summary>
    /// 열 하나를 고친다. 이름을 바꾸면 <b>적어 둔 값도 함께 옮긴다</b> —
    /// 값은 열 이름으로 매달려 있어서, 정의만 고치면 적어 둔 것이 통째로 미아가 된다.
    /// </summary>
    public void UpdateUserColumn(
        string entityType, string fieldName, string newName,
        string kind, IReadOnlyList<string> choices)
    {
        var name = CleanName(newName);
        CheckKind(kind);

        using (var connection = _database.Open())
        {
            CheckExists(connection, entityType, fieldName);
            CheckFree(connection, entityType, name, allow: fieldName);

            using var transaction = connection.BeginTransaction();

            connection.Execute(
                """
                UPDATE user_column SET field_name = @name, kind = @kind, options = @options
                WHERE entity_type = @entityType AND field_name = @fieldName;
                """,
                new { entityType, fieldName, name, kind, options = Pack(kind, choices) },
                transaction);

            if (name != fieldName)
            {
                var (table, _) = TableFor(entityType);
                connection.Execute(
                    $"UPDATE {table} SET field_name = @name WHERE field_name = @fieldName;",
                    new { name, fieldName }, transaction);
            }

            transaction.Commit();
        }

        _database.RefreshViews();
    }

    /// <summary>
    /// 열을 없앤다.
    ///
    /// <para><b>적어 둔 값은 남긴다.</b> 열 하나에 백 건의 메모가 달려 있을 수 있는데, 단추 한 번에
    /// 그것이 사라지면 되돌릴 길이 없다. 정의만 걷으면 표에서 사라지고, 같은 이름으로 다시 만들면
    /// 그대로 되살아난다 — 사람이 적은 것을 기계가 지우지 않는다는 원칙 그대로다(ADR-007).</para>
    /// </summary>
    public void RemoveUserColumn(string entityType, string fieldName)
    {
        using (var connection = _database.Open())
        {
            CheckExists(connection, entityType, fieldName);

            connection.Execute(
                "DELETE FROM user_column WHERE entity_type = @entityType AND field_name = @fieldName;",
                new { entityType, fieldName });
        }

        _database.RefreshViews();
    }

    /// <summary>
    /// 열의 차례를 한 칸 옮긴다. <paramref name="delta"/> 가 음수면 앞으로.
    ///
    /// <para>옮긴 뒤 <b>번호를 다시 매긴다</b>. 같은 <c>sort_order</c> 를 가진 열이 있으면
    /// 맞바꾸기만으로는 차례가 정해지지 않아, 눌러도 아무 일이 없는 것처럼 보인다.</para>
    /// </summary>
    public void MoveUserColumn(string entityType, string fieldName, int delta)
    {
        using (var connection = _database.Open())
        {
            CheckExists(connection, entityType, fieldName);

            var order = connection.Query<string>(
                """
                SELECT field_name FROM user_column
                WHERE entity_type = @entityType ORDER BY sort_order, field_name;
                """, new { entityType }).ToList();

            var at = order.IndexOf(fieldName);
            var to = Math.Clamp(at + delta, 0, order.Count - 1);
            if (at == to) return;

            order.RemoveAt(at);
            order.Insert(to, fieldName);

            using var transaction = connection.BeginTransaction();

            for (var i = 0; i < order.Count; i++)
                connection.Execute(
                    """
                    UPDATE user_column SET sort_order = @sort
                    WHERE entity_type = @entityType AND field_name = @name;
                    """,
                    new { entityType, name = order[i], sort = (i + 1) * 10 }, transaction);

            transaction.Commit();
        }

        _database.RefreshViews();
    }

    /// <summary>후보는 <c>choice</c> 일 때만 담는다. 다른 종류에 남아 있으면 화면이 헷갈린다.</summary>
    private static string? Pack(string kind, IReadOnlyList<string> choices)
    {
        if (kind != "choice") return null;

        var clean = choices
            .Select(c => c.Trim())
            .Where(c => c.Length > 0 && !c.Contains('|'))
            .Distinct(StringComparer.Ordinal)
            .ToList();

        return clean.Count == 0 ? null : string.Join('|', clean);
    }

    private static void CheckKind(string kind)
    {
        if (kind is not ("text" or "choice" or "date" or "number"))
            throw new InvalidOperationException($"모르는 종류입니다: {kind}");
    }

    /// <summary>
    /// 열 이름을 다듬고 쓸 수 있는지 본다.
    ///
    /// <para><b>이름이 SQL 에 그대로 박힌다.</b> 뷰를 지을 때 문자열 리터럴 안(<c>'이름'</c>)과
    /// 따옴표 친 별칭(<c>AS "이름"</c>) 두 자리에 들어간다. 따옴표가 섞이면 뷰 정의가 깨지는데,
    /// <b>뷰는 앱이 열릴 때마다 다시 지어지므로</b> 한 번 깨진 이름은 그 뒤로 앱이 아예 뜨지
    /// 못하게 만든다. 화면에서 걸러도 다리로 직접 부를 수 있으니 여기서 막는다.</para>
    /// </summary>
    private static string CleanName(string fieldName)
    {
        var name = fieldName.Trim();

        if (name.Length == 0) throw new InvalidOperationException("열 이름이 비었습니다.");
        if (name.Length > 20) throw new InvalidOperationException("열 이름은 20자까지입니다.");

        if (name.Any(ch => ch is '\'' or '"' or '\\' || char.IsControl(ch)))
            throw new InvalidOperationException("열 이름에 따옴표나 역슬래시를 쓸 수 없습니다.");

        return name;
    }

    private static void CheckExists(SqliteConnection connection, string entityType, string fieldName)
    {
        if (connection.ExecuteScalar<long>(
                """
                SELECT COUNT(*) FROM user_column
                WHERE entity_type = @entityType AND field_name = @fieldName;
                """, new { entityType, fieldName }) == 0)
            throw new InvalidOperationException($"그런 열이 없습니다: {fieldName}");
    }

    /// <summary>
    /// 이 이름을 써도 되는가.
    ///
    /// <para>이미 뷰가 내는 이름과 부딪히면 <b>SQLite 가 조용히 뒤엣것을 <c>이름:1</c> 로 바꾼다</b> —
    /// 오류가 나지 않아 표가 이상해진 뒤에야 알게 된다. 그래서 뷰가 실제로 내는 열 이름을 읽어
    /// 견준다. 목록을 코드에 적어 두면 뷰를 고칠 때마다 어긋난다.</para>
    ///
    /// <para><b>통합은 <c>v_통합</c>·<c>v_통합차수</c> 를 함께 본다.</b> 통합에만 있는 이름(접수
    /// 아홉 열, 「공고건」·「현행공고」)을 계약 뷰만 보는 검사가 그대로 지나면, 사람이 세운 열이
    /// 통합에서 조용히 부딪힌다.</para>
    /// </summary>
    private static void CheckFree(
        SqliteConnection connection, string entityType, string name, string? allow = null)
    {
        var mine = connection.Query<string>(
            "SELECT field_name FROM user_column WHERE entity_type = @entityType;",
            new { entityType }).ToList();

        if (name != allow && mine.Contains(name))
            throw new InvalidOperationException($"이미 있는 열입니다: {name}");

        // 사람이 채우는 열을 뺀 나머지가 파서·조인이 내는 이름이다.
        //
        // 계약만 통합을 본다. 통합이 계약 뷰의 열을 사람 열까지 받으므로 계약 쪽 사람 열은
        // 거기까지 흘러가 공고·접수에서 붙여 온 이름과 부딪힐 수 있다. 공고·접수 열은 통합이
        // 이름을 짚어 골라 오므로(<see cref="Views.UnifiedNoticeColumns"/>) 흘러가지 않는다.
        //
        // <b>통합은 v_통합·v_통합차수를 함께 본다.</b> v_통합차수를 빼면 거기 더한
        // 「공고건」·「현행공고」와 조용히 부딪힌다 — v_통합차수도 계약 뷰를 열째로 받으므로
        // 계약 쪽 사람 열이 그대로 흘러간다.
        //
        // 차수 뷰 둘은 보지 않는다. 열이 v_공고·v_계약 과 한 글자도 같아(공고열·계약열
        // 하나를 쓴다) 저쪽을 본 것이 곧 이쪽을 본 것이다.
        string[] views = entityType switch
        {
            "contract" => ["v_통합", "v_통합차수"],
            "request" => ["v_접수"],
            _ => ["v_공고"],
        };

        var taken = views
            .SelectMany(v => ViewColumns(connection, v))
            .Except(mine)
            .ToHashSet(StringComparer.Ordinal);

        if (taken.Contains(name))
            throw new InvalidOperationException($"표가 이미 쓰는 이름입니다: {name}");
    }

    private static IReadOnlyList<string> ViewColumns(SqliteConnection connection, string view)
    {
        using var command = connection.CreateCommand();
        command.CommandText = $"PRAGMA table_info(\"{view}\");";

        var names = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read()) names.Add(reader.GetString(1));

        return names;
    }

    /// <summary>
    /// 사람이 적은 값을 넣는다. <b>자료를 들이는 길은 이 표에 쓰지 않는다</b> — 다시 수집해도
    /// 여기 적힌 것이 지워지지 않는 것은 그 분리 덕분이다(ADR-007).
    ///
    /// <para>값은 <b>차수가 아니라 계열</b>에 붙는다. 진행상태·담당·메모는 "이 계약 건"에 대한
    /// 사람의 판단이지 "3차 계약서"에 대한 판단이 아니다. 차수에 매달아 두면 변경계약이
    /// 한 건 들어오는 순간 뷰에서 조용히 사라진다(ADR-012).</para>
    ///
    /// <para>계약용·공고용 표가 따로다. 하나로 두면 어느 쪽을 가리키는지 DB가 알 수 없어
    /// 외래키를 걸지 못한다.</para>
    /// </summary>
    public void SetUserField(EntityRef entity, string fieldName, string? value)
    {
        var (table, baseColumn) = TableFor(entity.EntityType);

        using var connection = _database.Open();

        if (string.IsNullOrEmpty(value))
        {
            connection.Execute(
                $"DELETE FROM {table} WHERE {baseColumn} = @base AND field_name = @fieldName;",
                new { entity.Base, fieldName });
            return;
        }

        connection.Execute(
            $"""
            INSERT INTO {table} ({baseColumn}, field_name, value, updated_at)
            VALUES (@Base, @fieldName, @value, @now)
            ON CONFLICT({baseColumn}, field_name) DO UPDATE SET
                value = excluded.value, updated_at = excluded.updated_at;
            """,
            new { entity.Base, fieldName, value, now = DateTime.UtcNow.ToString("O") });
    }

    // ── 파서가 읽은 값을 사람이 고치기 ───────────────────────────
    // 사람 값(user_field)과 나란히 서지만 매다는 자리가 다르다 — 저쪽은 "이 건" 에 대한
    // 판단이라 계열에, 이쪽은 "이 문서의 이 칸" 에 대한 정정이라 차수에 붙는다(ADR-012·020).

    /// <summary>
    /// 뷰의 열 하나에 사람이 적은 값을 씌운다.
    ///
    /// <para><see cref="SetUserField"/> 와 달리 <b>빈 문자열에 삭제 의미를 주지 않는다</b> —
    /// 잘못 읽힌 칸을 비우는 것이 실제 용례라, 되돌리기는 <see cref="ClearOverride"/> 가 맡는다.</para>
    ///
    /// <para>고치기 전에 그 자리에 있던 값을 함께 담되 <b>처음 씌울 때만</b> 적는다. 두 번째
    /// 편집에서 다시 적으면 "고치기 전의 값" 이 아니라 "직전에 내가 적은 값" 이 되어,
    /// 화면의 되돌리기 안내가 거짓이 된다.</para>
    /// </summary>
    public void SetOverride(EntityRef entity, string columnName, string value)
    {
        CheckCorrectable(entity, columnName);
        var original = FaceValue(entity, columnName);

        using var connection = _database.Open();

        connection.Execute(
            """
            INSERT INTO field_override
                (entity_type, base, seq, column_name, value, original, updated_at)
            VALUES (@EntityType, @Base, @Seq, @columnName, @value, @original, @now)
            ON CONFLICT(entity_type, base, seq, column_name) DO UPDATE SET
                value = excluded.value, updated_at = excluded.updated_at;
            """,
            new
            {
                entity.EntityType, entity.Base, entity.Seq,
                columnName, value, original, now = DateTime.UtcNow.ToString("O"),
            });
    }

    /// <summary>덮개를 걷는다. 뷰는 다시 파서가 읽은 값을 낸다.</summary>
    public void ClearOverride(EntityRef entity, string columnName)
    {
        CheckCorrectable(entity, columnName);

        using var connection = _database.Open();

        connection.Execute(
            """
            DELETE FROM field_override
            WHERE entity_type = @EntityType AND base = @Base AND seq = @Seq
              AND column_name = @columnName;
            """,
            new { entity.EntityType, entity.Base, entity.Seq, columnName });
    }

    /// <summary>
    /// 이 종류를 통째로 내는 계약면 뷰. <b>덮개를 씌울 수 있는 열은 이 뷰의 열이다.</b>
    /// </summary>
    public static string FaceView(string entityType) => entityType switch
    {
        "notice" => "v_공고",
        "request" => "v_접수",
        _ => "v_계약",
    };

    internal static string FaceKey(string entityType) => entityType switch
    {
        "notice" => "입찰공고번호",
        "request" => "접수번호",
        _ => "계약번호",
    };

    /// <summary>
    /// 덮개를 씌울 수 있는 열인가. 아니면 까닭을 담아 던진다.
    ///
    /// <para>씌우고 읽는 자리에서 <b>열 이름이 SQL 에 그대로 박히므로</b>, 통과한 이름이
    /// 계약면에 실재하는 것이어야 한다 — 화면이 걸러도 다리와 명령줄로 직접 부를 수 있어
    /// 검사를 여기 하나에 모은다.</para>
    ///
    /// <para>막는 것은 셋이다. <b>키 열</b>은 고치면 값이 바뀌는 것이 아니라 레코드가 옮겨간다.
    /// <b>사람이 채우는 열</b>은 담기는 표가 따로 있어 <see cref="SetUserField"/> 의 몫이다.
    /// 계약면에 없는 이름은 애초에 씌울 자리가 없다.</para>
    /// </summary>
    public void CheckCorrectable(EntityRef entity, string columnName)
    {
        if (Views.KeyColumns.Contains(columnName))
            throw new InvalidOperationException($"레코드를 가리키는 열이라 고칠 수 없습니다: {columnName}");

        using var connection = _database.Open();

        if (connection.ExecuteScalar<long>(
                """
                SELECT COUNT(*) FROM user_column
                WHERE entity_type = @EntityType AND field_name = @columnName;
                """,
                new { entity.EntityType, columnName }) > 0)
            throw new InvalidOperationException($"손으로 채우는 열입니다: {columnName}");

        if (!ViewColumns(connection, FaceView(entity.EntityType)).Contains(columnName))
            throw new InvalidOperationException($"계약면에 없는 열입니다: {columnName}");
    }

    /// <summary>
    /// 지금 그 칸에 보이는 값. 이미 덮개가 걸려 있으면 그쪽이 온다.
    /// <b><see cref="CheckCorrectable"/> 를 지난 이름으로만 부른다</b> — 이름이 SQL 에 박힌다.
    /// </summary>
    public string FaceValue(EntityRef entity, string columnName)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();

        command.CommandText =
            $"SELECT \"{columnName}\" FROM \"{FaceView(entity.EntityType)}\" " +
            $"WHERE \"{FaceKey(entity.EntityType)}\" = $key;";
        command.Parameters.AddWithValue("$key", entity.Display);

        var value = command.ExecuteScalar();
        return value is null or DBNull ? "" : value.ToString() ?? "";
    }

    /// <summary>
    /// 지금 걸려 있는 덮개 전부. 화면이 <b>어느 칸이 손으로 고쳐졌는지</b> 표시하는 데 쓴다 —
    /// 보이지 않으면 고칠 수 있다는 것도 안전하지 않다.
    /// </summary>
    public IReadOnlyList<FieldOverride> Overrides(string entityType)
    {
        using var connection = _database.Open();

        return [.. connection.Query<FieldOverride>(
            """
            SELECT base AS Base, seq AS Seq, column_name AS ColumnName, original AS Original
            FROM field_override WHERE entity_type = @entityType;
            """, new { entityType })];
    }

    // ── 지우기 ───────────────────────────────────────────────────
    // 쌓는 길만 있고 걷어내는 길이 없으면, 잘못 읽힌 문서와 남의 공고가 영원히 남는다.

    /// <summary>
    /// 지우면 무엇이 함께 사라지는지 세어 준다. <b>지우기 전에 반드시 이것을 보인다.</b>
    /// </summary>
    public DeletionPlan PlanDeletion(EntityRef entity, bool wholeSeries)
    {
        var (head, baseColumn, _, userFields, children) = Shape(entity.EntityType);

        using var connection = _database.Open();

        var revisions = connection.Query<string>(
            $"SELECT seq FROM {head} WHERE {baseColumn} = @Base ORDER BY CAST(seq AS INTEGER);",
            new { entity.Base }).ToList();

        if (revisions.Count == 0)
            throw new InvalidOperationException($"그런 접수·공고·계약이 없습니다: {entity.Display}");

        // 마지막 차수를 지우면 계열은 남을 자리가 없다 — 사람 값과 링크가 미아가 되기 전에 함께 걷는다.
        var seriesGoes = wholeSeries || revisions.Count <= 1;

        var scope = wholeSeries ? "" : " AND seq = @Seq";

        var childRows = children.Sum(table => (int)connection.ExecuteScalar<long>(
            $"SELECT COUNT(*) FROM {table} WHERE {baseColumn} = @Base{scope};",
            new { entity.Base, entity.Seq }));

        return new DeletionPlan(
            entity.EntityType,
            entity.Display,
            entity.Seq,
            connection.ExecuteScalar<string?>(
                $"SELECT title FROM {head} WHERE {baseColumn} = @Base AND seq = @Seq;",
                new { entity.Base, entity.Seq }) ?? "",
            revisions,
            seriesGoes,
            childRows,
            seriesGoes
                ? (int)connection.ExecuteScalar<long>(
                    $"SELECT COUNT(*) FROM {userFields} WHERE {baseColumn} = @Base;", new { entity.Base })
                : 0,
            (int)connection.ExecuteScalar<long>(
                $"""
                SELECT COUNT(*) FROM field_override
                WHERE entity_type = @EntityType AND base = @Base{scope};
                """, new { entity.EntityType, entity.Base, entity.Seq }),
            seriesGoes && LinkCount(connection, entity, baseColumn) > 0);
    }

    /// <summary>
    /// 이 계열을 지우면 함께 걷힐 링크가 있는가.
    ///
    /// <para><b>공고는 갈래가 다르다.</b> 링크의 끝점이 계열이 아니라 <b>건</b>이라
    /// (스키마 V15) 링크 표에는 <c>notice_base</c> 열이 아예 없다 — 다른 둘과 같은 SQL 을
    /// 조립하면 그 자리에서 터진다. 공고는 <c>notice_series</c> 를 거쳐 제 건을 찾아 센다.</para>
    ///
    /// <para>공고는 <b>계약 링크와 접수 링크를 함께</b> 센다. 접수 링크만 걷히고 마는 일은
    /// 없는데도 예전에는 <c>project_link</c> 만 세어, 접수만 이어져 있으면 「링크 없음」으로
    /// 보이고는 지운 뒤에 사라졌다.</para>
    ///
    /// <para>「건이 갈린다」는 세지 않는다. 그 셈은 <c>NoticeGroups.Rebuild</c> 를 가상으로
    /// 돌려야 나오는데 계산부와 반영부가 갈려 있지 않다 — 없는 수를 흉내 내지 않는다.</para>
    /// </summary>
    private static long LinkCount(SqliteConnection connection, EntityRef entity, string baseColumn) =>
        entity.EntityType == "notice"
            ? connection.ExecuteScalar<long>(
                """
                SELECT
                    (SELECT COUNT(*) FROM project_link l
                     JOIN notice_series s ON s.group_base = l.notice_group
                     WHERE s.notice_base = @Base)
                  + (SELECT COUNT(*) FROM request_link l
                     JOIN notice_series s ON s.group_base = l.notice_group
                     WHERE s.notice_base = @Base);
                """, new { entity.Base })
            : connection.ExecuteScalar<long>(
                $"SELECT COUNT(*) FROM {LinkTable(entity.EntityType)} WHERE {baseColumn} = @Base;",
                new { entity.Base });

    /// <summary>
    /// 이 종류의 링크가 담기는 표. 접수는 <c>request_link</c>, 나머지는 <c>project_link</c> 다 —
    /// <c>project_link</c> 에는 <c>request_base</c> 열이 아예 없어, 가리지 않으면 SQL 이 터진다.
    /// <b>공고는 여기 오지 않는다</b> — <see cref="LinkCount"/> 가 건을 거쳐 따로 센다.
    /// </summary>
    private static string LinkTable(string entityType) =>
        entityType == "request" ? "request_link" : "project_link";

    /// <summary>
    /// 레코드를 지운다. <paramref name="wholeSeries"/> 면 이 본번호의 <b>모든 차수</b>를.
    ///
    /// <para>딸린 줄은 외래키 캐스케이드가 데려간다. 손으로 지워야 하는 것은 <b>덮개</b>다 —
    /// 접수·공고·계약을 한 표에 담아 외래키를 걸지 못했다.</para>
    ///
    /// <para><b>묘비는 두지 않는다.</b> 지운 뒤에 같은 자료를 다시 수집하면 다시 선다.</para>
    /// </summary>
    public void Delete(EntityRef entity, bool wholeSeries)
    {
        var plan = PlanDeletion(entity, wholeSeries);
        var (head, baseColumn, series, _, _) = Shape(entity.EntityType);

        var scope = wholeSeries ? "" : " AND seq = @Seq";

        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();

        connection.Execute(
            $"""
            DELETE FROM field_override
            WHERE entity_type = @EntityType AND base = @Base{scope};
            """,
            new { entity.EntityType, entity.Base, entity.Seq }, transaction);

        connection.Execute(
            $"DELETE FROM {head} WHERE {baseColumn} = @Base{scope};",
            new { entity.Base, entity.Seq }, transaction);

        // 계열이 걷혀야 사람 값·링크·거부가 캐스케이드로 따라간다.
        if (plan.SeriesGoes)
            connection.Execute(
                $"DELETE FROM {series} WHERE {baseColumn} = @Base;", new { entity.Base }, transaction);

        // 아무 계약도 가리키지 않게 된 상대자. 남겨 두면 status 의 상대자 수가 실물과 어긋난다.
        if (entity.EntityType == "contract")
            connection.Execute(
                """
                DELETE FROM counterparty
                WHERE business_number NOT IN
                    (SELECT counterparty_number FROM contract WHERE counterparty_number IS NOT NULL);
                """, transaction: transaction);

        // 공고가 걷히면 건이 갈리거나 통째로 사라진다. 링크는 건에 매달려 있어 계열을 지운
        // 것만으로는 따라가지 않으므로, 걷어낼 것은 여기서 걷는다 — 캐스케이드에 기대지 않는다.
        NoticeGroups.Rebuild(connection, transaction);

        transaction.Commit();
    }

    /// <summary>종류마다 다른 표 이름을 한 자리에 모은다. 흩어 두면 한 군데만 어긋난다.</summary>
    private static (string Head, string BaseColumn, string Series, string UserFields, string[] Children)
        Shape(string entityType) => entityType switch
        {
            "contract" => ("contract", "contract_base", "contract_series", "contract_user_field",
                           ["contract_item", "contract_attachment"]),
            "notice" => ("notice", "notice_base", "notice_series", "notice_user_field",
                         ["notice_item", "notice_schedule", "notice_officer_contact",
                          "notice_relation"]),
            "request" => ("request", "request_base", "request_series", "request_user_field",
                          ["request_item"]),
            _ => throw new ArgumentOutOfRangeException(nameof(entityType), entityType, "모르는 종류입니다."),
        };

    private static (string Table, string BaseColumn) TableFor(string entityType) => entityType switch
    {
        "contract" => ("contract_user_field", "contract_base"),
        "notice" => ("notice_user_field", "notice_base"),
        "request" => ("request_user_field", "request_base"),
        _ => throw new ArgumentOutOfRangeException(nameof(entityType), entityType, "모르는 종류입니다."),
    };

    /// <summary>
    /// 사람이 쓰는 키 하나를 실제로 있는 행에 맞춰 <b>본번호와 차수로 쪼갠다</b>.
    ///
    /// <para>쪼개는 규칙이 종류마다 다르다(계약은 끝 두 자리, 공고·접수는 붙임표 뒤). 규칙이
    /// 여기저기 흩어져 있으면 한 군데만 어긋나도 조용히 안 맞으므로, 쪼개는 자리를 여기 하나로
    /// 모은다.</para>
    ///
    /// <para><b>접수는 길이 둘이다.</b> 접수번호(<c>R26DC00000001-000</c>)로도 찾고, 그 접수가
    /// 묶은 <b>아무 조달요구번호</b>(<c>MPKPLA26910290</c>)로도 찾는다. 수요기관이 손에 쥔 것은
    /// 대개 자기 조달요구번호이지 조달청이 매긴 접수번호가 아니다.</para>
    ///
    /// <para>접수번호는 <b>있는 행과 글자 그대로</b> 맞춘다. 표기가 공고번호 모양과 같다고
    /// 가정하지 않는다 — 접수번호 꼴을 우리가 정한 적이 없다.</para>
    /// </summary>
    public EntityRef? Resolve(string key)
    {
        using var connection = _database.Open();

        if (ValueParser.ContractNumber(key) is { } contract
            && connection.ExecuteScalar<long>(
                "SELECT COUNT(*) FROM contract WHERE contract_base = @b AND seq = @s;",
                new { b = contract.Base, s = contract.Seq }) > 0)
            return new EntityRef("contract", contract.Base, contract.Seq);

        if (ValueParser.NoticeNumber(key) is { } notice
            && connection.ExecuteScalar<long>(
                "SELECT COUNT(*) FROM notice WHERE notice_base = @b AND seq = @s;",
                new { b = notice.Base, s = notice.Seq }) > 0)
            return new EntityRef("notice", notice.Base, notice.Seq);

        // 접수번호. 있는 행과 글자 그대로 맞춘다.
        var request = connection.QuerySingleOrDefault<(string Base, string Seq)>(
            "SELECT request_base, seq FROM request WHERE request_base || '-' || seq = @key;",
            new { key = key.Trim() });

        if (request.Base is not null) return new EntityRef("request", request.Base, request.Seq);

        // 아무 조달요구번호. 그 계열의 최신 차수를 준다 — 사람이 가리킨 것은 조달요구이지
        // 특정 차수의 접수가 아니다.
        return RequestsCarrying(connection, [key.Trim()]).FirstOrDefault();
    }

    /// <summary>
    /// 이 조달요구번호 중 하나라도 <b>최신 차수의 품목에</b> 싣고 있는 접수들. 계열마다 하나,
    /// 최신 차수로 낸다 — <see cref="Resolve"/> 의 조달요구번호 길이다.
    /// </summary>
    private static IReadOnlyList<EntityRef> RequestsCarrying(
        SqliteConnection connection, IReadOnlyCollection<string> requestNumbers) =>
        [.. connection.Query<(string Base, string Seq)>(
            """
            SELECT DISTINCT i.request_base, i.seq FROM request_item i
            JOIN (SELECT request_base AS b, MAX(CAST(seq AS INTEGER)) AS s
                  FROM request GROUP BY request_base) latest
              ON i.request_base = latest.b AND CAST(i.seq AS INTEGER) = latest.s
            WHERE i.request_number IN @numbers
            ORDER BY i.request_base;
            """, new { numbers = requestNumbers })
            .Select(r => new EntityRef("request", r.Base, r.Seq))];

    private static void ReplaceChildren(
        SqliteConnection connection, SqliteTransaction transaction, string sql, object parameters) =>
        connection.Execute(sql, parameters, transaction);
}
