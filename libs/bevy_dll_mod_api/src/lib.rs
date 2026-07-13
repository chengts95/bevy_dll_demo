//! Experimental dynamic-mod ABI probes.
//!
//! This crate intentionally does not promise a stable Rust ABI.  It only gives
//! the host and a candidate dynamic library a tiny shared vocabulary for asking:
//! "were these critical ECS-facing types produced by the same compatible
//! compilation universe?"

use std::hash::{Hash, Hasher};
use bevy_ecs::stable_typeid::TypeId;
use std::collections::HashMap;
use bevy_ecs::prelude::Resource;

pub const TYPE_ID_PROBE_ABI_VERSION: u32 = 1;
pub const TYPE_ID_PROBE_ABI_VERSION_SYMBOL: &[u8] = b"bevy_dll_mod_type_id_probe_abi_version\0";
pub const TYPE_ID_PROBES_SYMBOL: &[u8] = b"bevy_dll_mod_type_id_probes_v1\0";
pub const COMPONENT_LIB_ABI_VERSION: u32 = 1;
pub const COMPONENT_LIB_ABI_VERSION_SYMBOL: &[u8] = b"bevy_dll_mod_component_lib_abi_version\0";
pub const COMPONENT_LIB_SYMBOL: &[u8] = b"bevy_dll_mod_component_lib_v1\0";

#[derive(Debug)]
pub struct AbiSentinel;

/// Raw trailing CLI arguments (passed after `--`) for mods to consume.
#[derive(Clone, Debug, Default, Resource)]
pub struct RawCliArgs {
    pub args: HashMap<String, String>,
}

#[derive(bevy_ecs::prelude::SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CaseLoadSet {
    /// Phase 1: parse case file and inject CaseExtensionFields
    LoadFile,
    /// Phase 2: read CaseExtensionFields and load mod-specific libraries
    ParseLibraries,
    /// Phase 3: spawn entities based on fully populated libraries
    SpawnInstances,
}

#[derive(bevy_ecs::component::Component, Debug)]
pub struct EcsSentinelComponent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeIdProbe {
    pub name: &'static str,
    pub hash: u64,
    pub size: usize,
    pub align: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedTypeIdProbe {
    pub name: String,
    pub hash: u64,
    pub size: usize,
    pub align: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedComponentDef {
    pub name: String,
    pub schema_json: String,
    pub example_json: String,
}

impl TypeIdProbe {
    pub fn of<T: 'static>(name: &'static str) -> Self {
        Self {
            name,
            hash: type_id_hash::<T>(),
            size: std::mem::size_of::<T>(),
            align: std::mem::align_of::<T>(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeIdProbeError {
    Missing {
        name: &'static str,
    },
    Mismatch {
        name: &'static str,
        expected_hash: u64,
        actual_hash: u64,
        expected_size: usize,
        actual_size: usize,
        expected_align: usize,
        actual_align: usize,
    },
}

impl std::fmt::Display for TypeIdProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { name } => write!(f, "missing TypeId probe '{name}'"),
            Self::Mismatch {
                name,
                expected_hash,
                actual_hash,
                expected_size,
                actual_size,
                expected_align,
                actual_align,
            } => write!(
                f,
                "TypeId probe '{name}' mismatch:\n  Hash : expected {expected_hash:#018x}, got {actual_hash:#018x}\n  Size : expected {expected_size}, got {actual_size}\n  Align: expected {expected_align}, got {actual_align}"
            ),
        }
    }
}

impl std::error::Error for TypeIdProbeError {}

pub fn bevy_type_id_probes() -> Vec<TypeIdProbe> {
    vec![
        TypeIdProbe {
            name: "__META_IS_DEBUG_BUILD__",
            hash: if cfg!(debug_assertions) { 1 } else { 0 },
            size: 0,
            align: 0,
        },
        TypeIdProbe::of::<AbiSentinel>("bevy_dll_mod_api::AbiSentinel"),
        TypeIdProbe::of::<EcsSentinelComponent>("bevy_dll_mod_api::EcsSentinelComponent"),
        TypeIdProbe::of::<bevy_ecs::world::World>("bevy_ecs::world::World"),
        TypeIdProbe::of::<bevy_ecs::schedule::Schedule>("bevy_ecs::schedule::Schedule"),
        TypeIdProbe::of::<bevy_app::App>("bevy_app::App"),
    ]
}

pub fn check_type_id_probes(
    expected: &[TypeIdProbe],
    actual: &[TypeIdProbe],
) -> Result<(), TypeIdProbeError> {
    for expected_probe in expected {
        let Some(actual_probe) = actual
            .iter()
            .find(|actual_probe| actual_probe.name == expected_probe.name)
        else {
            return Err(TypeIdProbeError::Missing {
                name: expected_probe.name,
            });
        };
        if actual_probe.hash != expected_probe.hash || actual_probe.size != expected_probe.size || actual_probe.align != expected_probe.align {
            return Err(TypeIdProbeError::Mismatch {
                name: expected_probe.name,
                expected_hash: expected_probe.hash,
                actual_hash: actual_probe.hash,
                expected_size: expected_probe.size,
                actual_size: actual_probe.size,
                expected_align: expected_probe.align,
                actual_align: actual_probe.align,
            });
        }
    }
    Ok(())
}

pub fn check_owned_type_id_probes(
    expected: &[TypeIdProbe],
    actual: &[OwnedTypeIdProbe],
) -> Result<(), TypeIdProbeError> {
    for expected_probe in expected {
        let Some(actual_probe) = actual
            .iter()
            .find(|actual_probe| actual_probe.name == expected_probe.name)
        else {
            return Err(TypeIdProbeError::Missing {
                name: expected_probe.name,
            });
        };
        if actual_probe.hash != expected_probe.hash || actual_probe.size != expected_probe.size || actual_probe.align != expected_probe.align {
            return Err(TypeIdProbeError::Mismatch {
                name: expected_probe.name,
                expected_hash: expected_probe.hash,
                actual_hash: actual_probe.hash,
                expected_size: expected_probe.size,
                actual_size: actual_probe.size,
                expected_align: expected_probe.align,
                actual_align: actual_probe.align,
            });
        }
    }
    Ok(())
}

pub fn type_id_hash<T: 'static>() -> u64 {
    let mut hasher = TypeIdHasher::default();
    TypeId::of::<T>().hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
struct TypeIdHasher {
    value: u64,
}

impl Hasher for TypeIdHasher {
    fn finish(&self) -> u64 {
        self.value
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.value ^= u64::from(*byte);
            self.value = self.value.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AbiStr {
    pub ptr: *const u8,
    pub len: usize,
}

unsafe impl Send for AbiStr {}
unsafe impl Sync for AbiStr {}

impl AbiStr {
    pub fn from_static(value: &'static str) -> Self {
        Self {
            ptr: value.as_ptr(),
            len: value.len(),
        }
    }

    /// # Safety
    ///
    /// `ptr..ptr+len` must be a valid UTF-8 byte range for the duration of
    /// this call.
    pub unsafe fn to_str(self) -> Result<&'static str, std::str::Utf8Error> {
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        std::str::from_utf8(bytes)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AbiTypeIdProbe {
    pub name: AbiStr,
    pub hash: u64,
    pub size: usize,
    pub align: usize,
}

unsafe impl Send for AbiTypeIdProbe {}
unsafe impl Sync for AbiTypeIdProbe {}

impl AbiTypeIdProbe {
    pub fn from_probe(probe: TypeIdProbe) -> Self {
        Self {
            name: AbiStr::from_static(probe.name),
            hash: probe.hash,
            size: probe.size,
            align: probe.align,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AbiTypeIdProbeSlice {
    pub ptr: *const AbiTypeIdProbe,
    pub len: usize,
}

impl AbiTypeIdProbeSlice {
    /// # Safety
    ///
    /// The slice must come from a loaded mod using the matching
    /// `TYPE_ID_PROBE_ABI_VERSION`, and its memory must remain valid while this
    /// function copies it.
    pub unsafe fn to_owned_vec(self) -> Result<Vec<OwnedTypeIdProbe>, std::str::Utf8Error> {
        let probes = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        probes
            .iter()
            .map(|probe| {
                Ok(OwnedTypeIdProbe {
                    name: unsafe { probe.name.to_str()? }.to_string(),
                    hash: probe.hash,
                    size: probe.size,
                    align: probe.align,
                })
            })
            .collect()
    }
}

pub type TypeIdProbeAbiVersionFn = unsafe extern "C" fn() -> u32;
pub type TypeIdProbesFn = unsafe extern "C" fn() -> AbiTypeIdProbeSlice;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AbiComponentDef {
    pub name: AbiStr,
    pub schema_json: AbiStr,
    pub example_json: AbiStr,
}

unsafe impl Send for AbiComponentDef {}
unsafe impl Sync for AbiComponentDef {}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AbiSnapshotRegistryDef {
    pub name: crate::AbiStr,
    pub registry_ptr: *mut std::ffi::c_void,
}
unsafe impl Send for AbiSnapshotRegistryDef {}
unsafe impl Sync for AbiSnapshotRegistryDef {}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AbiSnapshotRegistrySlice {
    pub ptr: *const AbiSnapshotRegistryDef,
    pub len: usize,
}
unsafe impl Send for AbiSnapshotRegistrySlice {}
unsafe impl Sync for AbiSnapshotRegistrySlice {}

impl AbiComponentDef {
    pub fn from_leaked_json(name: &'static str, schema_json: String, example_json: String) -> Self {
        let schema_json: &'static str = Box::leak(schema_json.into_boxed_str());
        let example_json: &'static str = Box::leak(example_json.into_boxed_str());
        Self {
            name: AbiStr::from_static(name),
            schema_json: AbiStr::from_static(schema_json),
            example_json: AbiStr::from_static(example_json),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AbiComponentDefSlice {
    pub ptr: *const AbiComponentDef,
    pub len: usize,
}

impl AbiComponentDefSlice {
    /// # Safety
    ///
    /// The slice must come from a loaded mod using the matching
    /// `COMPONENT_LIB_ABI_VERSION`, and its memory must remain valid while this
    /// function copies it.
    pub unsafe fn to_owned_vec(self) -> Result<Vec<OwnedComponentDef>, std::str::Utf8Error> {
        let components = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        components
            .iter()
            .map(|component| {
                Ok(OwnedComponentDef {
                    name: unsafe { component.name.to_str()? }.to_string(),
                    schema_json: unsafe { component.schema_json.to_str()? }.to_string(),
                    example_json: unsafe { component.example_json.to_str()? }.to_string(),
                })
            })
            .collect()
    }
}

pub type ComponentLibAbiVersionFn = unsafe extern "C" fn() -> u32;
pub type ComponentLibFn = unsafe extern "C" fn() -> AbiComponentDefSlice;

#[doc(hidden)]
pub mod __private {
    pub use schemars;
    pub use serde_json;
}

#[macro_export]
macro_rules! export_type_id_probes {
    ($($ty:ty => $name:expr),* $(,)?) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn bevy_dll_mod_type_id_probe_abi_version() -> u32 {
            $crate::TYPE_ID_PROBE_ABI_VERSION
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bevy_dll_mod_type_id_probes_v1() -> $crate::AbiTypeIdProbeSlice {
            static PROBES: std::sync::OnceLock<Box<[$crate::AbiTypeIdProbe]>> =
                std::sync::OnceLock::new();
            let probes = PROBES.get_or_init(|| {
                let mut values = $crate::bevy_type_id_probes()
                    .into_iter()
                    .map($crate::AbiTypeIdProbe::from_probe)
                    .collect::<Vec<_>>();
                $(
                    values.push($crate::AbiTypeIdProbe::from_probe(
                        $crate::TypeIdProbe::of::<$ty>($name),
                    ));
                )*
                values.into_boxed_slice()
            });
            $crate::AbiTypeIdProbeSlice {
                ptr: probes.as_ptr(),
                len: probes.len(),
            }
        }
    };
}

#[macro_export]
macro_rules! export_prefab_component_lib {
    ($($ty:ty => $name:expr),* $(,)?) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn bevy_dll_mod_component_lib_abi_version() -> u32 {
            $crate::COMPONENT_LIB_ABI_VERSION
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn bevy_dll_mod_component_lib_v1() -> $crate::AbiComponentDefSlice {
            static COMPONENTS: std::sync::OnceLock<Box<[$crate::AbiComponentDef]>> =
                std::sync::OnceLock::new();
            let components = COMPONENTS.get_or_init(|| {
                let mut values = Vec::new();
                $(
                    let schema = $crate::__private::schemars::schema_for!($ty);
                    let schema_json = $crate::__private::serde_json::to_string_pretty(&schema)
                        .expect("component schema must serialize");
                    let example_json =
                        $crate::__private::serde_json::to_string_pretty(&<$ty as Default>::default())
                            .expect("component default example must serialize");
                    values.push($crate::AbiComponentDef::from_leaked_json(
                        $name,
                        schema_json,
                        example_json,
                    ));
                )*
                values.into_boxed_slice()
            });
            $crate::AbiComponentDefSlice {
                ptr: components.as_ptr(),
                len: components.len(),
            }
        }
    };
}

#[macro_export]
macro_rules! export_snapshot_registries {
    ($($layer:expr => [$($ty:ty => $name:expr),* $(,)?]),* $(,)?) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn bevy_dll_mod_get_registries() -> $crate::AbiSnapshotRegistrySlice {
            static REGISTRIES: std::sync::OnceLock<std::boxed::Box<[$crate::AbiSnapshotRegistryDef]>> =
                std::sync::OnceLock::new();
            
            let slice = REGISTRIES.get_or_init(|| {
                let mut vec = std::vec::Vec::new();
                $(
                    {
                        let mut reg = bevy_archive::bevy_registry::SnapshotRegistry::default();
                        $(
                            reg.register_named::<$ty>($name);
                        )*
                        vec.push($crate::AbiSnapshotRegistryDef {
                            name: $crate::AbiStr::from_static($layer),
                            registry_ptr: std::boxed::Box::into_raw(std::boxed::Box::new(reg)) as *mut std::ffi::c_void,
                        });
                    }
                )*
                vec.into_boxed_slice()
            });

            $crate::AbiSnapshotRegistrySlice {
                ptr: slice.as_ptr(),
                len: slice.len(),
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_probe_sets_pass() {
        let probes = bevy_type_id_probes();
        check_type_id_probes(&probes, &probes).unwrap();
    }

    #[test]
    fn mismatched_probe_is_rejected() {
        let expected = [TypeIdProbe::of::<AbiSentinel>("sentinel")];
        let actual = [TypeIdProbe {
            name: "sentinel",
            hash: expected[0].hash ^ 1,
        }];
        let err = check_type_id_probes(&expected, &actual).unwrap_err();
        assert!(matches!(err, TypeIdProbeError::Mismatch { .. }));
    }
}
