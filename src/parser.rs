//! Парсинг конкретных таблиц отчёта из DOM.

use std::sync::LazyLock;

use crate::error::ReportError;
use crate::raw::DomReport;
use crate::types::{
    AccountId, AccountKind, AssetValuation, AssetValuationRow, CashFlowKind, CashFlowRow,
    CashFlowSummary, IisContribution, IisContributionsTable, Money, Portfolio, PortfolioMarket,
    ReportMetadata, SecurityPosition,
};
use crate::utils::{
    capitalize_words,
    capture_text,
    collect_text,
    find_table_with_headers,
    parse_date,
    parse_money_or_zero,
};
use regex::Regex;
use rust_decimal::Decimal;
use scraper::Selector;

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

impl DomReport {
    /// Извлекает метаданные из шапки отчёта.
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

        let period_start = parse_date(period_caps.get(1).unwrap().as_str())?;
        let period_end = parse_date(period_caps.get(2).unwrap().as_str())?;
        let generated_at = parse_date(period_caps.get(3).unwrap().as_str())?;

        let mut investor_block = None;
        for p in self.doc.select(&P_SELECTOR) {
            let text: String = p.text().collect();
            if text.to_lowercase().contains("инвестор") {
                investor_block = Some(text);
                break;
            }
        }
        let investor_text =
            investor_block.ok_or(ReportError::MissingField { field: "investor" })?;

        let investor_name = capture_text(&investor_text, &INVESTOR_RE)
            .map(|c| c.trim().to_string())
            .map(|name| capitalize_words(&name))
            .ok_or_else(|| ReportError::Regex(investor_text.clone()))?;

        let contract_number = capture_text(&investor_text, &CONTRACT_RE)
            .map(|c| c.trim().to_string())
            .ok_or_else(|| ReportError::Regex(investor_text.clone()))?;

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
    pub fn parse_asset_valuation(&self) -> Result<AssetValuation, ReportError> {
        let table = self
            .doc
            .select(&RATING_SELECTOR)
            .next()
            .ok_or(ReportError::TableNotFound {
                table: "RatingAssets",
            })?;

        let mut rows = Vec::new();
        let mut total_delta = Decimal::ZERO;
        let mut summary_seen = false;

        for (idx, tr) in table.select(&TR_SELECTOR).enumerate() {
            if idx < 3 {
                continue;
            }
            let cells: Vec<String> = tr.select(&TD_SELECTOR).map(collect_text).collect();
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

        Ok(AssetValuation { rows, total_delta })
    }

    /// Парсит «Сводную информацию по движению ДС».
    pub fn parse_cash_flow_summary(&self) -> Result<CashFlowSummary, ReportError> {
        let table = find_table_with_headers(
            &self.doc,
            &["Описание", "Сумма", "Валюта"],
            None
        ).ok_or(
            ReportError::TableNotFound {
                table: "CashFlowSummary",
            },
        )?;

        let mut rows = Vec::new();
        for (idx, tr) in table.select(&TR_SELECTOR).enumerate() {
            if idx < 2 {
                continue;
            }
            let cells: Vec<String> = tr.select(&TD_SELECTOR).map(collect_text).collect();
            if cells.len() < 3 {
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

        Ok(CashFlowSummary { rows })
    }

    /// Парсит таблицу «Портфель ценных бумаг».
    pub fn parse_portfolio(&self) -> Result<Portfolio, ReportError> {
        let table = find_table_with_headers(
            &self.doc,
            &[
                "ISIN",
                "Рыночная стоимость, без НКД",
                "Рыночная цена",
                "Плановые зачисления",
            ],
            Some(2)
        )
        .ok_or(ReportError::TableNotFound { table: "Portfolio" })?;

        let mut markets: Vec<PortfolioMarket> = Vec::new();
        let mut current_market: Option<PortfolioMarket> = None;

        for (idx, tr) in table.select(&TR_SELECTOR).enumerate() {
            if idx < 3 {
                continue;
            }
            let cells: Vec<String> = tr.select(&TD_SELECTOR).map(collect_text).collect();
            if cells.is_empty() {
                continue;
            }
            if cells[0].starts_with("Площадка") {
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

        Ok(Portfolio { markets })
    }

    /// Парсит таблицу пополнений ИИС, если она есть в отчёте.
    pub fn parse_iis_contributions(&self) -> Result<IisContributionsTable, ReportError> {
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
            None
        )
        .ok_or(ReportError::TableNotFound {
            table: "IISContributions",
        })?;

        let mut rows = Vec::new();
        let mut current_year: Option<i32> = None;
        let mut current_limit: Option<Money> = None;

        for (idx, tr) in table.select(&TR_SELECTOR).enumerate() {
            if idx < 3 {
                continue;
            }
            let cells: Vec<String> = tr.select(&TD_SELECTOR).map(collect_text).collect();
            if cells.len() < 6 {
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
                let lower = cells[1].to_lowercase();
                if lower.contains("ограничений нет") {
                    current_limit = Some(Decimal::ZERO);
                } else {
                    current_limit = Some(parse_money_or_zero(&cells[1], "Лимит ИИС")?);
                }
            }
            if cells[2].is_empty() {
                continue;
            }

            let year = current_year.ok_or(ReportError::MissingField { field: "Год" })?;
            let limit = current_limit.unwrap_or(Decimal::ZERO);
            let date = parse_date(&cells[2])?;
            let amount = parse_money_or_zero(&cells[3], "Сумма ИИС")?;
            let remaining_limit = {
                let lower = cells[5].to_lowercase();
                if lower.contains("ограничений нет") {
                    Decimal::ZERO
                } else {
                    parse_money_or_zero(&cells[5], "Остаток лимита")?
                }
            };

            rows.push(IisContribution {
                year,
                limit_rub: limit,
                date,
                amount,
                operation_reason: cells[4].clone(),
                remaining_limit,
            });
        }

        Ok(IisContributionsTable { rows })
    }
}

/// Классифицирует строку сводки ДС по известным типам.
fn classify_cash_flow(description: &str) -> CashFlowKind {
    let lower = description.to_lowercase();
    if lower.contains("входящий остаток") {
        CashFlowKind::OpeningBalance
    } else if lower.contains("сальдо расчетов по сделкам") {
        CashFlowKind::TradesNet
    } else if lower.contains("корпоративные действия") {
        CashFlowKind::CorporateActions
    } else if lower.contains("комиссия брокера") {
        CashFlowKind::BrokerFee
    } else if lower.contains("комиссия биржи") {
        CashFlowKind::ExchangeFee
    } else if lower.contains("исходящий остаток") {
        CashFlowKind::ClosingBalance
    } else {
        CashFlowKind::Unknown
    }
}
