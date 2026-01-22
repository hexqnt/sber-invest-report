//! Работа с исходным HTML и подготовленным DOM-деревом.

use crate::error::ReportError;
use scraper::Html;
use std::io::Read;

/// Исходный HTML отчёта без разбора DOM.
#[derive(Debug, Clone)]
pub struct RawReport {
    /// Полный HTML отчёта.
    pub html: String,
}

impl RawReport {
    /// Читает HTML-отчёт из произвольного `Read`.
    pub fn from_reader<R: Read>(mut reader: R) -> Result<Self, ReportError> {
        let mut html = String::new();
        reader.read_to_string(&mut html)?;
        Ok(Self { html })
    }

    /// Создаёт отчёт из готовой HTML-строки.
    #[inline]
    pub fn from_str(s: &str) -> Self {
        Self {
            html: s.to_string(),
        }
    }
}

/// Разобранный DOM отчёта с удобными методами поиска таблиц.
#[derive(Debug, Clone)]
pub struct DomReport {
    pub(crate) doc: Html,
}

impl DomReport {
    /// Парсит DOM из исходного HTML.
    #[inline]
    pub fn parse(raw: &RawReport) -> Result<Self, ReportError> {
        Ok(Self {
            doc: Html::parse_document(&raw.html),
        })
    }
}
