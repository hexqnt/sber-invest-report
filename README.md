# sber-invest-report

[![crates.io](https://img.shields.io/crates/v/sber-invest-report.svg)](https://crates.io/crates/sber-invest-report)
[![docs.rs](https://docs.rs/sber-invest-report/badge.svg)](https://docs.rs/sber-invest-report)

Парсер HTML-отчётов брокера Сбербанка и утилиты для агрегации данных отчётов.

## Возможности

- Парсинг метаданных (счёт, период, дата формирования, инвестор).
- Таблицы: оценка активов, сводка движения ДС, портфель, пополнения ИИС.
- Набор отчётов и агрегация (сводная ДС, суммирование позиций по ISIN).

## Установка

```sh
cargo add sber-invest-report
```

## Быстрый старт

### Парсинг одного отчёта

```rust
use sber_invest_report::{RawReport, ReportBuilder};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = RawReport::from_reader(File::open("report.html")?)?;
    let report = ReportBuilder::new(&raw).parse()?;
    println!("Счёт: {}", report.meta.account_id.0);
    Ok(())
}
```

### Загрузка каталога и агрегация

```rust
use sber_invest_report::ReportSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let set = ReportSet::from_dir("reports")?;
    let merged_cash = set.merge_cash_flows();
    let merged_positions = set.merge_positions();
    println!("Всего отчётов: {}", set.reports.len());
    println!("Движение ДС строк: {}", merged_cash.rows.len());
    println!("Позиции ISIN: {}", merged_positions.len());
    Ok(())
}
```

### Частичный парсинг через билдер

```rust
use sber_invest_report::{RawReport, ReportBuilder};
use std::fs::File;

let raw = RawReport::from_reader(File::open("report.html")?)?;
let report = ReportBuilder::new(&raw)
    .portfolio(true)
    .asset_valuation(false)
    .cash_flow(true)
    .iis_contributions(false)
    .parse()?;
```

## Тесты

- Фиктивные отчёты лежат в `tests/fixtures/` и используются в интеграционных тестах.
- Для локальной проверки реальных отчётов можно задать `REAL_REPORT_DIR=/path/to/reports cargo test`.

## Лицензия

MIT или Apache-2.0
