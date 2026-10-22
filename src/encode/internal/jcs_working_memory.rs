// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Accounts for auxiliary capacities held by the JCS renderer.

use std::mem::size_of;

use crate::encode::JcsWorkingMemoryError;

/// Failure reported while reserving tracked working memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JcsReserveError {
    /// The requested capacity would exceed the configured working-memory limit.
    Limit(JcsWorkingMemoryError),
    /// The allocator could not reserve the requested capacity.
    Allocation,
}

/// Tracks live stack and object member-table capacities.
pub(super) struct JcsWorkingMemory {
    /// Maximum number of bytes that may be held by tracked capacities.
    limit: usize,
    /// Bytes currently held by the traversal stack.
    stack_bytes: usize,
    /// Bytes currently held by object member tables.
    object_bytes: usize,
    /// Largest combined tracked capacity observed so far.
    peak_bytes: usize,
}

impl JcsWorkingMemory {
    /// Creates a tracker with the given capacity limit and no reserved memory.
    ///
    /// # Parameters
    ///
    /// - `limit`: Maximum combined byte capacity allowed for tracked
    ///   collections.
    pub(super) const fn new(limit: usize) -> Self {
        Self {
            limit,
            stack_bytes: 0,
            object_bytes: 0,
            peak_bytes: 0,
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
    /// # Errors
    ///
    /// Returns `JcsReserveError::Limit` if the resulting tracked capacity
    /// exceeds the configured limit, or `JcsReserveError::Allocation` if
    /// reservation fails.
    pub(super) fn reserve_stack<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), JcsReserveError> {
        self.reserve(values, additional, true)
    }

    /// Reserves enough capacity for additional entries in an object member
    /// table.
    ///
    /// # Parameters
    ///
    /// - `values`: Object member table whose capacity is tracked.
    /// - `additional`: Number of entries the table must hold beyond its current
    ///   length.
    ///
    /// # Errors
    ///
    /// Returns `JcsReserveError::Limit` if the resulting tracked capacity
    /// exceeds the configured limit, or `JcsReserveError::Allocation` if
    /// reservation fails.
    pub(super) fn reserve_object_entries<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), JcsReserveError> {
        self.reserve(values, additional, false)
    }

    /// Removes an object's member-table capacity from the tracked live total.
    ///
    /// # Parameters
    ///
    /// - `values`: Object member table whose capacity is no longer live.
    ///
    /// # Errors
    ///
    /// The caller must invoke this before dropping the corresponding vector.
    /// Returns a limit error if its capacity cannot be represented or is
    /// not currently tracked.
    pub(super) fn release_object_entries<T>(
        &mut self,
        values: &Vec<T>,
    ) -> Result<(), JcsReserveError> {
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
    ///
    /// # Errors
    ///
    /// The old capacity is excluded while checking the replacement allocation.
    /// Returns a limit error on arithmetic overflow or when the requested or
    /// actual capacity exceeds the configured limit, and an allocation
    /// error if reserving memory fails.
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        is_stack: bool,
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
        let requested_bytes = required_capacity
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        let current = self
            .stack_bytes
            .checked_add(self.object_bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        let without_old = current
            .checked_sub(old_bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        let requested_total = without_old
            .checked_add(requested_bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        if requested_total > self.limit {
            return Err(self.limit_error(requested_total));
        }

        values
            .try_reserve_exact(additional)
            .map_err(|_| JcsReserveError::Allocation)?;
        let actual_bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        let actual_total = without_old
            .checked_add(actual_bytes)
            .ok_or_else(|| self.limit_error(usize::MAX))?;
        if actual_total > self.limit {
            return Err(self.limit_error(actual_total));
        }

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
        Ok(())
    }

    /// Creates the limit error for a requested total byte count.
    ///
    /// # Parameters
    ///
    /// - `requested`: Combined byte capacity that triggered the limit error.
    fn limit_error(&self, requested: usize) -> JcsReserveError {
        JcsReserveError::Limit(JcsWorkingMemoryError::new(self.limit, requested))
    }
}

#[cfg(test)]
mod tests {
    use super::JcsReserveError;
    use super::JcsWorkingMemory;

    #[test]
    fn test_tracks_exact_capacity_boundary_and_rejects_one_byte_less() {
        let requested = std::mem::size_of::<u64>() * 2;
        let mut exact = JcsWorkingMemory::new(requested);
        let mut entries = Vec::<u64>::new();
        exact.reserve_object_entries(&mut entries, 2).unwrap();
        assert!(entries.capacity() >= 2);
        assert!(exact.peak_bytes() >= requested);

        let mut short = JcsWorkingMemory::new(requested - 1);
        let mut entries = Vec::<u64>::new();
        match short.reserve_object_entries(&mut entries, 2) {
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
    fn test_releases_object_capacity_before_a_later_object_is_reserved() {
        let mut memory = JcsWorkingMemory::new(usize::MAX);
        let mut stack = Vec::<u64>::new();
        memory.reserve_stack(&mut stack, 1).unwrap();

        let mut first_entries = Vec::<u64>::new();
        memory
            .reserve_object_entries(&mut first_entries, 2)
            .unwrap();
        let total = memory.stack_bytes + memory.object_bytes;
        memory.limit = total;
        memory.release_object_entries(&first_entries).unwrap();
        drop(first_entries);

        let mut second_entries = Vec::<u64>::new();
        memory
            .reserve_object_entries(&mut second_entries, 2)
            .unwrap();
        assert_eq!(
            memory.object_bytes,
            second_entries.capacity() * std::mem::size_of::<u64>()
        );
        assert!(memory.stack_bytes + memory.object_bytes <= memory.limit);
    }
}
