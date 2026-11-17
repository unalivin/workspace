#![no_main]

use app_core::git;
use libfuzzer_sys::fuzz_target;

const LIMIT: u64 = 1 << 16;

fuzz_target!(|data: &[u8]| {
    let split = usize::from(data.first().copied().unwrap_or(0)).min(data.len().saturating_sub(1));
    let (base, delta) = data.get(1..).unwrap_or_default().split_at(split);
    match git::apply_delta(base, delta, LIMIT) {
        Ok(result) => {
            assert!(result.len() as u64 <= LIMIT);
            assert_eq!(git::delta_result_size(delta).ok(), Some(result.len() as u64));
        }
        Err(error) => assert!(error.to_string().starts_with("the Git data is damaged: ")),
    }
});
