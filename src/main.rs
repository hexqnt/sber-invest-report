//! Пример CLI: читает HTML-отчёт и выводит метаданные периода.

use std::env;
use std::fs::File;

use sber_invest_report::{RawReport, ReportBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = if let Some(path) = env::args().nth(1) {
        path
    } else {
        println!("Usage: sber-invest-report <path-to-report.html>");
        return Ok(());
    };

    let raw = RawReport::from_reader(File::open(&path)?)?;
    let report = ReportBuilder::new(&raw).parse()?;

    println!(
        "Счёт: {}, период {} — {}",
        report.meta.account_id.0, report.meta.period_start, report.meta.period_end
    );
    println!("Инвестор: {}", report.meta.investor_name);
    println!("Договор: {}", report.meta.contract_number);
    if let Some(av) = &report.asset_valuation {
        println!(
            "Оценка активов: {} строк, итоговое изменение {}",
            av.rows.len(),
            av.total_delta
        );
    }
    if let Some(portfolio) = &report.portfolio {
        let positions: usize = portfolio.markets.iter().map(|m| m.positions.len()).sum();
        println!(
            "Портфель: {} площадок, {} позиций",
            portfolio.markets.len(),
            positions
        );
    }
    if let Some(cash) = &report.cash_flow_summary {
        let total: sber_invest_report::Money = cash.rows.iter().map(|r| r.amount).sum();
        println!("Движение ДС: {} строк, сумма {}", cash.rows.len(), total);
    }
    if let Some(iis) = &report.iis_contributions {
        println!("Взносы на ИИС: {} записей", iis.rows.len());
    }
    Ok(())
}
