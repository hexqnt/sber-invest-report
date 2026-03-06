#![warn(missing_docs)]
//! Библиотека для парсинга HTML-отчётов брокера Сбербанка и их агрегации.

mod diagnostics;
mod error;
mod parse_config;
mod parser;
pub mod prelude;
mod raw;
mod report;
mod report_set;
mod types;
mod utils;

pub use crate::diagnostics::ParseWarning;
pub use crate::error::ReportError;
pub use crate::parse_config::{ParseConfig, ParseMode, ReportSection, SectionSet};
pub use crate::raw::{DomReport, RawReport};
pub use crate::report::{Report, ReportBuilder};
pub use crate::report_set::ReportSet;
pub use crate::types::*;
