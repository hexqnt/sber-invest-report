//! Диагностика мягкого парсинга: предупреждения, не приводящие к ошибке.

use crate::parse_config::ReportSection;

/// Предупреждение парсинга, которое фиксируется в мягком режиме.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseWarning {
    /// Запрошенная таблица отсутствует в отчёте.
    MissingTable {
        /// Логическая секция, к которой относится таблица.
        section: ReportSection,
        /// Техническое имя таблицы.
        table: &'static str,
    },
    /// У строки таблицы недостаточно ячеек для ожидаемого формата.
    MalformedRow {
        /// Имя таблицы.
        table: &'static str,
        /// Индекс строки внутри таблицы (0-based).
        row_index: usize,
        /// Минимально ожидаемое количество ячеек.
        expected_cells: usize,
        /// Фактическое количество ячеек.
        actual_cells: usize,
    },
}

impl ParseWarning {
    /// Возвращает имя таблицы, к которой относится предупреждение.
    #[must_use]
    pub const fn table(&self) -> &'static str {
        match self {
            Self::MissingTable { table, .. } | Self::MalformedRow { table, .. } => table,
        }
    }
}
