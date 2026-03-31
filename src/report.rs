//! Обёртка над парсерами для построения итогового отчёта.

use crate::diagnostics::ParseWarning;
use crate::error::ReportError;
use crate::parse_config::{ParseConfig, ParseMode, ReportSection, SectionSet};
use crate::raw::{DomReport, RawReport};
use crate::types::{
    AssetValuation, CashFlowRow, CashFlowSummary, IisContribution, IisContributionsTable,
    Portfolio, PortfolioMarket, ReportMetadata, SecurityPosition,
};

/// Итоговая модель одного отчёта.
#[derive(Debug, Clone)]
pub struct Report {
    /// Метаданные отчёта.
    pub(crate) meta: ReportMetadata,
    /// Таблица «Оценка активов, руб.».
    pub(crate) asset_valuation: Option<AssetValuation>,
    /// Сводка движения денежных средств.
    pub(crate) cash_flow_summary: Option<CashFlowSummary>,
    /// Портфель ценных бумаг.
    pub(crate) portfolio: Option<Portfolio>,
    /// Таблица пополнений ИИС.
    pub(crate) iis_contributions: Option<IisContributionsTable>,
}

impl Report {
    /// Возвращает метаданные отчёта.
    #[must_use]
    pub const fn meta(&self) -> &ReportMetadata {
        &self.meta
    }

    /// Возвращает таблицу оценки активов, если она была запрошена и найдена.
    #[must_use]
    pub const fn asset_valuation(&self) -> Option<&AssetValuation> {
        self.asset_valuation.as_ref()
    }

    /// Возвращает сводку движения денежных средств, если она была запрошена и найдена.
    #[must_use]
    pub const fn cash_flow_summary(&self) -> Option<&CashFlowSummary> {
        self.cash_flow_summary.as_ref()
    }

    /// Возвращает портфель, если он был запрошен и найден.
    #[must_use]
    pub const fn portfolio(&self) -> Option<&Portfolio> {
        self.portfolio.as_ref()
    }

    /// Возвращает таблицу пополнений ИИС, если она была запрошена и найдена.
    #[must_use]
    pub const fn iis_contributions(&self) -> Option<&IisContributionsTable> {
        self.iis_contributions.as_ref()
    }

    /// Возвращает копию отчёта с заменённой таблицей оценки активов.
    #[must_use]
    pub fn with_asset_valuation(mut self, asset_valuation: Option<AssetValuation>) -> Self {
        self.asset_valuation = asset_valuation;
        self
    }

    /// Возвращает копию отчёта с заменённой сводкой движения денежных средств.
    #[must_use]
    pub fn with_cash_flow_summary(mut self, cash_flow_summary: Option<CashFlowSummary>) -> Self {
        self.cash_flow_summary = cash_flow_summary;
        self
    }

    /// Возвращает копию отчёта с заменённым портфелем.
    #[must_use]
    pub fn with_portfolio(mut self, portfolio: Option<Portfolio>) -> Self {
        self.portfolio = portfolio;
        self
    }

    /// Возвращает копию отчёта с заменённой таблицей пополнений ИИС.
    #[must_use]
    pub fn with_iis_contributions(
        mut self,
        iis_contributions: Option<IisContributionsTable>,
    ) -> Self {
        self.iis_contributions = iis_contributions;
        self
    }

    /// Возвращает итератор по строкам движения денежных средств без дополнительных аллокаций.
    #[inline]
    pub fn cash_flow_rows(&self) -> impl Iterator<Item = &CashFlowRow> {
        self.cash_flow_summary
            .iter()
            .flat_map(CashFlowSummary::iter_rows)
    }

    /// Возвращает итератор по площадкам портфеля без дополнительных аллокаций.
    #[inline]
    pub fn markets(&self) -> impl Iterator<Item = &PortfolioMarket> {
        self.portfolio.iter().flat_map(Portfolio::iter_markets)
    }

    /// Возвращает итератор по всем позициям портфеля без дополнительных аллокаций.
    #[inline]
    pub fn positions(&self) -> impl Iterator<Item = &SecurityPosition> {
        self.portfolio.iter().flat_map(Portfolio::iter_positions)
    }

    /// Возвращает итератор по строкам таблицы пополнений ИИС без дополнительных аллокаций.
    #[inline]
    pub fn iis_rows(&self) -> impl Iterator<Item = &IisContribution> {
        self.iis_contributions
            .iter()
            .flat_map(IisContributionsTable::iter_rows)
    }

    /// Парсит один HTML-отчёт в мягком режиме, загружая все секции.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если не удалось распарсить метаданные или данные включённых секций.
    #[inline]
    pub fn parse(raw: &RawReport) -> Result<Self, ReportError> {
        Self::parse_with_config(raw, ParseConfig::default())
    }

    /// Парсит один HTML-отчёт в строгом режиме, требуя успешный разбор всех секций.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если отсутствует запрошенная таблица или структура строк не соответствует ожиданиям.
    #[inline]
    pub fn parse_strict(raw: &RawReport) -> Result<Self, ReportError> {
        Self::parse_with_config(raw, ParseConfig::strict())
    }

    /// Парсит отчёт с явной конфигурацией секций и режима.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку для невалидных данных. В мягком режиме `TableNotFound` для секций
    /// преобразуется в `None`, а в строгом режиме возвращается как ошибка.
    pub fn parse_with_config(raw: &RawReport, config: ParseConfig) -> Result<Self, ReportError> {
        let mut ignored_warnings = Vec::new();
        Self::parse_with_warnings(raw, config, &mut ignored_warnings)
    }

    /// Парсит отчёт и возвращает предупреждения мягкого режима.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если встретились критические проблемы для выбранного режима.
    pub fn parse_with_diagnostics(
        raw: &RawReport,
        config: ParseConfig,
    ) -> Result<(Self, Vec<ParseWarning>), ReportError> {
        let mut warnings = Vec::new();
        let report = Self::parse_with_warnings(raw, config, &mut warnings)?;
        Ok((report, warnings))
    }

    fn parse_with_warnings(
        raw: &RawReport,
        config: ParseConfig,
        warnings: &mut Vec<ParseWarning>,
    ) -> Result<Self, ReportError> {
        let dom = DomReport::parse(raw)?;
        let meta = dom.meta()?;

        let asset_valuation = parse_optional(
            config,
            ReportSection::AssetValuation,
            warnings,
            |warnings| dom.parse_asset_valuation_with_mode(config.mode, warnings),
        )?;
        let cash_flow_summary = parse_optional(
            config,
            ReportSection::CashFlowSummary,
            warnings,
            |warnings| dom.parse_cash_flow_summary_with_mode(config.mode, warnings),
        )?;
        let portfolio = parse_optional(config, ReportSection::Portfolio, warnings, |warnings| {
            dom.parse_portfolio_with_mode(config.mode, warnings)
        })?;
        let iis_contributions = parse_optional(
            config,
            ReportSection::IisContributions,
            warnings,
            |warnings| dom.parse_iis_contributions_with_mode(config.mode, warnings),
        )?;

        Ok(Self {
            meta,
            asset_valuation,
            cash_flow_summary,
            portfolio,
            iis_contributions,
        })
    }
}

/// Builder для удобного парсинга `Report` с выбором секций и режима.
pub struct ReportBuilder<'a> {
    raw: &'a RawReport,
    config: ParseConfig,
}

impl<'a> ReportBuilder<'a> {
    /// Создаёт builder для указанного исходного отчёта.
    ///
    /// # Пример
    ///
    /// ```
    /// # use sber_invest_report::{RawReport, ReportBuilder, ReportSection, SectionSet};
    /// # let raw = RawReport::from_html("<html></html>");
    /// let report = ReportBuilder::new(&raw)
    ///     .sections(
    ///         SectionSet::meta_only()
    ///             .with(ReportSection::CashFlowSummary)
    ///             .with(ReportSection::Portfolio),
    ///     )
    ///     .section(ReportSection::Portfolio, false)
    ///     .parse();
    /// ```
    #[inline]
    #[must_use]
    pub const fn new(raw: &'a RawReport) -> Self {
        Self {
            raw,
            config: ParseConfig::lenient(),
        }
    }

    /// Полностью заменяет конфигурацию парсинга.
    #[inline]
    #[must_use]
    pub const fn config(mut self, config: ParseConfig) -> Self {
        self.config = config;
        self
    }

    /// Устанавливает режим парсинга.
    #[inline]
    #[must_use]
    pub const fn mode(mut self, mode: ParseMode) -> Self {
        self.config = self.config.with_mode(mode);
        self
    }

    /// Переключатель строгого режима.
    #[inline]
    #[must_use]
    pub const fn strict(self, enabled: bool) -> Self {
        let mode = if enabled {
            ParseMode::Strict
        } else {
            ParseMode::Lenient
        };
        self.mode(mode)
    }

    /// Полностью заменяет набор загружаемых секций.
    #[inline]
    #[must_use]
    pub const fn sections(mut self, sections: SectionSet) -> Self {
        self.config = self.config.with_sections(sections);
        self
    }

    /// Включает или отключает конкретную секцию.
    #[inline]
    #[must_use]
    pub const fn section(mut self, section: ReportSection, enabled: bool) -> Self {
        self.config = if enabled {
            self.config.include(section)
        } else {
            self.config.exclude(section)
        };
        self
    }

    /// Выполняет парсинг с текущими настройками.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если парсинг не удался с текущей конфигурацией.
    #[inline]
    pub fn parse(self) -> Result<Report, ReportError> {
        Report::parse_with_config(self.raw, self.config)
    }

    /// Выполняет парсинг и возвращает предупреждения мягкого режима.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если парсинг не удался с текущей конфигурацией.
    #[inline]
    pub fn parse_with_diagnostics(self) -> Result<(Report, Vec<ParseWarning>), ReportError> {
        Report::parse_with_diagnostics(self.raw, self.config)
    }
}

/// Вызывает парсер секции, возвращая `None` в мягком режиме при отсутствии таблицы.
fn parse_optional<T, F>(
    config: ParseConfig,
    section: ReportSection,
    warnings: &mut Vec<ParseWarning>,
    loader: F,
) -> Result<Option<T>, ReportError>
where
    F: FnOnce(&mut Vec<ParseWarning>) -> Result<T, ReportError>,
{
    if !config.loads(section) {
        return Ok(None);
    }

    match loader(warnings) {
        Ok(value) => Ok(Some(value)),
        Err(ReportError::TableNotFound { table }) if !config.mode.is_strict() => {
            warnings.push(ParseWarning::MissingTable { section, table });
            Ok(None)
        }
        Err(err) => Err(err),
    }
}
