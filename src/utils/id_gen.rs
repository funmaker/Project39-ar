use std::sync::atomic::{AtomicU64, Ordering};

static ID_GEN: AtomicU64 = AtomicU64::new(0);

pub fn next_uid() -> u64 {
	ID_GEN.fetch_add(1, Ordering::Relaxed)
}
