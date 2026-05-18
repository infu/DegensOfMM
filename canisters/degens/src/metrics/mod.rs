//! Metrics and cycle/storage measurement hooks for canister service paths.

use crate::dto::public::ApiError;

#[cfg(feature = "benchmark")]
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

#[cfg(feature = "benchmark")]
use crate::contract::{DiagnosticBenchmarkCallPage, DiagnosticBenchmarkCallView, EndpointKind};

#[cfg(feature = "benchmark")]
const MAX_BENCHMARK_CALLS: usize = 4096;

#[cfg(feature = "benchmark")]
thread_local! {
    static BENCHMARK_CALLS: RefCell<VecDeque<DiagnosticBenchmarkCallView>> =
        RefCell::new(VecDeque::new());
    static NEXT_BENCHMARK_SEQUENCE: Cell<u64> = const { Cell::new(1) };
}

#[cfg(feature = "benchmark")]
pub(crate) fn benchmark_update<T>(
    method: &'static str,
    body: impl FnOnce() -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let stable_memory_pages_before = canic_cdk::api::stable_size();
    let instruction_before = canic_cdk::api::instruction_counter();
    let result = body();
    let instruction_after = canic_cdk::api::instruction_counter();
    let stable_memory_pages_after = canic_cdk::api::stable_size();
    let error_code = result.as_ref().err().map(|error| error.code.clone());

    record_benchmark_call(DiagnosticBenchmarkCallView {
        sequence: next_sequence(),
        method: method.to_string(),
        kind: EndpointKind::Update,
        ok: result.is_ok(),
        error_code,
        instruction_delta: instruction_after.saturating_sub(instruction_before),
        stable_memory_pages_before,
        stable_memory_pages_after,
    });

    result
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn benchmark_update<T>(
    _method: &'static str,
    body: impl FnOnce() -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    body()
}

#[cfg(feature = "benchmark")]
pub(crate) fn reset_benchmark_metrics() {
    BENCHMARK_CALLS.with(|calls| calls.borrow_mut().clear());
    NEXT_BENCHMARK_SEQUENCE.with(|sequence| sequence.set(1));
}

#[cfg(feature = "benchmark")]
pub(crate) fn benchmark_metrics_page(
    cursor: Option<u64>,
    limit: u32,
) -> DiagnosticBenchmarkCallPage {
    let cursor = cursor.unwrap_or(0);
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);

    BENCHMARK_CALLS.with(|calls| {
        let calls = calls.borrow();
        let total_recorded = calls.back().map_or(0, |call| call.sequence);
        let mut page = Vec::new();
        let mut next_cursor = None;

        for call in calls.iter().filter(|call| call.sequence > cursor) {
            if page.len() >= limit {
                next_cursor = Some(call.sequence.saturating_sub(1));
                break;
            }
            page.push(call.clone());
        }

        DiagnosticBenchmarkCallPage {
            calls: page,
            next_cursor,
            total_recorded,
        }
    })
}

#[cfg(feature = "benchmark")]
fn next_sequence() -> u64 {
    NEXT_BENCHMARK_SEQUENCE.with(|sequence| {
        let next = sequence.get();
        sequence.set(next.saturating_add(1));
        next
    })
}

#[cfg(feature = "benchmark")]
fn record_benchmark_call(call: DiagnosticBenchmarkCallView) {
    BENCHMARK_CALLS.with(|calls| {
        let mut calls = calls.borrow_mut();
        if calls.len() >= MAX_BENCHMARK_CALLS {
            calls.pop_front();
        }
        calls.push_back(call);
    });
}
