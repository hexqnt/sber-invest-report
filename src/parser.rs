//! Парсинг конкретных таблиц отчёта из DOM.

use std::sync::LazyLock;

use crate::diagnostics::ParseWarning;
use crate::error::ReportError;
use crate::parse_config::ParseMode;
use crate::raw::DomReport;
use crate::types::{
    AccountId, AccountKind, AssetValuation, AssetValuationRow, CashFlowKind, CashFlowRow,
    CashFlowSummary, IisContribution, IisContributionsTable, IisLimit, Portfolio, PortfolioMarket,
    ReportMetadata, SecurityPosition,
};
use crate::utils::{
    capitalize_words, capture_text, collect_text, find_table_with_headers, parse_date,
    parse_money_or_zero,
};
use regex::Regex;
use rust_decimal::Decimal;
use scraper::{ElementRef, Selector};

// Жёстко под шапку отчёта: три даты в одном заголовке.
static PERIOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"за период с\s+(\d{2}\.\d{2}\.\d{4})\s+по\s+(\d{2}\.\d{2}\.\d{4}),\s*дата создания\s+(\d{2}\.\d{2}\.\d{4})",
    )
    .expect("valid period regex")
});

static INVESTOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Инвестор:\s*([^\n<]+)").expect("valid investor regex"));

static CONTRACT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Договор[^A-Za-z0-9]*([A-Za-z0-9]+)").expect("valid contract regex")
});

static RATING_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("table.RatingAssets").expect("valid rating selector"));
static TR_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("tr").expect("valid tr selector"));
static TD_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("td").expect("valid td selector"));
static H3_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h3").expect("valid h3 selector"));
static P_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("p").expect("valid p selector"));

const TABLE_ASSET_VALUATION: &str = "RatingAssets";
const TABLE_CASH_FLOW: &str = "CashFlowSummary";
const TABLE_PORTFOLIO: &str = "Portfolio";
const TABLE_IIS: &str = "IISContributions";

const CASH_FLOW_RULES: [(&str, CashFlowKind); 6] = [
    ("входящий остаток", CashFlowKind::OpeningBalance),
    ("сальдо расчетов по сделкам", CashFlowKind::TradesNet),
    ("корпоративные действия", CashFlowKind::CorporateActions),
    ("комиссия брокера", CashFlowKind::BrokerFee),
    ("комиссия биржи", CashFlowKind::ExchangeFee),
    ("исходящий остаток", CashFlowKind::ClosingBalance),
];

impl DomReport {
    /// Извлекает метаданные из шапки отчёта.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если отсутствуют обязательные поля шапки или не удалось распарсить даты.
    pub fn meta(&self) -> Result<ReportMetadata, ReportError> {
        let heading_text = self
            .doc
            .select(&H3_SELECTOR)
            .next()
            .map(collect_text)
            .ok_or(ReportError::MissingField { field: "heading" })?;

        let period_caps = PERIOD_RE
            .captures(&heading_text)
            .ok_or_else(|| ReportError::Regex(heading_text.clone()))?;

        let period_start = parse_capture_date(&period_caps, 1, &heading_text)?;
        let period_end = parse_capture_date(&period_caps, 2, &heading_text)?;
        let generated_at = parse_capture_date(&period_caps, 3, &heading_text)?;

        // Ищем блок "Инвестор" без стабильного селектора.
        let investor_text = self
            .doc
            .select(&P_SELECTOR)
            .find_map(|p| {
                let text: String = p.text().collect();
                text.to_lowercase().contains("инвестор").then_some(text)
            })
            .ok_or(ReportError::MissingField { field: "investor" })?;

        let investor_name = capture_text(&investor_text, &INVESTOR_RE)
            .map(str::trim)
            .map(capitalize_words)
            .ok_or_else(|| ReportError::Regex(investor_text.clone()))?;

        let contract_number = capture_text(&investor_text, &CONTRACT_RE)
            .map(str::trim)
            .map(str::to_owned)
            .ok_or_else(|| ReportError::Regex(investor_text.clone()))?;

        // Эвристика: тип счёта определяем по тексту.
        let investor_lower = investor_text.to_lowercase();
        let account_kind = if investor_lower.contains("индивидуального инвестиционного счета")
            || investor_lower.contains("иис")
        {
            AccountKind::Iis
        } else {
            AccountKind::Broker
        };

        Ok(ReportMetadata {
            account_id: AccountId(contract_number.clone()),
            account_kind,
            period_start,
            period_end,
            generated_at,
            investor_name,
            contract_number,
        })
    }

    /// Парсит таблицу «Оценка активов, руб.».
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если таблица отсутствует или в числовых полях встретились невалидные данные.
    pub fn parse_asset_valuation(&self) -> Result<AssetValuation, ReportError> {
        let mut ignored_warnings = Vec::new();
        self.parse_asset_valuation_with_mode(ParseMode::Lenient, &mut ignored_warnings)
    }

    pub(crate) fn parse_asset_valuation_with_mode(
        &self,
        mode: ParseMode,
        warnings: &mut Vec<ParseWarning>,
    ) -> Result<AssetValuation, ReportError> {
        let table = self
            .doc
            .select(&RATING_SELECTOR)
            .next()
            .ok_or(ReportError::TableNotFound {
                table: TABLE_ASSET_VALUATION,
            })?;

        let mut rows = Vec::new();
        let mut total_delta = Decimal::ZERO;
        let mut summary_seen = false;

        for (idx, tr) in table.select(&TR_SELECTOR).enumerate() {
            if idx < 3 {
                // Первые строки — заголовки, не данные.
                continue;
            }
            let cells = row_cells(tr);
            if cells.is_empty() {
                continue;
            }
            if cells[0].to_lowercase().contains("итого") {
                if let Some(last) = cells.last() {
                    total_delta = parse_money_or_zero(last, "Итого")?;
                    summary_seen = true;
                }
                continue;
            }
            if cells.len() < 10 {
                ensure_min_cells(TABLE_ASSET_VALUATION, idx, cells.len(), 10, mode, warnings)?;
                continue;
            }

            rows.push(AssetValuationRow {
                venue: cells[0].clone(),
                start_securities: parse_money_or_zero(&cells[1], "ЦБ начало")?,
                start_cash: parse_money_or_zero(&cells[2], "Денежные средства начало")?,
                start_total: parse_money_or_zero(&cells[3], "Всего начало")?,
                end_securities: parse_money_or_zero(&cells[4], "ЦБ конец")?,
                end_cash: parse_money_or_zero(&cells[5], "Денежные средства конец")?,
                end_total: parse_money_or_zero(&cells[6], "Всего конец")?,
                delta_securities: parse_money_or_zero(&cells[7], "ЦБ изменение")?,
                delta_cash: parse_money_or_zero(&cells[8], "Денежные средства изменение")?,
                delta_total: parse_money_or_zero(&cells[9], "Всего изменение")?,
            });
        }

        if !summary_seen {
            total_delta = rows
                .iter()
                .map(|r| r.delta_total)
                .fold(Decimal::ZERO, |acc, v| acc + v);
        }

        Ok(AssetValuation::new(rows, total_delta))
    }

    /// Парсит «Сводную информацию по движению ДС».
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если таблица отсутствует или в её строках не удалось распарсить суммы.
    pub fn parse_cash_flow_summary(&self) -> Result<CashFlowSummary, ReportError> {
        let mut ignored_warnings = Vec::new();
        self.parse_cash_flow_summary_with_mode(ParseMode::Lenient, &mut ignored_warnings)
    }

    pub(crate) fn parse_cash_flow_summary_with_mode(
        &self,
        mode: ParseMode,
        warnings: &mut Vec<ParseWarning>,
    ) -> Result<CashFlowSummary, ReportError> {
        let table = find_table_with_headers(&self.doc, &["Описание", "Сумма", "Валюта"], None)
            .ok_or(ReportError::TableNotFound {
                table: TABLE_CASH_FLOW,
            })?;

        let mut rows = Vec::new();
        for (idx, tr) in table.select(&TR_SELECTOR).enumerate() {
            if idx < 2 {
                continue;
            }
            let cells = row_cells(tr);
            if cells.len() < 3 {
                ensure_min_cells(TABLE_CASH_FLOW, idx, cells.len(), 3, mode, warnings)?;
                continue;
            }
            if cells.iter().all(String::is_empty) {
                continue;
            }
            let description = cells[0].clone();
            rows.push(CashFlowRow {
                kind: classify_cash_flow(&description),
                description_raw: description,
                amount: parse_money_or_zero(&cells[1], "Сумма ДС")?,
                currency: cells[2].clone(),
            });
        }

        Ok(CashFlowSummary::new(rows))
    }

    /// Парсит таблицу «Портфель ценных бумаг».
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если таблица отсутствует или в её строках невалидные значения.
    pub fn parse_portfolio(&self) -> Result<Portfolio, ReportError> {
        let mut ignored_warnings = Vec::new();
        self.parse_portfolio_with_mode(ParseMode::Lenient, &mut ignored_warnings)
    }

    pub(crate) fn parse_portfolio_with_mode(
        &self,
        mode: ParseMode,
        warnings: &mut Vec<ParseWarning>,
    ) -> Result<Portfolio, ReportError> {
        // Здесь заголовок занимает две строки.
        let table = find_table_with_headers(
            &self.doc,
            &[
                "ISIN",
                "Рыночная стоимость, без НКД",
                "Рыночная цена",
                "Плановые зачисления",
            ],
            Some(2),
        )
        .ok_or(ReportError::TableNotFound {
            table: TABLE_PORTFOLIO,
        })?;

        let mut markets: Vec<PortfolioMarket> = Vec::new();
        let mut current_market: Option<PortfolioMarket> = None;

        for (idx, tr) in table.select(&TR_SELECTOR).enumerate() {
            if idx < 3 {
                continue;
            }
            let cells = row_cells(tr);
            if cells.is_empty() {
                continue;
            }
            if cells[0].starts_with("Площадка") {
                // Разделитель блоков по рынкам.
                if let Some(m) = current_market.take() {
                    markets.push(m);
                }
                let name = cells[0].trim_start_matches("Площадка:").trim().to_string();
                current_market = Some(PortfolioMarket {
                    name,
                    positions: Vec::new(),
                });
                continue;
            }
            if cells.len() < 18 {
                ensure_min_cells(TABLE_PORTFOLIO, idx, cells.len(), 18, mode, warnings)?;
                continue;
            }
            let position = SecurityPosition {
                name: cells[0].clone(),
                isin: cells[1].clone(),
                price_currency: cells[2].clone(),
                qty_start: parse_money_or_zero(&cells[3], "Количество начало")?,
                nominal_start: parse_money_or_zero(&cells[4], "Номинал начало")?,
                price_start: parse_money_or_zero(&cells[5], "Цена начало")?,
                value_start_no_ai: parse_money_or_zero(&cells[6], "Стоимость без НКД начало")?,
                accrued_interest_start: parse_money_or_zero(&cells[7], "НКД начало")?,
                qty_end: parse_money_or_zero(&cells[8], "Количество конец")?,
                nominal_end: parse_money_or_zero(&cells[9], "Номинал конец")?,
                price_end: parse_money_or_zero(&cells[10], "Цена конец")?,
                value_end_no_ai: parse_money_or_zero(&cells[11], "Стоимость без НКД конец")?,
                accrued_interest_end: parse_money_or_zero(&cells[12], "НКД конец")?,
                qty_delta: parse_money_or_zero(&cells[13], "Количество изменение")?,
                value_delta: parse_money_or_zero(&cells[14], "Стоимость изменение")?,
                planned_in_qty: parse_money_or_zero(&cells[15], "Плановые зачисления")?,
                planned_out_qty: parse_money_or_zero(&cells[16], "Плановые списания")?,
                planned_end_qty: parse_money_or_zero(&cells[17], "Плановый исходящий остаток")?,
            };

            if let Some(market) = current_market.as_mut() {
                market.positions.push(position);
            } else {
                current_market = Some(PortfolioMarket {
                    name: "Неизвестно".to_string(),
                    positions: vec![position],
                });
            }
        }

        if let Some(market) = current_market {
            markets.push(market);
        }

        Ok(Portfolio::new(markets))
    }

    /// Парсит таблицу пополнений ИИС, если она есть в отчёте.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если таблица отсутствует или в строках встречены невалидные значения.
    pub fn parse_iis_contributions(&self) -> Result<IisContributionsTable, ReportError> {
        let mut ignored_warnings = Vec::new();
        self.parse_iis_contributions_with_mode(ParseMode::Lenient, &mut ignored_warnings)
    }

    pub(crate) fn parse_iis_contributions_with_mode(
        &self,
        mode: ParseMode,
        warnings: &mut Vec<ParseWarning>,
    ) -> Result<IisContributionsTable, ReportError> {
        let table = find_table_with_headers(
            &self.doc,
            &[
                "Год",
                "Лимит, руб.",
                "Дата операции",
                "Сумма, руб.",
                "Основание операции",
                "Остаток лимита",
            ],
            None,
        )
        .ok_or(ReportError::TableNotFound { table: TABLE_IIS })?;

        let mut rows = Vec::new();
        // Год и лимит могут быть только в первой строке блока.
        let mut current_year: Option<i32> = None;
        let mut current_limit: Option<IisLimit> = None;

        for (idx, tr) in table.select(&TR_SELECTOR).enumerate() {
            if idx < 3 {
                continue;
            }
            let cells = row_cells(tr);
            if cells.len() < 6 {
                ensure_min_cells(TABLE_IIS, idx, cells.len(), 6, mode, warnings)?;
                continue;
            }
            if cells.iter().all(String::is_empty) {
                continue;
            }
            if !cells[0].is_empty() {
                current_year =
                    Some(
                        cells[0]
                            .trim()
                            .parse::<i32>()
                            .map_err(|_| ReportError::Number {
                                value: cells[0].clone(),
                                column: "Год",
                            })?,
                    );
            }
            if !cells[1].is_empty() {
                current_limit = Some(parse_iis_limit(&cells[1], "Лимит ИИС")?);
            }
            if cells[2].is_empty() {
                continue;
            }

            let year = current_year.ok_or(ReportError::MissingField { field: "Год" })?;
            let limit = current_limit.unwrap_or(IisLimit::Amount(Decimal::ZERO));
            let date = parse_date(&cells[2])?;
            let amount = parse_money_or_zero(&cells[3], "Сумма ИИС")?;
            let remaining_limit = parse_iis_limit(&cells[5], "Остаток лимита")?;

            rows.push(IisContribution {
                year,
                limit_rub: limit,
                date,
                amount,
                operation_reason: cells[4].clone(),
                remaining_limit,
            });
        }

        Ok(IisContributionsTable::new(rows))
    }
}

/// Классифицирует строку сводки ДС по известным типам.
fn classify_cash_flow(description: &str) -> CashFlowKind {
    let lower = description.to_lowercase();
    CASH_FLOW_RULES
        .iter()
        .find_map(|(needle, kind)| lower.contains(needle).then_some(*kind))
        .unwrap_or(CashFlowKind::Unknown)
}

fn parse_capture_date(
    captures: &regex::Captures<'_>,
    index: usize,
    original_text: &str,
) -> Result<chrono::NaiveDate, ReportError> {
    let value = captures
        .get(index)
        .ok_or_else(|| ReportError::Regex(original_text.to_string()))?
        .as_str();
    parse_date(value)
}

#[allow(clippy::missing_const_for_fn)]
fn ensure_min_cells(
    table: &'static str,
    row_index: usize,
    actual_cells: usize,
    expected_cells: usize,
    mode: ParseMode,
    warnings: &mut Vec<ParseWarning>,
) -> Result<(), ReportError> {
    if mode.is_strict() {
        return Err(ReportError::MalformedRow {
            table,
            row_index,
            expected_cells,
            actual_cells,
        });
    }
    warnings.push(ParseWarning::MalformedRow {
        table,
        row_index,
        expected_cells,
        actual_cells,
    });
    Ok(())
}

fn parse_iis_limit(value: &str, column: &'static str) -> Result<IisLimit, ReportError> {
    if value.to_lowercase().contains("ограничений нет") {
        Ok(IisLimit::Unlimited)
    } else {
        parse_money_or_zero(value, column).map(IisLimit::Amount)
    }
}

fn row_cells(tr: ElementRef<'_>) -> Vec<String> {
    tr.select(&TD_SELECTOR).map(collect_text).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn classify_cash_flow_matches_known_labels() {
        assert_eq!(
            classify_cash_flow("Входящий остаток денежных средств"),
            CashFlowKind::OpeningBalance
        );
        assert_eq!(
            classify_cash_flow("Комиссия брокера, удержанная за операции"),
            CashFlowKind::BrokerFee
        );
        assert_eq!(
            classify_cash_flow("исходящий остаток"),
            CashFlowKind::ClosingBalance
        );
    }

    #[test]
    fn classify_cash_flow_unknown_for_unmatched_description() {
        assert_eq!(
            classify_cash_flow("Произвольная строка"),
            CashFlowKind::Unknown
        );
    }

    #[test]
    fn parse_iis_limit_handles_unlimited_and_amount() {
        assert_eq!(
            parse_iis_limit("ОГРАНИЧЕНИЙ НЕТ", "Лимит").expect("must parse unlimited"),
            IisLimit::Unlimited
        );
        assert_eq!(
            parse_iis_limit("12345.67", "Лимит").expect("must parse amount"),
            IisLimit::Amount(Decimal::new(1_234_567, 2))
        );
    }
}
