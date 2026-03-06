use rust_decimal::Decimal;
use sber_invest_report::{
    CashFlowKind, CashFlowRow, CashFlowSummary, IisLimit, ParseConfig, ParseWarning, Report,
    ReportBuilder, ReportError, ReportSection, ReportSet, SectionSet,
};

fn load_fixture(name: &str) -> Report {
    let raw = load_raw_fixture(name);
    ReportBuilder::new(&raw).parse().expect("parse fixture")
}

fn load_raw_fixture(name: &str) -> sber_invest_report::RawReport {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    let html = std::fs::read_to_string(path).expect("read fixture");
    sber_invest_report::RawReport::from_html(&html)
}

#[test]
fn parses_broker_fixture() {
    let report = load_fixture("broker_report.html");
    assert_eq!(report.meta().account_id.0, "100ABC");
    assert!(report.asset_valuation().is_some());
    assert!(report.portfolio().is_some());
    let portfolio = report.portfolio().unwrap();
    assert_eq!(portfolio.markets().len(), 1);
    assert_eq!(portfolio.markets()[0].positions().len(), 1);
    let cash = report.cash_flow_summary().unwrap();
    assert_eq!(cash.rows().len(), 3);
}

#[test]
fn parses_iis_fixture() {
    let report = load_fixture("iis_report.html");
    assert_eq!(report.meta().account_id.0, "I000XYZ");
    assert_eq!(report.meta().investor_name, "Петр Петров");
    assert_eq!(
        report.meta().account_kind,
        sber_invest_report::AccountKind::Iis
    );
    let iis = report.iis_contributions().unwrap();
    assert_eq!(iis.rows().len(), 2);
    assert!(matches!(iis.rows()[0].limit_rub, IisLimit::Unlimited));
}

#[test]
fn parses_prod_fixture() {
    let report = load_fixture("prod_data.html");
    assert_eq!(report.meta().investor_name, "Иванов Иван Иванович");
    assert_eq!(
        report.meta().account_kind,
        sber_invest_report::AccountKind::Iis
    );
    assert!(report.portfolio().is_some());
    assert_eq!(
        report.meta().account_kind,
        sber_invest_report::AccountKind::Iis
    );

    let markets = report.portfolio().unwrap().markets();
    assert_eq!(markets.len(), 1);
    assert_eq!(markets[0].positions().len(), 3);
}

#[test]
fn parse_real_dir_if_present() {
    if let Ok(dir) = std::env::var("REAL_REPORT_DIR") {
        let set = ReportSet::from_dir(&dir).expect("parse real reports");
        assert!(!set.is_empty());
    }
}

#[test]
fn strict_mode_fails_when_requested_table_is_missing() {
    let raw = load_raw_fixture("broker_report.html");
    let config = ParseConfig::strict()
        .with_sections(SectionSet::meta_only().with(ReportSection::IisContributions));
    let err = Report::parse_with_config(&raw, config).expect_err("expected missing IIS table");
    assert!(matches!(
        err,
        ReportError::TableNotFound {
            table: "IISContributions"
        }
    ));
}

#[test]
fn merge_cash_flows_keeps_distinct_unknown_descriptions() {
    let report_a = load_fixture("broker_report.html").with_cash_flow_summary(Some(
        CashFlowSummary::new(vec![CashFlowRow {
            kind: CashFlowKind::Unknown,
            description_raw: "Неизвестная строка A".to_string(),
            amount: Decimal::new(10, 0),
            currency: "RUB".to_string(),
        }]),
    ));
    let report_b = load_fixture("broker_report.html").with_cash_flow_summary(Some(
        CashFlowSummary::new(vec![CashFlowRow {
            kind: CashFlowKind::Unknown,
            description_raw: "Неизвестная строка B".to_string(),
            amount: Decimal::new(20, 0),
            currency: "RUB".to_string(),
        }]),
    ));

    let merged = ReportSet::new(vec![report_a, report_b]).merge_cash_flows();

    assert_eq!(merged.rows().len(), 2);
    assert!(
        merged
            .rows()
            .iter()
            .any(|row| row.description_raw == "Неизвестная строка A")
    );
    assert!(
        merged
            .rows()
            .iter()
            .any(|row| row.description_raw == "Неизвестная строка B")
    );
}

#[test]
fn report_set_zero_copy_iterators_work() {
    let set = ReportSet::new(vec![
        load_fixture("broker_report.html"),
        load_fixture("prod_data.html"),
    ]);
    assert_eq!(set.iter_reports().count(), set.len());
    assert!(set.iter_positions().count() > 0);
    assert!(set.iter_cash_flows().count() > 0);
}

#[test]
fn from_dir_strict_works_for_meta_only_mode() {
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let set = ReportSet::from_dir_with_config(
        fixture_dir,
        ParseConfig::strict().with_sections(SectionSet::meta_only()),
    )
    .expect("parse fixtures in strict meta-only mode");

    assert_eq!(set.len(), 3);
    assert!(
        set.iter_reports()
            .all(|report| report.asset_valuation().is_none()
                && report.cash_flow_summary().is_none()
                && report.portfolio().is_none()
                && report.iis_contributions().is_none())
    );
}

#[test]
fn parse_with_diagnostics_reports_missing_optional_table() {
    let raw = load_raw_fixture("broker_report.html");
    let (report, warnings) = Report::parse_with_diagnostics(&raw, ParseConfig::default())
        .expect("parse with diagnostics");

    assert!(report.iis_contributions().is_none());
    assert!(warnings.iter().any(|warning| matches!(
        warning,
        ParseWarning::MissingTable {
            section: ReportSection::IisContributions,
            table: "IISContributions",
        }
    )));
}
