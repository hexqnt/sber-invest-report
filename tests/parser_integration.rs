use sber_invest_report::{Report, ReportBuilder, ReportSet};

fn load_fixture(name: &str) -> Report {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    let html = std::fs::read_to_string(path).expect("read fixture");
    let raw = sber_invest_report::RawReport::from_str(&html);
    ReportBuilder::new(&raw).parse().expect("parse fixture")
}

#[test]
fn parses_broker_fixture() {
    let report = load_fixture("broker_report.html");
    assert_eq!(report.meta.account_id.0, "100ABC");
    assert!(report.asset_valuation.is_some());
    assert!(report.portfolio.is_some());
    let portfolio = report.portfolio.as_ref().unwrap();
    assert_eq!(portfolio.markets.len(), 1);
    assert_eq!(portfolio.markets[0].positions.len(), 1);
    let cash = report.cash_flow_summary.as_ref().unwrap();
    assert_eq!(cash.rows.len(), 3);
}

#[test]
fn parses_iis_fixture() {
    let report = load_fixture("iis_report.html");
    assert_eq!(report.meta.account_id.0, "I000XYZ");
    assert_eq!(report.meta.investor_name, "Петр Петров");
    assert_eq!(
        report.meta.account_kind,
        sber_invest_report::AccountKind::Iis
    );
    let iis = report.iis_contributions.as_ref().unwrap();
    assert_eq!(iis.rows.len(), 2);
    assert_eq!(iis.rows[0].limit_rub, sber_invest_report::Money::ZERO);
}

#[test]
fn parses_prod_fixture() {
    let report = load_fixture("prod_data.html");
    assert_eq!(report.meta.investor_name, "Иванов Иван Иванович");
    assert_eq!(
        report.meta.account_kind,
        sber_invest_report::AccountKind::Iis
    );
    assert!(report.portfolio.is_some());
    assert_eq!(
        report.meta.account_kind,
        sber_invest_report::AccountKind::Iis
    );
    
    let markets = report.portfolio.unwrap().markets;
    assert_eq!(markets.len(), 1);
    assert_eq!(markets[0].positions.len(), 3);
}

#[test]
fn parse_real_dir_if_present() {
    if let Ok(dir) = std::env::var("REAL_REPORT_DIR") {
        let set = ReportSet::from_dir(&dir).expect("parse real reports");
        assert!(!set.reports.is_empty());
    }
}
