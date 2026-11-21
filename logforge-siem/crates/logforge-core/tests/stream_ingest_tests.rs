use logforge_core::{Database, SiemService};
use std::io::Cursor;

#[test]
fn streams_large_files_in_bounded_batches_without_loading_all_rows() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("stream", "file", "memory").unwrap();
    let mut input = String::new();
    for index in 0..=logforge_core::service::MAX_BATCH_SIZE {
        input.push_str(&format!("2026-11-21T10:00:00Z host app: event-{index}\n"));
    }
    let result = service.ingest_reader(source, Cursor::new(input)).unwrap();
    assert_eq!(result.received, logforge_core::service::MAX_BATCH_SIZE + 1);
    assert_eq!(result.malformed, 0);
    assert_eq!(
        service.overview_counts().unwrap().total_events,
        result.received as i64
    );
}

#[test]
fn streaming_ingestion_accepts_explicit_format_selection() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("cef", "file", "memory").unwrap();
    let record = "CEF:0|Vendor|Product|1.0|login|Blocked login|7|src=192.0.2.4\n";
    let result = service
        .ingest_reader_with_format(
            source,
            Cursor::new(record),
            logforge_core::parser_suite::LogFormat::Cef,
        )
        .unwrap();
    assert_eq!(result.received, 1);
    assert_eq!(service.search_events("Blocked", 10).unwrap()[0].severity, "error");
}

#[test]
fn streaming_bounds_oversized_records_and_reports_io_errors() {
    let db = Database::open_in_memory().unwrap();
    let service = SiemService::new(db);
    let source = service.add_source("bounded", "file", "memory").unwrap();
    let line = format!(
        "2026-11-21T10:00:00Z host app: {}\n",
        "x".repeat(logforge_core::ingest::MAX_LINE_BYTES + 100)
    );
    let result = service.ingest_reader(source, Cursor::new(line)).unwrap();
    assert_eq!(result.malformed, 1);

    struct BrokenReader;
    impl std::io::Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("simulated read failure"))
        }
    }
    impl std::io::BufRead for BrokenReader {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Err(std::io::Error::other("simulated read failure"))
        }
        fn consume(&mut self, _: usize) {}
    }
    assert!(service.ingest_reader(source, BrokenReader).is_err());
}
