//! Вспомогательные парсеры чисел, дат и поиск элементов в HTML.

use crate::error::ReportError;
use crate::types::Money;
use chrono::NaiveDate;
use regex::Regex;
use rust_decimal::Decimal;
use scraper::{ElementRef, Html, Selector};
use std::str::FromStr;
use std::sync::LazyLock;

/// Нормализует последовательность символов, схлопывая группы пробельных.
fn normalize_chars<I: IntoIterator<Item = char>>(iter: I) -> String {
    let mut output = String::new();
    let mut prev_space = false;
    for ch in iter {
        let is_space = ch.is_whitespace();
        if is_space {
            if !prev_space {
                output.push(' ');
            }
        } else {
            output.push(ch);
        }
        prev_space = is_space;
    }
    output.trim().to_string()
}

/// Нормализует числовую строку, удаляя пробелы, знак плюса итд.
fn normalize_number(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !matches!(*ch, ' ' | '\u{a0}' | '\u{202f}' | '+'))
        .collect::<String>()
        .trim()
        .to_string()
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

/// Ищет таблицу, чьи заголовки содержат все требуемые фразы.
/// По умолчанию глубина заголовка 1
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
pub fn capture_text(text: &str, pattern: &Regex) -> Option<String> {
    pattern
        .captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

// Перевести первую букву каждого слова в верхний регистр (для ФИО)
pub fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}
