#![warn(missing_docs)]
//! Библиотека для парсинга HTML-отчётов брокера Сбербанка и их агрегации.

mod error;
mod parser;
mod raw;
mod report;
mod report_set;
mod types;
mod utils;

pub use crate::error::ReportError;
pub use crate::raw::{DomReport, RawReport};
pub use crate::report::{Report, ReportBuilder};
pub use crate::report_set::ReportSet;
pub use crate::types::*;
