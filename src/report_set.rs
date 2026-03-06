//! Набор отчётов и функции их агрегации.

use crate::error::ReportError;
use crate::parse_config::ParseConfig;
use crate::raw::RawReport;
use crate::report::{Report, ReportBuilder};
use crate::types::{
    AccountId, CashFlowKind, CashFlowRow, CashFlowSummary, MergedPosition, Money, SecurityPosition,
};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::fs::{self, DirEntry};
use std::path::Path;

/// Набор отчётов с утилитами для агрегации.
#[derive(Debug, Clone, Default)]
pub struct ReportSet {
    /// Собранные отчёты.
    pub(crate) reports: Vec<Report>,
}

impl ReportSet {
    /// Создаёт набор отчётов из готового списка.
    #[must_use]
    pub const fn new(reports: Vec<Report>) -> Self {
        Self { reports }
    }

    /// Возвращает срез отчётов.
    #[must_use]
    pub fn reports(&self) -> &[Report] {
        &self.reports
    }

    /// Возвращает количество отчётов.
    #[must_use]
    pub fn len(&self) -> usize {
        self.reports.len()
    }

    /// Возвращает `true`, если набор пуст.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }

    /// Возвращает итератор по отчётам набора.
    pub fn iter_reports(&self) -> impl Iterator<Item = &Report> {
        self.reports.iter()
    }

    /// Возвращает итератор по строкам движения денежных средств всех отчётов.
    pub fn iter_cash_flows(&self) -> impl Iterator<Item = &CashFlowRow> {
        self.reports.iter().flat_map(Report::cash_flow_rows)
    }

    /// Возвращает итератор по позициям портфеля всех отчётов.
    pub fn iter_positions(&self) -> impl Iterator<Item = &SecurityPosition> {
        self.reports.iter().flat_map(Report::positions)
    }

    /// Загружает и парсит все HTML-файлы из каталога с полным набором таблиц.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если не удалось прочитать каталог/файлы или распарсить отчёт.
    #[inline]
    pub fn from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, ReportError> {
        Self::from_dir_with(dir, parse_with_default_builder)
    }

    /// Загружает и парсит все HTML-файлы из каталога с указанной конфигурацией.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если не удалось прочитать каталог/файлы или распарсить отчёт.
    #[inline]
    pub fn from_dir_with_config<P: AsRef<Path>>(
        dir: P,
        config: ParseConfig,
    ) -> Result<Self, ReportError> {
        Self::from_dir_with(dir, |builder| builder.config(config).parse())
    }

    /// Загружает и парсит все HTML-файлы из каталога в строгом режиме.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если у отчёта отсутствует запрошенная таблица или структура строк нарушена.
    #[inline]
    pub fn from_dir_strict<P: AsRef<Path>>(dir: P) -> Result<Self, ReportError> {
        Self::from_dir_with_config(dir, ParseConfig::strict())
    }

    /// Загружает и парсит все HTML-файлы из каталога, позволяя настроить билдер.
    ///
    /// # Пример
    ///
    /// ```
    /// # use sber_invest_report::{ReportSection, ReportSet, SectionSet};
    /// # let dir = "tests/fixtures";
    /// let set = ReportSet::from_dir_with(dir, |builder| {
    ///     builder
    ///         .sections(
    ///             SectionSet::meta_only()
    ///                 .with(ReportSection::CashFlowSummary)
    ///                 .with(ReportSection::Portfolio),
    ///         )
    ///         .section(ReportSection::Portfolio, false)
    ///         .parse()
    /// }).unwrap();
    /// assert!(!set.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если не удалось прочитать каталог/файлы или `parse_fn` вернул ошибку.
    pub fn from_dir_with<P, F>(dir: P, mut parse_fn: F) -> Result<Self, ReportError>
    where
        P: AsRef<Path>,
        for<'a> F: FnMut(ReportBuilder<'a>) -> Result<Report, ReportError>,
    {
        let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<Vec<DirEntry>, _>>()?;
        // Делаем порядок файлов детерминированным.
        entries.sort_by_key(DirEntry::path);

        let mut reports = Vec::new();
        for entry in entries {
            let path = entry.path();
            if !is_html_file(&path) {
                continue;
            }

            let file = fs::File::open(&path)?;
            let raw = RawReport::from_reader(file)?;
            let report = parse_fn(ReportBuilder::new(&raw))?;
            reports.push(report);
        }

        Ok(Self::new(reports))
    }

    /// Возвращает итератор по отчётам конкретного договора.
    #[inline]
    pub fn by_account<'a>(&'a self, id: &'a AccountId) -> impl Iterator<Item = &'a Report> {
        self.iter_reports()
            .filter(move |r| &r.meta().account_id == id)
    }

    /// Объединяет таблицы движения денежных средств по всем отчётам.
    #[must_use]
    pub fn merge_cash_flows(&self) -> CashFlowSummary {
        let mut map: BTreeMap<(CashFlowKind, String, Option<String>), (Money, String)> =
            BTreeMap::new();

        for row in self.iter_cash_flows() {
            let key = (
                row.kind,
                row.currency.clone(),
                (row.kind == CashFlowKind::Unknown).then(|| row.description_raw.clone()),
            );
            let entry = map
                .entry(key)
                .or_insert_with(|| (Decimal::ZERO, row.description_raw.clone()));
            entry.0 += row.amount;
        }

        let rows = map
            .into_iter()
            .map(
                |((kind, currency, _), (amount, description_raw))| CashFlowRow {
                    kind,
                    description_raw,
                    amount,
                    currency,
                },
            )
            .collect();

        CashFlowSummary::new(rows)
    }

    /// Агрегирует позиции по ISIN из портфелей всех отчётов.
    #[must_use]
    pub fn merge_positions(&self) -> Vec<MergedPosition> {
        let mut map: BTreeMap<String, MergedPosition> = BTreeMap::new();

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
        } in self.iter_positions()
        {
            let entry = map.entry(isin.clone()).or_insert_with(|| MergedPosition {
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

        map.into_values().collect()
    }
}

fn is_html_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "html" | "htm"))
}

fn parse_with_default_builder(builder: ReportBuilder<'_>) -> Result<Report, ReportError> {
    builder.parse()
}
