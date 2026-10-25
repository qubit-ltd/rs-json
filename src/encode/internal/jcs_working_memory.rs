// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Accounts for auxiliary capacities held by the JCS renderer.

use std::mem::size_of;

use super::jcs_reserve_error::JcsReserveError;
use crate::encode::JcsWorkingMemoryError;

/// Tracks live stack and object member-table capacities.
pub(super) struct JcsWorkingMemory {
    /// Maximum number of bytes that may be held by tracked capacities.
    limit: usize,
    /// Bytes currently held by the traversal stack.
    stack_bytes: usize,
    /// Bytes currently held by object member tables.
    object_bytes: usize,
    /// Whether object-table pressure disabled speculative stack growth for
    /// the rest of this render, avoiding repeated copies of deep ancestors.
    stack_growth_is_exact: bool,
    /// Largest combined tracked capacity observed so far.
    peak_bytes: usize,
    /// Successful stack growths observed by the allocation diagnostic.
    #[cfg(feature = "jcs-allocation-diagnostics")]
    stack_growth_count: usize,
    /// Successful exact stack replacements observed by the diagnostic.
    #[cfg(feature = "jcs-allocation-diagnostics")]
    stack_replacement_count: usize,
    /// Sum of requested stack capacities, including successful replacements.
    #[cfg(feature = "jcs-allocation-diagnostics")]
    stack_requested_bytes: usize,
    /// Largest actual stack capacity observed by the diagnostic.
    #[cfg(feature = "jcs-allocation-diagnostics")]
    stack_max_actual_bytes: usize,
    /// Successful object-table growths observed by the allocation diagnostic.
    #[cfg(feature = "jcs-allocation-diagnostics")]
    object_growth_count: usize,
    /// Sum of requested object-table capacities observed by the diagnostic.
    #[cfg(feature = "jcs-allocation-diagnostics")]
    object_requested_bytes: usize,
    /// Largest actual object-table capacity observed by the diagnostic.
    #[cfg(feature = "jcs-allocation-diagnostics")]
    object_max_actual_bytes: usize,
}

impl JcsWorkingMemory {
    /// Creates a tracker with the given capacity limit and no reserved memory.
    ///
    /// # Parameters
    ///
    /// - `limit`: Maximum combined byte capacity allowed for tracked
    ///   collections.
    ///
    /// # Returns
    ///
    /// A tracker initialized with zero live and peak capacity.
    #[inline]
    pub(super) const fn new(limit: usize) -> Self {
        Self {
            limit,
            stack_bytes: 0,
            object_bytes: 0,
            stack_growth_is_exact: false,
            peak_bytes: 0,
            #[cfg(feature = "jcs-allocation-diagnostics")]
            stack_growth_count: 0,
            #[cfg(feature = "jcs-allocation-diagnostics")]
            stack_replacement_count: 0,
            #[cfg(feature = "jcs-allocation-diagnostics")]
            stack_requested_bytes: 0,
            #[cfg(feature = "jcs-allocation-diagnostics")]
            stack_max_actual_bytes: 0,
            #[cfg(feature = "jcs-allocation-diagnostics")]
            object_growth_count: 0,
            #[cfg(feature = "jcs-allocation-diagnostics")]
            object_requested_bytes: 0,
            #[cfg(feature = "jcs-allocation-diagnostics")]
            object_max_actual_bytes: 0,
        }
    }

    /// Reserves enough capacity for additional traversal-stack values.
    ///
    /// # Parameters
    ///
    /// - `values`: Traversal stack whose capacity is tracked.
    /// - `additional`: Number of elements the stack must hold beyond its
    ///   current length.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the stack has enough capacity and its actual allocation
    /// has been added to the live and peak totals.
    ///
    /// # Errors
    ///
    /// Returns `JcsReserveError::Limit` if the resulting tracked capacity
    /// exceeds the configured limit, or `JcsReserveError::Allocation` if
    /// reservation fails.
    pub(super) fn reserve_stack<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), JcsReserveError> {
        self.reserve::<T, T>(values, additional, true, None)
    }

    /// Reserves enough capacity for additional entries in an object member
    /// table.
    ///
    /// # Parameters
    ///
    /// - `values`: Object member table whose capacity is tracked.
    /// - `additional`: Number of entries the table must hold beyond its current
    ///   length.
    /// - `stack`: Traversal stack whose unused capacity may be reclaimed if
    ///   needed to keep an otherwise acceptable member table within budget.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the member table has enough capacity and its actual
    /// allocation has been added to the live and peak totals.
    ///
    /// # Errors
    ///
    /// Returns `JcsReserveError::Limit` if the resulting tracked capacity
    /// exceeds the configured limit, or `JcsReserveError::Allocation` if
    /// reservation fails.
    pub(super) fn reserve_object_entries<T, S>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        stack: &mut Vec<S>,
    ) -> Result<(), JcsReserveError> {
        self.reserve(values, additional, false, Some(stack))
    }

    /// Removes an object's member-table capacity from the tracked live total.
    ///
    /// # Parameters
    ///
    /// - `values`: Object member table whose capacity is no longer live.
    ///
    /// # Returns
    ///
    /// `Ok(())` after removing the vector's capacity from the live object
    /// total.
    ///
    /// # Errors
    ///
    /// The caller must invoke this before dropping the corresponding vector.
    /// Returns a limit error if its capacity cannot be represented or is
    /// not currently tracked.
    pub(super) fn release_object_entries<T>(&mut self, values: &Vec<T>) -> Result<(), JcsReserveError> {
        let bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        self.object_bytes = self
            .object_bytes
            .checked_sub(bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        Ok(())
    }

    /// Prints one renderer capacity report when the dedicated diagnostic is
    /// enabled.
    #[cfg(feature = "jcs-allocation-diagnostics")]
    pub(super) fn report_diagnostics(&self, success: bool) {
        if std::env::var_os("JCS_RENDERER_ALLOCATION_DIAGNOSTICS").is_some() {
            eprintln!(
                "JCS_RENDERER status={} stack_growths={} stack_replacements={} stack_requested_bytes={} stack_max_actual_bytes={} object_growths={} object_requested_bytes={} object_max_actual_bytes={}",
                if success { "success" } else { "failure" },
                self.stack_growth_count,
                self.stack_replacement_count,
                self.stack_requested_bytes,
                self.stack_max_actual_bytes,
                self.object_growth_count,
                self.object_requested_bytes,
                self.object_max_actual_bytes,
            );
        }
    }

    #[cfg(test)]
    fn peak_bytes(&self) -> usize {
        self.peak_bytes
    }

    /// Reserves capacity and updates the tracked bytes for the selected
    /// collection.
    ///
    /// # Parameters
    ///
    /// - `values`: Collection to reserve and account for.
    /// - `additional`: Number of elements required beyond the current length.
    /// - `is_stack`: Whether the collection contributes to stack or object
    ///   bytes.
    /// - `stack`: Stack available for reclaiming spare capacity when reserving
    ///   an object table; absent when reserving the stack itself.
    ///
    /// # Returns
    ///
    /// `Ok(())` after capacity is sufficient and the live and peak totals have
    /// been updated.
    ///
    /// # Errors
    ///
    /// The old capacity is excluded while checking the replacement allocation.
    /// Returns a limit error on arithmetic overflow or when the requested or
    /// actual capacity exceeds the configured limit, and an allocation
    /// error if reserving memory fails.
    fn reserve<T, S>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        is_stack: bool,
        mut stack: Option<&mut Vec<S>>,
    ) -> Result<(), JcsReserveError> {
        let required_capacity = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        if required_capacity <= values.capacity() {
            return Ok(());
        }

        let old_bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        let current = self
            .stack_bytes
            .checked_add(self.object_bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        let mut without_old = current
            .checked_sub(old_bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        let target_capacity = if is_stack && !self.stack_growth_is_exact {
            let preferred = values.capacity().max(1).saturating_mul(2).max(required_capacity);
            let preferred = if self.limit == usize::MAX {
                preferred
            } else {
                // Use the remaining budget for the last growth of a tight
                // non-power-of-two chain instead of growing one slot at a time.
                let affordable = self
                    .limit
                    .saturating_sub(without_old)
                    .checked_div(size_of::<T>())
                    .unwrap_or(usize::MAX);
                preferred.min(affordable).max(required_capacity)
            };
            if preferred
                .checked_mul(size_of::<T>())
                .filter(|bytes| *bytes <= isize::MAX as usize)
                .and_then(|bytes| without_old.checked_add(bytes))
                .is_some_and(|total| total <= self.limit)
            {
                preferred
            } else {
                required_capacity
            }
        } else {
            required_capacity
        };
        let requested_bytes = target_capacity
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        self.check_total_after_reclaim(&mut without_old, requested_bytes, stack.as_deref_mut())?;

        if values.try_reserve_exact(target_capacity - values.len()).is_err() {
            if self.limit == usize::MAX || target_capacity == required_capacity {
                return Err(JcsReserveError::Allocation);
            }
            // A speculative growth must not turn an exact-fit allocation into
            // an allocation failure.
            values
                .try_reserve_exact(required_capacity - values.len())
                .map_err(|_| JcsReserveError::Allocation)?;
        }
        let mut actual_bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        if is_stack
            && self.limit != usize::MAX
            && target_capacity > required_capacity
            && without_old
                .checked_add(actual_bytes)
                .is_none_or(|total| total > self.limit)
        {
            // Allocators may return more capacity than requested. Retry with
            // the exact requirement before rejecting a speculative growth.
            self.replace_stack(values, required_capacity)?;
            actual_bytes = self.stack_bytes;
        }
        let actual_total = self.check_total_after_reclaim(&mut without_old, actual_bytes, stack)?;

        if is_stack {
            self.stack_bytes = actual_bytes;
        } else {
            self.object_bytes = self
                .object_bytes
                .checked_sub(old_bytes)
                .and_then(|current| current.checked_add(actual_bytes))
                .ok_or_else(|| self.limit_error(usize::MAX))?;
        }
        self.peak_bytes = self.peak_bytes.max(actual_total);
        #[cfg(feature = "jcs-allocation-diagnostics")]
        if is_stack {
            self.stack_growth_count = self.stack_growth_count.saturating_add(1);
            self.stack_requested_bytes = self.stack_requested_bytes.saturating_add(requested_bytes);
            self.stack_max_actual_bytes = self.stack_max_actual_bytes.max(actual_bytes);
        } else {
            self.object_growth_count = self.object_growth_count.saturating_add(1);
            self.object_requested_bytes = self.object_requested_bytes.saturating_add(requested_bytes);
            self.object_max_actual_bytes = self.object_max_actual_bytes.max(actual_bytes);
        }
        Ok(())
    }

    /// Checks a replacement capacity, reclaiming unused stack slots only when
    /// the requested or actual total would exceed the limit.
    ///
    /// Returns the accepted total, or a limit/allocation error if reclaiming
    /// the stack cannot make the replacement fit. `without_old` is adjusted
    /// to reflect any successful reclamation.
    fn check_total_after_reclaim<T>(
        &mut self,
        without_old: &mut usize,
        replacement_bytes: usize,
        stack: Option<&mut Vec<T>>,
    ) -> Result<usize, JcsReserveError> {
        let mut total = without_old
            .checked_add(replacement_bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        if total > self.limit {
            if let Some(stack) = stack {
                let old_stack_bytes = self.stack_bytes;
                let minimum_total = without_old
                    .checked_sub(old_stack_bytes)
                    .and_then(|bytes| {
                        stack
                            .len()
                            .checked_mul(size_of::<T>())
                            .and_then(|live| bytes.checked_add(live))
                    })
                    .and_then(|bytes| bytes.checked_add(replacement_bytes))
                    .ok_or_else(|| self.limit_error(usize::MAX))?;
                if minimum_total > self.limit {
                    return Err(self.limit_error(minimum_total));
                }
                self.replace_stack(stack, stack.len())?;
                // Future siblings may need progressively larger tables.
                // Reusing their reclaimed spare would copy these ancestors
                // again on each sibling, even when the old exact path fits.
                self.stack_growth_is_exact = true;
                *without_old = without_old
                    .checked_sub(old_stack_bytes)
                    .and_then(|bytes| bytes.checked_add(self.stack_bytes))
                    .ok_or_else(|| self.limit_error(usize::MAX))?;
                total = without_old
                    .checked_add(replacement_bytes)
                    .ok_or_else(|| self.limit_error(usize::MAX))?;
            }
            if total > self.limit {
                return Err(self.limit_error(total));
            }
        }
        Ok(total)
    }

    /// Replaces a stack with a fallibly allocated exact-capacity vector.
    ///
    /// Requested and actual replacement capacities must fit alongside live
    /// object tables or a limit error is returned. Allocation failure leaves
    /// the original stack intact. As with reserve, accounting excludes the
    /// old allocation; both allocations can briefly coexist physically.
    fn replace_stack<T>(&mut self, stack: &mut Vec<T>, capacity: usize) -> Result<(), JcsReserveError> {
        if capacity >= stack.capacity() {
            return Ok(());
        }
        let requested = capacity
            .checked_mul(size_of::<T>())
            .and_then(|bytes| self.object_bytes.checked_add(bytes))
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        if requested > self.limit {
            return Err(self.limit_error(requested));
        }
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|_| JcsReserveError::Allocation)?;
        let actual_bytes = replacement
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        let actual = self
            .object_bytes
            .checked_add(actual_bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        if actual > self.limit {
            return Err(self.limit_error(actual));
        }
        replacement.append(stack);
        *stack = replacement;
        self.stack_bytes = actual_bytes;
        #[cfg(feature = "jcs-allocation-diagnostics")]
        {
            self.stack_replacement_count = self.stack_replacement_count.saturating_add(1);
            self.stack_requested_bytes = self.stack_requested_bytes.saturating_add(capacity * size_of::<T>());
            self.stack_max_actual_bytes = self.stack_max_actual_bytes.max(actual_bytes);
        }
        Ok(())
    }

    /// Creates the limit error for a requested total byte count.
    ///
    /// # Parameters
    ///
    /// - `requested`: Combined byte capacity that triggered the limit error.
    ///
    /// # Returns
    ///
    /// A limit error containing the configured limit and requested total.
    #[inline]
    fn limit_error(&self, requested: usize) -> JcsReserveError {
        JcsReserveError::Limit(JcsWorkingMemoryError::new(self.limit, requested))
    }
}

#[cfg(test)]
mod tests {
    use super::JcsReserveError;
    use super::JcsWorkingMemory;

    #[test]
    fn test_reserve_rejects_accounting_overflow_without_allocating() {
        for path in ["live-total", "untracked-capacity", "element-bytes", "requested-total"] {
            let mut memory = JcsWorkingMemory::new(usize::MAX);
            let mut values = Vec::<u64>::new();
            let additional = match path {
                "live-total" => {
                    memory.stack_bytes = usize::MAX;
                    memory.object_bytes = 1;
                    1
                }
                "untracked-capacity" => {
                    values = vec![0];
                    values.capacity() + 1
                }
                "element-bytes" => usize::MAX / std::mem::size_of::<u64>() + 1,
                _ => {
                    memory.stack_bytes = usize::MAX - 1;
                    1
                }
            };
            let capacity = values.capacity();
            let tracked = (memory.stack_bytes, memory.object_bytes, memory.peak_bytes());
            match memory.reserve_object_entries(&mut values, additional) {
                Err(JcsReserveError::Limit(error)) => {
                    assert_eq!(error.limit(), usize::MAX);
                    assert_eq!(error.requested(), usize::MAX);
                }
                other => panic!("expected arithmetic rejection for {path}, got {other:?}"),
            }
            assert_eq!(values.capacity(), capacity, "{path} allocated before checking");
            assert_eq!((memory.stack_bytes, memory.object_bytes, memory.peak_bytes()), tracked);
        }
    }

    #[test]
    fn test_tracks_exact_capacity_boundary_and_rejects_one_byte_less() {
        let requested = std::mem::size_of::<u64>() * 2;
        let mut exact = JcsWorkingMemory::new(requested);
        let mut entries = Vec::<u64>::new();
        exact
            .reserve_object_entries(&mut entries, 2, &mut Vec::<u64>::new())
            .unwrap();
        assert!(entries.capacity() >= 2);
        assert!(exact.peak_bytes() >= requested);

        let mut short = JcsWorkingMemory::new(requested - 1);
        let mut entries = Vec::<u64>::new();
        match short.reserve_object_entries(&mut entries, 2, &mut Vec::<u64>::new()) {
            Err(JcsReserveError::Limit(error)) => {
                assert_eq!(error.limit(), requested - 1);
                assert!(error.requested() >= requested);
            }
            other => panic!("expected a working memory limit error, got {other:?}"),
        }
        assert!(entries.is_empty());
    }

    #[test]
    fn test_tracks_stack_capacity_and_keeps_it_after_a_no_growth_reserve() {
        let mut memory = JcsWorkingMemory::new(usize::MAX);
        let mut stack = Vec::<u64>::new();

        memory.reserve_stack(&mut stack, 2).unwrap();
        let expected = stack.capacity() * std::mem::size_of::<u64>();
        assert_eq!(memory.stack_bytes, expected);
        assert_eq!(memory.peak_bytes(), expected);

        memory.reserve_stack(&mut stack, 1).unwrap();
        assert_eq!(memory.stack_bytes, expected);
        assert_eq!(memory.peak_bytes(), expected);
    }

    #[test]
    fn test_unlimited_stack_growth_is_amortized() {
        let mut memory = JcsWorkingMemory::new(usize::MAX);
        let mut stack = Vec::<u64>::new();
        let mut growths = 0;
        for _ in 0..8192 {
            let old = stack.capacity();
            memory.reserve_stack(&mut stack, 1).unwrap();
            growths += usize::from(stack.capacity() != old);
            stack.push(0);
        }
        assert!(growths <= 15, "unexpected growth count: {growths}");
        assert_eq!(memory.stack_bytes, stack.capacity() * std::mem::size_of::<u64>());
    }

    #[test]
    fn test_finite_stack_growth_is_amortized() {
        for depth in [4097, 6000, 8192] {
            let mut memory = JcsWorkingMemory::new(depth * std::mem::size_of::<u64>());
            let mut stack = Vec::<u64>::new();
            let mut growths = 0;
            let mut requested_bytes = 0;
            for _ in 0..depth {
                let old = stack.capacity();
                memory
                    .reserve_stack(&mut stack, 1)
                    .expect("finite deep stack fits its exact budget");
                if stack.capacity() != old {
                    growths += 1;
                    requested_bytes += stack.capacity() * std::mem::size_of::<u64>();
                }
                stack.push(0);
            }
            assert!(growths <= 15, "unexpected finite growth count: {growths}");
            // A final budget-capped growth adds at most one full budget to
            // the preceding geometric series, which is below two budgets.
            assert!(requested_bytes <= 3 * memory.limit);
            assert_eq!(memory.stack_bytes, stack.capacity() * std::mem::size_of::<u64>());
            assert!(memory.peak_bytes() <= memory.limit);
        }
    }

    #[test]
    fn test_finite_stack_falls_back_to_exact_capacity_near_limit() {
        let mut memory = JcsWorkingMemory::new(13 * std::mem::size_of::<u64>());
        let mut stack = Vec::new();
        for value in 0_u64..13 {
            memory
                .reserve_stack(&mut stack, 1)
                .expect("exact capacity fits the remaining budget");
            stack.push(value);
        }
        assert_eq!(stack, (0_u64..13).collect::<Vec<_>>());
        assert_eq!(memory.stack_bytes, memory.limit);
        match memory.reserve_stack(&mut stack, 1) {
            Err(JcsReserveError::Limit(error)) => {
                assert_eq!(error.limit(), memory.limit);
                assert_eq!(error.requested(), 14 * std::mem::size_of::<u64>());
            }
            other => panic!("expected the next stack frame to exceed the budget, got {other:?}"),
        }
    }

    #[test]
    fn test_object_table_reclaims_stack_spare_without_losing_active_values() {
        let mut memory = JcsWorkingMemory::new(10 * std::mem::size_of::<u64>());
        let mut stack = Vec::new();
        for value in 0_u64..5 {
            memory
                .reserve_stack(&mut stack, 1)
                .expect("stack fits before the object table");
            stack.push(value);
        }
        let mut entries = Vec::<u64>::new();
        memory
            .reserve_object_entries(&mut entries, 4, &mut stack)
            .expect("reclaiming stack spare must leave room for the member table");
        assert_eq!(stack, (0_u64..5).collect::<Vec<_>>());
        assert_eq!(memory.stack_bytes, stack.capacity() * std::mem::size_of::<u64>());
        memory
            .reserve_stack(&mut stack, 1)
            .expect("the object frame fits after its table");
        stack.push(5);
        assert_eq!(memory.stack_bytes + memory.object_bytes, memory.limit);
        assert!(memory.peak_bytes() <= memory.limit);
        #[cfg(feature = "jcs-allocation-diagnostics")]
        assert_eq!(memory.stack_replacement_count, 1);
    }

    #[test]
    fn test_growing_sibling_tables_do_not_repeatedly_copy_deep_ancestors() {
        let depth = 8192;
        let widths = 128;
        let mut memory = JcsWorkingMemory::new(
            (depth + 1) * std::mem::size_of::<[u64; 4]>() + widths * std::mem::size_of::<[u64; 3]>(),
        );
        let mut stack = Vec::new();
        for _ in 0..depth {
            memory
                .reserve_stack(&mut stack, 1)
                .expect("ancestor stack fits the exact budget");
            stack.push([0_u64; 4]);
        }
        let mut replacements = 0;
        for width in 1..=widths {
            let mut entries = Vec::<[u64; 3]>::new();
            let old_capacity = stack.capacity();
            memory
                .reserve_object_entries(&mut entries, width, &mut stack)
                .expect("all sibling tables fit alongside the exact stack high-water capacity");
            replacements += usize::from(stack.capacity() < old_capacity);
            memory.reserve_stack(&mut stack, 1).expect("the sibling's frame fits");
            stack.push([0_u64; 4]);
            stack.pop();
            memory
                .release_object_entries(&entries)
                .expect("release the completed sibling table");
        }
        assert!(replacements <= 1, "repeated ancestor copies: {replacements}");
        assert!(memory.peak_bytes() <= memory.limit);
    }

    #[test]
    fn test_releases_object_capacity_before_a_later_object_is_reserved() {
        let mut memory = JcsWorkingMemory::new(usize::MAX);
        let mut stack = Vec::<u64>::new();
        memory.reserve_stack(&mut stack, 1).unwrap();

        let mut first_entries = Vec::<u64>::new();
        memory
            .reserve_object_entries(&mut first_entries, 2, &mut stack)
            .unwrap();
        let total = memory.stack_bytes + memory.object_bytes;
        memory.limit = total;
        memory.release_object_entries(&first_entries).unwrap();
        drop(first_entries);

        let mut second_entries = Vec::<u64>::new();
        memory
            .reserve_object_entries(&mut second_entries, 2, &mut stack)
            .unwrap();
        assert_eq!(
            memory.object_bytes,
            second_entries.capacity() * std::mem::size_of::<u64>()
        );
        assert!(memory.stack_bytes + memory.object_bytes <= memory.limit);
    }

    #[test]
    fn test_reports_allocation_and_accounting_failures() {
        let impossible_capacity = isize::MAX as usize + 1;

        let mut stack_memory = JcsWorkingMemory::new(usize::MAX);
        let mut stack = Vec::<u8>::new();
        assert!(matches!(
            stack_memory.reserve_stack(&mut stack, impossible_capacity),
            Err(JcsReserveError::Allocation),
        ));
        assert!(stack.is_empty());

        let mut object_memory = JcsWorkingMemory::new(usize::MAX);
        let mut entries = Vec::<u8>::new();
        assert!(matches!(
            object_memory.reserve_object_entries(&mut entries, impossible_capacity, &mut Vec::<u8>::new()),
            Err(JcsReserveError::Allocation),
        ));
        assert!(entries.is_empty());

        let mut populated_stack = vec![0_u8];
        let mut memory = JcsWorkingMemory::new(usize::MAX);
        match memory.reserve_stack(&mut populated_stack, usize::MAX) {
            Err(JcsReserveError::Limit(error)) => {
                assert_eq!(error.limit(), usize::MAX);
                assert_eq!(error.requested(), usize::MAX);
            }
            other => panic!("expected a stack capacity overflow, got {other:?}"),
        }

        let untracked_entries = vec![0_u8];
        match memory.release_object_entries(&untracked_entries) {
            Err(JcsReserveError::Limit(error)) => {
                assert_eq!(error.limit(), usize::MAX);
                assert_eq!(error.requested(), usize::MAX);
            }
            other => panic!("expected an untracked capacity error, got {other:?}"),
        }
    }

    #[cfg(feature = "jcs-allocation-diagnostics")]
    #[test]
    fn test_diagnostic_counts_successful_growth_by_collection() {
        let mut memory = JcsWorkingMemory::new(usize::MAX);
        let mut stack = Vec::<u64>::new();
        let mut entries = Vec::<(usize, usize)>::new();

        memory.reserve_stack(&mut stack, 2).unwrap();
        memory.reserve_stack(&mut stack, 1).unwrap();
        memory
            .reserve_object_entries(&mut entries, 2, &mut Vec::<u64>::new())
            .unwrap();

        assert_eq!(memory.stack_growth_count, 1);
        assert_eq!(memory.stack_requested_bytes, 2 * std::mem::size_of::<u64>());
        assert_eq!(
            memory.stack_max_actual_bytes,
            stack.capacity() * std::mem::size_of::<u64>()
        );
        assert_eq!(memory.object_growth_count, 1);
        assert_eq!(memory.object_requested_bytes, 2 * std::mem::size_of::<(usize, usize)>());
        assert_eq!(
            memory.object_max_actual_bytes,
            entries.capacity() * std::mem::size_of::<(usize, usize)>(),
        );
    }
}
