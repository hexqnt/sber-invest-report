//! Ошибки парсинга и агрегации брокерских отчётов.

/// Ошибка разбора или агрегации брокерских отчётов.
#[derive(thiserror::Error, Debug)]
pub enum ReportError {
    /// Ошибка ввода-вывода при чтении исходного файла.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Ошибка парсинга HTML.
    #[error("HTML parsing error: {0}")]
    Html(String),
    /// В отчёте не удалось найти ожидаемую таблицу.
    #[error("Table '{table}' not found")]
    TableNotFound {
        /// Имя таблицы.
        table: &'static str,
    },
    /// Ошибка разбора числового значения.
    #[error("Invalid number '{value}' in column '{column}'")]
    Number {
        /// Некорректное исходное значение.
        value: String,
        /// Название столбца.
        column: &'static str,
    },
    /// Ошибка разбора даты.
    #[error("Invalid date '{value}'")]
    Date {
        /// Некорректная дата.
        value: String,
    },
    /// В отчёте отсутствует обязательное поле.
    #[error("Required field '{field}' missing")]
    MissingField {
        /// Имя пропавшего поля.
        field: &'static str,
    },
    /// Не удалось сопоставить текст с ожидаемым форматом.
    #[error("Regex did not match: {0}")]
    Regex(String),
}
