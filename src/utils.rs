//! Вспомогательные парсеры чисел, дат и поиск элементов в HTML.

use crate::error::ReportError;
use crate::types::Money;
use chrono::NaiveDate;
use regex::Regex;
use rust_decimal::Decimal;
use scraper::{ElementRef, Html, Selector};
use std::str::FromStr;
use std::sync::LazyLock;

/// Нормализует последовательность символов, схлопывая группы пробельных символов.
fn normalize_chars<I: IntoIterator<Item = char>>(iter: I) -> String {
    let mut output = String::new();
    let mut pending_space = false;
    for ch in iter {
        if ch.is_whitespace() {
            if !output.is_empty() {
                pending_space = true;
            }
            continue;
        }

        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(ch);
    }
    output
}

/// Нормализует числовую строку, удаляя пробелы, NBSP/NNBSP и плюс.
fn normalize_number(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !(ch.is_whitespace() || matches!(*ch, '\u{a0}' | '\u{202f}' | '+')))
        .collect()
}

/// Разбирает денежное значение, трактуя пустую ячейку как ноль.
pub fn parse_money_or_zero(value: &str, column: &'static str) -> Result<Money, ReportError> {
    let normalized = normalize_number(value);
    if normalized.is_empty() {
        return Ok(Decimal::ZERO);
    }
    Decimal::from_str(&normalized).map_err(|_| ReportError::Number {
        value: value.trim().to_string(),
        column,
    })
}

/// Разбирает дату в формате `dd.mm.yyyy`.
pub fn parse_date(value: &str) -> Result<NaiveDate, ReportError> {
    NaiveDate::parse_from_str(value.trim(), "%d.%m.%Y").map_err(|_| ReportError::Date {
        value: value.trim().to_string(),
    })
}

/// Собирает текст всех потомков элемента и нормализует пробелы.
pub fn collect_text(element: ElementRef) -> String {
    normalize_chars(element.text().flat_map(|s| s.chars()))
}

static TABLE_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("table").expect("valid table selector"));
static ROW_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("tr").expect("valid tr selector"));
static HEADER_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("td, th").expect("valid header selector"));

/// Ищет таблицу, в которой одна из первых строк заголовка содержит все требуемые фразы.
///
/// Сравнение выполняется по подстроке (`contains`), а по умолчанию проверяется только первая строка
/// заголовка (`custom_header_depth = 1`).
pub fn find_table_with_headers<'a>(
    doc: &'a Html,
    required_headers: &[&str],
    custom_header_depth: Option<u8>,
) -> Option<ElementRef<'a>> {
    let header_depth = custom_header_depth.unwrap_or(1);

    for table in doc.select(&TABLE_SELECTOR) {
        let mut rows = table.select(&ROW_SELECTOR);
        for _ in 0..header_depth {
            if let Some(header_row) = rows.next() {
                let headers: Vec<String> = header_row
                    .select(&HEADER_SELECTOR)
                    .map(collect_text)
                    .collect();
                let matches = required_headers
                    .iter()
                    .all(|target| headers.iter().any(|h| h.contains(target)));
                if matches {
                    return Some(table);
                }
            }
        }
    }
    None
}

/// Находит первый фрагмент текста, совпадающий с регулярным выражением.
///
/// Возвращает срез первой захватывающей группы (группа `1`).
pub fn capture_text<'a>(text: &'a str, pattern: &Regex) -> Option<&'a str> {
    pattern
        .captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
}

/// Делает первую букву каждого слова заглавной, а остальные — строчными (используется для ФИО).
pub fn capitalize_words(s: &str) -> String {
    let mut normalized = String::new();
    for word in s.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }

        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            normalized.extend(first.to_uppercase());
            normalized.push_str(&chars.as_str().to_lowercase());
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_chars_trims_and_collapses_whitespace() {
        let text = normalize_chars(" \n Иванов \t Иван  \u{a0}Иванович ".chars());
        assert_eq!(text, "Иванов Иван Иванович");
    }

    #[test]
    fn normalize_number_removes_spaces_and_plus() {
        let normalized = normalize_number(" +1 234\u{a0}567\u{202f}.89 ");
        assert_eq!(normalized, "1234567.89");
    }

    #[test]
    fn capture_text_returns_first_group() {
        let re = Regex::new(r"Инвестор:\s*(.+)").expect("valid regex");
        let text = "Инвестор: Иванов Иван Иванович";
        let captured = capture_text(text, &re).expect("must capture investor");
        assert_eq!(captured, "Иванов Иван Иванович");
    }

    #[test]
    fn capitalize_words_normalizes_case() {
        assert_eq!(
            capitalize_words("иВАНОВ иВАН иВАНОВИЧ"),
            "Иванов Иван Иванович"
        );
    }
}
