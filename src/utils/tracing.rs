use std::sync::Once;

use tracing_subscriber::fmt::format::FmtSpan;

/// Initializes a test-friendly tracing subscriber once per process.
///
/// The subscriber writes through the test harness, omits timestamps and
/// targets, and enables TRACE-level output for opt-in failure diagnostics.
pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_test_writer()
            .with_target(false)
            .with_span_events(FmtSpan::NONE)
            .without_time()
            .with_max_level(tracing::Level::TRACE)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
}
