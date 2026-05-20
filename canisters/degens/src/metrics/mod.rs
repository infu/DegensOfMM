//! Metrics and cycle/storage measurement hooks for canister service paths.

use crate::dto::public::ApiError;

#[cfg(feature = "benchmark")]
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, VecDeque},
};

#[cfg(feature = "benchmark")]
use crate::contract::{
    DiagnosticBenchmarkCallPage, DiagnosticBenchmarkCallView, DiagnosticBenchmarkRepoOpView,
    EndpointKind,
};

#[cfg(feature = "benchmark")]
const MAX_BENCHMARK_CALLS: usize = 4096;

#[cfg(feature = "benchmark")]
thread_local! {
    static BENCHMARK_CALLS: RefCell<VecDeque<DiagnosticBenchmarkCallView>> =
        RefCell::new(VecDeque::new());
    static CURRENT_BENCHMARK_REPO_OPS: RefCell<BTreeMap<&'static str, DiagnosticBenchmarkRepoOpView>> =
        RefCell::new(BTreeMap::new());
    static NEXT_BENCHMARK_SEQUENCE: Cell<u64> = const { Cell::new(1) };
}

#[cfg(feature = "benchmark")]
pub(crate) fn benchmark_update<T>(
    method: &'static str,
    body: impl FnOnce() -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    reset_current_call_details();
    let instruction_before = canic_cdk::api::instruction_counter();
    let result = body();
    let instruction_after = canic_cdk::api::instruction_counter();
    let repo_ops = take_current_repo_ops();

    record_benchmark_call(DiagnosticBenchmarkCallView {
        sequence: next_sequence(),
        method: method.to_string(),
        kind: EndpointKind::Update,
        instruction_delta: instruction_after.saturating_sub(instruction_before),
        repo_ops,
    });

    result
}

#[cfg(feature = "benchmark")]
pub(crate) fn benchmark_query<T>(
    method: &'static str,
    body: impl FnOnce() -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    reset_current_call_details();
    let stable_memory_pages_before = canic_cdk::api::stable_size();
    let instruction_before = canic_cdk::api::instruction_counter();
    let result = body();
    let instruction_after = canic_cdk::api::instruction_counter();
    let stable_memory_pages_after = canic_cdk::api::stable_size();
    let error_code = result
        .as_ref()
        .err()
        .map(|error| error.code.as_str())
        .unwrap_or("-");

    canic_cdk::eprintln!(
        "DOMM_BENCH_QUERY method={} ok={} error_code={} instruction_delta={} stable_pages_before={} stable_pages_after={}",
        method,
        result.is_ok(),
        error_code,
        instruction_after.saturating_sub(instruction_before),
        stable_memory_pages_before,
        stable_memory_pages_after
    );

    result
}

#[cfg(feature = "benchmark")]
pub(crate) fn benchmark_repo_operation<T>(operation: &'static str, body: impl FnOnce() -> T) -> T {
    let stable_memory_pages_before = canic_cdk::api::stable_size();
    let instruction_before = canic_cdk::api::instruction_counter();
    let result = body();
    let instruction_after = canic_cdk::api::instruction_counter();
    let stable_memory_pages_after = canic_cdk::api::stable_size();
    let instruction_delta = instruction_after.saturating_sub(instruction_before);
    let stable_memory_page_delta =
        signed_delta(stable_memory_pages_after, stable_memory_pages_before);

    CURRENT_BENCHMARK_REPO_OPS.with(|ops| {
        let mut ops = ops.borrow_mut();
        let op = ops
            .entry(operation)
            .or_insert_with(|| DiagnosticBenchmarkRepoOpView {
                operation: operation.to_string(),
                calls: 0,
                instruction_delta: 0,
                stable_memory_page_delta: 0,
            });
        op.calls = op.calls.saturating_add(1);
        op.instruction_delta = op.instruction_delta.saturating_add(instruction_delta);
        op.stable_memory_page_delta = op
            .stable_memory_page_delta
            .saturating_add(stable_memory_page_delta);
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

#[cfg(not(feature = "benchmark"))]
pub(crate) fn benchmark_query<T>(
    _method: &'static str,
    body: impl FnOnce() -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    body()
}

#[cfg(not(feature = "benchmark"))]
pub(crate) fn benchmark_repo_operation<T>(_operation: &'static str, body: impl FnOnce() -> T) -> T {
    body()
}

#[cfg(feature = "benchmark")]
pub(crate) fn reset_benchmark_metrics() {
    BENCHMARK_CALLS.with(|calls| calls.borrow_mut().clear());
    reset_current_call_details();
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
fn reset_current_call_details() {
    CURRENT_BENCHMARK_REPO_OPS.with(|ops| ops.borrow_mut().clear());
}

#[cfg(feature = "benchmark")]
fn take_current_repo_ops() -> Vec<DiagnosticBenchmarkRepoOpView> {
    CURRENT_BENCHMARK_REPO_OPS.with(|ops| {
        std::mem::take(&mut *ops.borrow_mut())
            .into_values()
            .collect()
    })
}

#[cfg(feature = "benchmark")]
fn signed_delta(after: u64, before: u64) -> i64 {
    if after >= before {
        i64::try_from(after - before).unwrap_or(i64::MAX)
    } else {
        -i64::try_from(before - after).unwrap_or(i64::MAX)
    }
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
