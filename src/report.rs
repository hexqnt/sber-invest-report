//! Обёртка над парсерами для построения итогового отчёта.

use crate::error::ReportError;
use crate::raw::{DomReport, RawReport};
use crate::types::{
    AssetValuation, CashFlowSummary, IisContributionsTable, Portfolio, ReportMetadata,
};

/// Набор флагов, определяющий, какие таблицы загружать (внутренний тип).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ParseOptions {
    pub load_asset_valuation: bool,
    pub load_cash_flow: bool,
    pub load_portfolio: bool,
    pub load_iis_contributions: bool,
}

impl ParseOptions {
    /// Загружает все известные таблицы.
    pub const fn everything() -> Self {
        Self {
            load_asset_valuation: true,
            load_cash_flow: true,
            load_portfolio: true,
            load_iis_contributions: true,
        }
    }

    #[allow(dead_code)]
    /// Отключает парсинг всех таблиц, оставляя только метаданные.
    pub const fn meta_only() -> Self {
        Self {
            load_asset_valuation: false,
            load_cash_flow: false,
            load_portfolio: false,
            load_iis_contributions: false,
        }
    }
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self::everything()
    }
}

/// Итоговая модель одного отчёта.
#[derive(Debug, Clone)]
pub struct Report {
    /// Метаданные отчёта.
    pub meta: ReportMetadata,
    /// Таблица «Оценка активов, руб.».
    pub asset_valuation: Option<AssetValuation>,
    /// Сводка движения денежных средств.
    pub cash_flow_summary: Option<CashFlowSummary>,
    /// Портфель ценных бумаг.
    pub portfolio: Option<Portfolio>,
    /// Таблица пополнений ИИС.
    pub iis_contributions: Option<IisContributionsTable>,
}

impl Report {
    /// Парсит один HTML-отчёт, загружая все таблицы.
    #[inline]
    pub fn parse(raw: &RawReport) -> Result<Self, ReportError> {
        Self::parse_with_options(raw, ParseOptions::everything())
    }

    /// Парсит отчёт с внутренними опциями (используется билдером).
    pub(crate) fn parse_with_options(
        raw: &RawReport,
        options: ParseOptions,
    ) -> Result<Self, ReportError> {
        let dom = DomReport::parse(raw)?;
        let meta = dom.meta()?;

        let asset_valuation =
            parse_optional(options.load_asset_valuation, || dom.parse_asset_valuation())?;
        let cash_flow_summary =
            parse_optional(options.load_cash_flow, || dom.parse_cash_flow_summary())?;
        let portfolio = parse_optional(options.load_portfolio, || dom.parse_portfolio())?;
        let iis_contributions = parse_optional(options.load_iis_contributions, || {
            dom.parse_iis_contributions()
        })?;

        Ok(Report {
            meta,
            asset_valuation,
            cash_flow_summary,
            portfolio,
            iis_contributions,
        })
    }
}

/// Builder для удобного парсинга `Report` с выбором таблиц.
pub struct ReportBuilder<'a> {
    raw: &'a RawReport,
    options: ParseOptions,
}

impl<'a> ReportBuilder<'a> {
    /// Создаёт builder для указанного исходного отчёта.
    ///
    /// # Пример
    ///
    /// ```
    /// # use sber_invest_report::{RawReport, ReportBuilder};
    /// # let raw = RawReport::from_str("<html></html>");
    /// let report = ReportBuilder::new(&raw)
    ///     .cash_flow(true)
    ///     .portfolio(false)
    ///     .parse();
    /// ```
    #[inline]
    pub fn new(raw: &'a RawReport) -> Self {
        Self {
            raw,
            options: ParseOptions::everything(),
        }
    }

    /// Включает или отключает таблицу оценки активов.
    #[inline]
    pub const fn asset_valuation(mut self, enabled: bool) -> Self {
        self.options.load_asset_valuation = enabled;
        self
    }

    /// Включает или отключает таблицу движения ДС.
    #[inline]
    pub const fn cash_flow(mut self, enabled: bool) -> Self {
        self.options.load_cash_flow = enabled;
        self
    }

    /// Включает или отключает портфель ценных бумаг.
    #[inline]
    pub const fn portfolio(mut self, enabled: bool) -> Self {
        self.options.load_portfolio = enabled;
        self
    }

    /// Включает или отключает таблицу взносов на ИИС.
    #[inline]
    pub const fn iis_contributions(mut self, enabled: bool) -> Self {
        self.options.load_iis_contributions = enabled;
        self
    }

    /// Выполняет парсинг с текущими настройками.
    #[inline]
    pub fn parse(self) -> Result<Report, ReportError> {
        Report::parse_with_options(self.raw, self.options)
    }
}

/// Вызывает парсер таблицы, возвращая `None`, если таблица отсутствует.
fn parse_optional<T, F>(enabled: bool, loader: F) -> Result<Option<T>, ReportError>
where
    F: FnOnce() -> Result<T, ReportError>,
{
    if !enabled {
        return Ok(None);
    }
    // Отсутствие таблицы — нормальный случай для части отчётов.
    match loader() {
        Ok(value) => Ok(Some(value)),
        Err(ReportError::TableNotFound { .. }) => Ok(None),
        Err(err) => Err(err),
    }
}
