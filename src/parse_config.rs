//! Конфигурация парсинга отчёта: выбор секций и строгость обработки.

/// Секция отчёта, которую можно включить или отключить при парсинге.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ReportSection {
    /// Таблица «Оценка активов, руб.».
    AssetValuation = 0,
    /// Сводка движения денежных средств.
    CashFlowSummary = 1,
    /// Портфель ценных бумаг.
    Portfolio = 2,
    /// Таблица пополнений ИИС.
    IisContributions = 3,
}

impl ReportSection {
    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// Набор секций, включаемых в парсинг.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionSet(u8);

impl SectionSet {
    const ALL_BITS: u8 = ReportSection::AssetValuation.bit()
        | ReportSection::CashFlowSummary.bit()
        | ReportSection::Portfolio.bit()
        | ReportSection::IisContributions.bit();

    /// Включает все известные секции отчёта.
    #[must_use]
    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }

    /// Оставляет только метаданные без таблиц.
    #[must_use]
    pub const fn meta_only() -> Self {
        Self(0)
    }

    /// Проверяет, включена ли секция.
    #[must_use]
    pub const fn contains(self, section: ReportSection) -> bool {
        self.0 & section.bit() != 0
    }

    /// Возвращает новый набор с добавленной секцией.
    #[must_use]
    pub const fn with(mut self, section: ReportSection) -> Self {
        self.0 |= section.bit();
        self
    }

    /// Возвращает новый набор с удалённой секцией.
    #[must_use]
    pub const fn without(mut self, section: ReportSection) -> Self {
        self.0 &= !section.bit();
        self
    }
}

impl Default for SectionSet {
    fn default() -> Self {
        Self::all()
    }
}

/// Режим парсинга: мягкий или строгий.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParseMode {
    /// Мягкий режим: часть структурных проблем пропускается.
    #[default]
    Lenient,
    /// Строгий режим: отсутствующие таблицы и битые строки считаются ошибкой.
    Strict,
}

impl ParseMode {
    /// Возвращает `true` для строгого режима.
    #[must_use]
    pub const fn is_strict(self) -> bool {
        matches!(self, Self::Strict)
    }
}

/// Параметры парсинга одного отчёта.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseConfig {
    /// Режим обработки ошибок структуры.
    pub mode: ParseMode,
    /// Какие секции нужно загружать.
    pub sections: SectionSet,
}

impl ParseConfig {
    /// Конфигурация по умолчанию: все секции в мягком режиме.
    #[must_use]
    pub const fn lenient() -> Self {
        Self {
            mode: ParseMode::Lenient,
            sections: SectionSet::all(),
        }
    }

    /// Строгая конфигурация: все секции должны быть корректно распарсены.
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            mode: ParseMode::Strict,
            sections: SectionSet::all(),
        }
    }

    /// Устанавливает режим парсинга.
    #[must_use]
    pub const fn with_mode(mut self, mode: ParseMode) -> Self {
        self.mode = mode;
        self
    }

    /// Полностью заменяет набор секций.
    #[must_use]
    pub const fn with_sections(mut self, sections: SectionSet) -> Self {
        self.sections = sections;
        self
    }

    /// Добавляет секцию в набор.
    #[must_use]
    pub const fn include(mut self, section: ReportSection) -> Self {
        self.sections = self.sections.with(section);
        self
    }

    /// Удаляет секцию из набора.
    #[must_use]
    pub const fn exclude(mut self, section: ReportSection) -> Self {
        self.sections = self.sections.without(section);
        self
    }

    /// Проверяет, включена ли секция.
    #[must_use]
    pub const fn loads(self, section: ReportSection) -> bool {
        self.sections.contains(section)
    }
}

impl Default for ParseConfig {
    fn default() -> Self {
        Self::lenient()
    }
}
