//! Persistent compare hash cache: the on-disk record format and, later, the file store.
//! Lets a repeat compare skip re-reading files whose size, time and identity are unchanged.

pub mod record;

#[cfg(test)]
mod record_tests;
