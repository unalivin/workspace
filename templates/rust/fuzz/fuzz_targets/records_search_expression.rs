#![no_main]

use app_core::records;
use libfuzzer_sys::fuzz_target;
use rusqlite::Connection;

thread_local! {
    static PROBE: Connection = {
        let connection = Connection::open_in_memory().expect("an in-memory database opens");
        connection
            .execute_batch("CREATE VIRTUAL TABLE probe USING fts5 (body); INSERT INTO probe (body) VALUES ('sample text');")
            .expect("the probe table is created");
        connection
    };
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let expression = records::search_expression(text);
    assert!(expression.split(' ').filter(|term| !term.is_empty()).count() <= 16);
    if expression.is_empty() {
        return;
    }
    PROBE.with(|connection| {
        let matched: rusqlite::Result<i64> =
            connection.query_row("SELECT count(*) FROM probe WHERE probe MATCH ?1", [&expression], |row| row.get(0));
        assert!(matched.is_ok(), "{expression}: {matched:?}");
    });
});
