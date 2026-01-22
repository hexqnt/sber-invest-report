//! Набор отчётов и функции их агрегации.

use crate::error::ReportError;
use crate::raw::RawReport;
use crate::report::{Report, ReportBuilder};
use crate::types::{
    AccountId, CashFlowKind, CashFlowRow, CashFlowSummary, MergedPosition, Money, PortfolioMarket,
    SecurityPosition,
};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::fs::{self, DirEntry};
use std::path::Path;

/// Набор отчётов с утилитами для агрегации.
#[derive(Debug, Clone, Default)]
pub struct ReportSet {
    /// Собранные отчёты.
    pub reports: Vec<Report>,
}

impl ReportSet {
    /// Загружает и парсит все HTML-файлы из каталога с полным набором таблиц.
    #[inline]
    pub fn from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, ReportError> {
        Self::from_dir_with(dir, |builder| builder.parse())
    }

    /// Загружает и парсит все HTML-файлы из каталога, позволяя настроить билдер.
    ///
    /// # Пример
    ///
    /// ```
    /// # use sber_invest_report::ReportSet;
    /// # let dir = "tests/fixtures";
    /// let set = ReportSet::from_dir_with(dir, |builder| builder.cash_flow(true).portfolio(false).parse()).unwrap();
    /// assert!(!set.reports.is_empty());
    /// ```
    pub fn from_dir_with<P, F>(dir: P, mut parse_fn: F) -> Result<Self, ReportError>
    where
        P: AsRef<Path>,
        for<'a> F: FnMut(ReportBuilder<'a>) -> Result<Report, ReportError>,
    {
        let mut entries: Vec<_> = fs::read_dir(dir)?
            .filter_map(std::result::Result::ok)
            .collect();
        // Делаем порядок файлов детерминированным.
        entries.sort_by_key(DirEntry::path);

        let mut reports = Vec::new();
        for entry in entries {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lower = ext.to_ascii_lowercase();
                if ext_lower != "html" && ext_lower != "htm" {
                    continue;
                }
            } else {
                continue;
            }

            let file = fs::File::open(&path)?;
            let raw = RawReport::from_reader(file)?;
            let report = parse_fn(ReportBuilder::new(&raw))?;
            reports.push(report);
        }

        Ok(Self { reports })
    }

    /// Возвращает итератор по отчётам конкретного договора.
    #[inline]
    pub fn by_account<'a>(&'a self, id: &'a AccountId) -> impl Iterator<Item = &'a Report> {
        self.reports
            .iter()
            .filter(move |r| &r.meta.account_id == id)
    }

    /// Объединяет таблицы движения денежных средств по всем отчётам.
    pub fn merge_cash_flows(&self) -> CashFlowSummary {
        let mut map: BTreeMap<(CashFlowKind, String), (Money, String)> = BTreeMap::new();
        for report in &self.reports {
            if let Some(summary) = &report.cash_flow_summary {
                for row in &summary.rows {
                    let key = (row.kind, row.currency.clone());
                    let entry = map
                        .entry(key)
                        .or_insert((Decimal::ZERO, row.description_raw.clone()));
                    entry.0 += row.amount;
                }
            }
        }

        let rows = map
            .into_iter()
            .map(
                |((kind, currency), (amount, description_raw))| CashFlowRow {
                    kind,
                    description_raw,
                    amount,
                    currency,
                },
            )
            .collect();

        CashFlowSummary { rows }
    }

    /// Агрегирует позиции по ISIN из портфелей всех отчётов.
    pub fn merge_positions(&self) -> Vec<MergedPosition> {
        let mut map: BTreeMap<String, MergedPosition> = BTreeMap::new();

        for report in &self.reports {
            if let Some(portfolio) = &report.portfolio {
                for PortfolioMarket { positions, .. } in &portfolio.markets {
                    for SecurityPosition {
                        isin,
                        name,
                        price_currency,
                        qty_start,
                        qty_end,
                        value_start_no_ai,
                        value_end_no_ai,
                        qty_delta,
                        value_delta,
                        ..
                    } in positions
                    {
                        let entry = map.entry(isin.clone()).or_insert(MergedPosition {
                            isin: isin.clone(),
                            name: name.clone(),
                            price_currency: price_currency.clone(),
                            qty_start: Decimal::ZERO,
                            qty_end: Decimal::ZERO,
                            value_start_no_ai: Decimal::ZERO,
                            value_end_no_ai: Decimal::ZERO,
                            qty_delta: Decimal::ZERO,
                            value_delta: Decimal::ZERO,
                        });

                        entry.qty_start += *qty_start;
                        entry.qty_end += *qty_end;
                        entry.value_start_no_ai += *value_start_no_ai;
                        entry.value_end_no_ai += *value_end_no_ai;
                        entry.qty_delta += *qty_delta;
                        entry.value_delta += *value_delta;
                    }
                }
            }
        }

        map.into_values().collect()
    }
}
