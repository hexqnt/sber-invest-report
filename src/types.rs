//! Доменные типы и структуры, соответствующие разделам отчёта.

use chrono::NaiveDate;
use rust_decimal::Decimal;

/// Денежное значение, используем `Decimal` для точных расчётов.
pub type Money = Decimal;

/// Идентификатор брокерского счёта в отчёте.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId(pub String);

/// Тип счёта, встречающийся в отчётах.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountKind {
    /// Обычный брокерский счёт.
    Broker,
    /// Индивидуальный инвестиционный счёт.
    Iis,
}

/// Метаданные отчёта: шапка, период и владелец.
#[derive(Debug, Clone)]
pub struct ReportMetadata {
    /// Идентификатор счёта.
    pub account_id: AccountId,
    /// Тип счёта.
    pub account_kind: AccountKind,
    /// Начало периода отчёта.
    pub period_start: NaiveDate,
    /// Конец периода отчёта.
    pub period_end: NaiveDate,
    /// Дата формирования отчёта.
    pub generated_at: NaiveDate,
    /// Имя инвестора.
    pub investor_name: String,
    /// Номер договора.
    pub contract_number: String,
}

/// Строка таблицы «Оценка активов, руб.».
#[derive(Debug, Clone)]
pub struct AssetValuationRow {
    /// Торговая площадка.
    pub venue: String,
    /// Стоимость ЦБ на начало периода.
    pub start_securities: Money,
    /// Денежные средства на начало периода.
    pub start_cash: Money,
    /// Всего на начало периода.
    pub start_total: Money,
    /// Стоимость ЦБ на конец периода.
    pub end_securities: Money,
    /// Денежные средства на конец периода.
    pub end_cash: Money,
    /// Всего на конец периода.
    pub end_total: Money,
    /// Изменение ЦБ.
    pub delta_securities: Money,
    /// Изменение денежных средств.
    pub delta_cash: Money,
    /// Общее изменение.
    pub delta_total: Money,
}

/// Итоги по таблице «Оценка активов, руб.».
#[derive(Debug, Clone)]
pub struct AssetValuation {
    /// Строки таблицы.
    pub rows: Vec<AssetValuationRow>,
    /// Итоговое изменение.
    pub total_delta: Money,
}

/// Тип строки в сводной таблице движения денежных средств.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CashFlowKind {
    /// Входящий остаток.
    OpeningBalance,
    /// Сальдо расчётов по сделкам.
    TradesNet,
    /// Корпоративные действия.
    CorporateActions,
    /// Комиссия брокера.
    BrokerFee,
    /// Комиссия биржи.
    ExchangeFee,
    /// Исходящий остаток.
    ClosingBalance,
    /// Неизвестный тип строки.
    Unknown,
}

/// Строка сводной таблицы движения денежных средств.
#[derive(Debug, Clone)]
pub struct CashFlowRow {
    /// Классификация строки.
    pub kind: CashFlowKind,
    /// Исходное описание из отчёта.
    pub description_raw: String,
    /// Сумма.
    pub amount: Money,
    /// Валюта.
    pub currency: String,
}

/// Сводка движения денежных средств.
#[derive(Debug, Clone)]
pub struct CashFlowSummary {
    /// Строки сводки.
    pub rows: Vec<CashFlowRow>,
}

/// Позиция ценной бумаги на начало и конец периода.
#[derive(Debug, Clone)]
pub struct SecurityPosition {
    /// Наименование бумаги.
    pub name: String,
    /// ISIN.
    pub isin: String,
    /// Валюта цены.
    pub price_currency: String,

    /// Количество на начало.
    pub qty_start: Money,
    /// Номинал на начало.
    pub nominal_start: Money,
    /// Цена на начало.
    pub price_start: Money,
    /// Стоимость без НКД на начало.
    pub value_start_no_ai: Money,
    /// НКД на начало.
    pub accrued_interest_start: Money,

    /// Количество на конец.
    pub qty_end: Money,
    /// Номинал на конец.
    pub nominal_end: Money,
    /// Цена на конец.
    pub price_end: Money,
    /// Стоимость без НКД на конец.
    pub value_end_no_ai: Money,
    /// НКД на конец.
    pub accrued_interest_end: Money,

    /// Изменение количества.
    pub qty_delta: Money,
    /// Изменение стоимости.
    pub value_delta: Money,

    /// Плановые зачисления по сделкам.
    pub planned_in_qty: Money,
    /// Плановые списания.
    pub planned_out_qty: Money,
    /// Плановый исходящий остаток.
    pub planned_end_qty: Money,
}

/// Набор позиций по конкретной торговой площадке.
#[derive(Debug, Clone)]
pub struct PortfolioMarket {
    /// Название площадки.
    pub name: String,
    /// Позиции на площадке.
    pub positions: Vec<SecurityPosition>,
}

/// Портфель ценных бумаг отчёта.
#[derive(Debug, Clone)]
pub struct Portfolio {
    /// Площадки с позициями.
    pub markets: Vec<PortfolioMarket>,
}

/// Строка таблицы пополнений ИИС.
#[derive(Debug, Clone)]
pub struct IisContribution {
    /// Год.
    pub year: i32,
    /// Лимит на год.
    pub limit_rub: Money,
    /// Дата операции.
    pub date: NaiveDate,
    /// Сумма.
    pub amount: Money,
    /// Основание операции.
    pub operation_reason: String,
    /// Остаток лимита.
    pub remaining_limit: Money,
}

/// Таблица пополнений ИИС.
#[derive(Debug, Clone)]
pub struct IisContributionsTable {
    /// Операции пополнения ИИС.
    pub rows: Vec<IisContribution>,
}

/// Итоговая позиция после агрегации нескольких отчётов.
#[derive(Debug, Clone)]
pub struct MergedPosition {
    /// ISIN.
    pub isin: String,
    /// Имя бумаги.
    pub name: String,
    /// Валюта.
    pub price_currency: String,
    /// Суммарное количество на начало.
    pub qty_start: Money,
    /// Суммарное количество на конец.
    pub qty_end: Money,
    /// Стоимость на начало.
    pub value_start_no_ai: Money,
    /// Стоимость на конец.
    pub value_end_no_ai: Money,
    /// Изменение количества.
    pub qty_delta: Money,
    /// Изменение стоимости.
    pub value_delta: Money,
}
