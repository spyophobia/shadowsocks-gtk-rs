//! This module contains helper functions used in multithreading.

use std::{
    any::type_name,
    sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use log::warn;

/// Obtain a lock on a `Mutex`, recover if poisoned.
pub fn mutex_lock<T>(lock: &Mutex<T>) -> MutexGuard<T> {
    lock.lock().unwrap_or_else(|err| {
        warn!("Lock on {} poisoned, recovering", type_name::<T>());
        err.into_inner()
    })
}

/// Obtain a read lock on a `RwLock` , recover if poisoned.
pub fn rwlock_read<T>(lock: &RwLock<T>) -> RwLockReadGuard<T> {
    lock.read().unwrap_or_else(|err| {
        warn!("Read lock on {} poisoned, recovering", type_name::<T>());
        err.into_inner()
    })
}
/// Obtain a write lock on a `RwLock` , recover if poisoned.
pub fn rwlock_write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<T> {
    lock.write().unwrap_or_else(|err| {
        warn!("Write lock on {} poisoned, recovering", type_name::<T>());
        err.into_inner()
    })
}
