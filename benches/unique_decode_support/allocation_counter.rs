// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Per-operation requested-allocation accounting for this bench binary only.

use std::alloc::GlobalAlloc;
use std::alloc::Layout;
use std::alloc::System;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

struct CountingSystem;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static REQUESTED_BYTES: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

fn add_live(bytes: usize) {
    let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    if ACTIVE.load(Ordering::Relaxed) {
        PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
    }
}

fn remove_live(bytes: usize) {
    LIVE_BYTES.fetch_sub(bytes, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add_live(layout.size());
            if ACTIVE.load(Ordering::Relaxed) {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                REQUESTED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            }
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            add_live(layout.size());
            if ACTIVE.load(Ordering::Relaxed) {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                REQUESTED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            }
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
        remove_live(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            if new_size >= layout.size() {
                add_live(new_size - layout.size());
            } else {
                remove_live(layout.size() - new_size);
            }
            if ACTIVE.load(Ordering::Relaxed) {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                REQUESTED_BYTES.fetch_add(new_size, Ordering::Relaxed);
                REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            }
        }
        resized
    }
}

#[global_allocator]
static ALLOCATOR: CountingSystem = CountingSystem;

pub(crate) struct AllocationMeasurement {
    pub(crate) allocation_count: usize,
    pub(crate) allocated_request_bytes: usize,
    pub(crate) reallocation_count: usize,
    #[allow(dead_code)]
    pub(crate) baseline_live_bytes: usize,
    #[allow(dead_code)]
    pub(crate) peak_live_bytes: usize,
    #[allow(dead_code)]
    pub(crate) end_live_bytes: usize,
    #[allow(dead_code)]
    pub(crate) peak_extra_bytes: usize,
}

pub(crate) fn measure<T>(operation: impl FnOnce() -> T) -> (T, AllocationMeasurement) {
    ACTIVE.store(false, Ordering::SeqCst);
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    REQUESTED_BYTES.store(0, Ordering::Relaxed);
    REALLOCATIONS.store(0, Ordering::Relaxed);
    let baseline_live_bytes = LIVE_BYTES.load(Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(baseline_live_bytes, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::SeqCst);
    let guard = MeasurementGuard;
    let result = operation();
    drop(guard);
    let end_live_bytes = LIVE_BYTES.load(Ordering::Relaxed);
    let peak_live_bytes = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
    let measurement = AllocationMeasurement {
        allocation_count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_request_bytes: REQUESTED_BYTES.load(Ordering::Relaxed),
        reallocation_count: REALLOCATIONS.load(Ordering::Relaxed),
        baseline_live_bytes,
        peak_live_bytes,
        end_live_bytes,
        peak_extra_bytes: peak_live_bytes.saturating_sub(baseline_live_bytes),
    };
    (result, measurement)
}

#[allow(dead_code)]
pub(crate) fn self_check() {
    let layout_64 = Layout::from_size_align(64, 8).expect("valid probe layout");
    let ((), allocated) = measure(|| unsafe {
        let pointer = ALLOCATOR.alloc(layout_64);
        assert!(!pointer.is_null(), "64-byte allocation probe must succeed");
        ALLOCATOR.dealloc(pointer, layout_64);
    });
    assert_eq!(allocated.peak_extra_bytes, 64);
    assert_eq!(allocated.end_live_bytes, allocated.baseline_live_bytes);

    let pointer = unsafe { ALLOCATOR.alloc(layout_64) };
    assert!(!pointer.is_null(), "cross-window allocation probe must succeed");
    let ((), released) = measure(|| unsafe { ALLOCATOR.dealloc(pointer, layout_64) });
    assert_eq!(released.end_live_bytes, released.baseline_live_bytes - 64);
    assert_eq!(released.peak_extra_bytes, 0);

    let pointer = unsafe { ALLOCATOR.alloc(layout_64) };
    assert!(!pointer.is_null(), "realloc growth probe allocation must succeed");
    let (resized, grown) = measure(|| unsafe { ALLOCATOR.realloc(pointer, layout_64, 128) });
    assert!(!resized.is_null(), "realloc growth probe must succeed");
    assert_eq!(grown.end_live_bytes, grown.baseline_live_bytes + 64);
    assert_eq!(grown.peak_extra_bytes, 64);
    assert_eq!(grown.allocated_request_bytes, 128);
    assert_eq!(grown.reallocation_count, 1);
    unsafe { ALLOCATOR.dealloc(resized, Layout::from_size_align(128, 8).expect("valid resized layout")) };

    let layout_128 = Layout::from_size_align(128, 8).expect("valid probe layout");
    let pointer = unsafe { ALLOCATOR.alloc(layout_128) };
    assert!(!pointer.is_null(), "realloc shrink probe allocation must succeed");
    let (resized, shrunk) = measure(|| unsafe { ALLOCATOR.realloc(pointer, layout_128, 64) });
    assert!(!resized.is_null(), "realloc shrink probe must succeed");
    assert_eq!(shrunk.end_live_bytes, shrunk.baseline_live_bytes - 64);
    assert_eq!(shrunk.peak_extra_bytes, 0);
    unsafe { ALLOCATOR.dealloc(resized, layout_64) };
}

struct MeasurementGuard;

impl Drop for MeasurementGuard {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::SeqCst);
    }
}
