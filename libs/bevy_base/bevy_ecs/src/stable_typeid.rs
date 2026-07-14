//! Deterministic type identifiers for serialized ECS metadata.

use core::any::type_name;
use core::hash::Hasher;

#[cfg(test)]
std::thread_local! {
    static TYPE_ID_CALLS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// A stable hash of a Rust type name.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct TypeId(u64);

impl TypeId {
    /// Returns the stable identifier for `T`.
    #[inline(always)]
    pub fn of<T: ?Sized + 'static>() -> Self {
        #[cfg(test)]
        TYPE_ID_CALLS.set(TYPE_ID_CALLS.get() + 1);
        Self(fnv1a_64(type_name::<T>()))
    }
}

#[cfg(test)]
pub(crate) fn reset_call_count() {
    TYPE_ID_CALLS.set(0);
}

#[cfg(test)]
pub(crate) fn call_count() -> usize {
    TYPE_ID_CALLS.get()
}

impl core::fmt::Debug for TypeId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "TypeId({:016x})", self.0)
    }
}

/// Computes the 64-bit FNV-1a hash of `s`.
pub const fn fnv1a_64(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        hash = hash ^ (bytes[i] as u64);
        hash = hash.wrapping_mul(0x100000001b3);
        i += 1;
    }
    hash
}

/// Hasher that preserves already-computed 64-bit [`TypeId`] hashes.
#[derive(Default)]
pub struct TypeIdHasher(u64);

impl Hasher for TypeIdHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() == 8 {
            self.0 = u64::from_ne_bytes(bytes.try_into().unwrap());
        } else {
            for &b in bytes {
                self.0 = self.0.wrapping_mul(31).wrapping_add(b as u64);
            }
        }
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// Hash map keyed by stable [`TypeId`] values.
pub type TypeIdMap<V> =
    bevy_platform::collections::HashMap<TypeId, V, core::hash::BuildHasherDefault<TypeIdHasher>>;

/// Index map keyed by stable [`TypeId`] values.
pub type TypeIndexMap<V> =
    indexmap::IndexMap<TypeId, V, core::hash::BuildHasherDefault<TypeIdHasher>>;
