//! Traits used by label implementations

use core::{
    any::Any,
    hash::{Hash, Hasher},
};

// Re-exported for use within `define_label!`
#[doc(hidden)]
pub use alloc::boxed::Box;

/// An object safe version of [`Eq`]. This trait is automatically implemented
/// for any `'static` type that implements `Eq`.
pub trait DynEq: Any {
    /// This method tests for `self` and `other` values to be equal.
    ///
    /// Implementers should avoid returning `true` when the underlying types are
    /// not the same.
    fn dyn_eq(&self, other: &dyn DynEq) -> bool;
    
    /// Returns the stable TypeId of the underlying type.
    fn stable_type_id(&self) -> crate::stable_typeid::TypeId;
}

// Tests that this trait is dyn-compatible
const _: Option<Box<dyn DynEq>> = None;

impl<T> DynEq for T
where
    T: Any + Eq + 'static,
{
    fn dyn_eq(&self, other: &dyn DynEq) -> bool {
        if self.stable_type_id() == other.stable_type_id() {
            // SAFETY: Since the stable type IDs match, `other` is fundamentally the same type `T`.
            // Even across DLL boundaries, they are the same logical type.
            let other_ptr = other as *const dyn DynEq as *const ();
            let other_t = unsafe { &*(other_ptr as *const T) };
            return self == other_t;
        }
        false
    }
    
    fn stable_type_id(&self) -> crate::stable_typeid::TypeId {
        crate::stable_typeid::TypeId::of::<T>()
    }
}

/// An object safe version of [`Hash`]. This trait is automatically implemented
/// for any `'static` type that implements `Hash`.
pub trait DynHash: DynEq {
    /// Feeds this value into the given [`Hasher`].
    fn dyn_hash(&self, state: &mut dyn Hasher);
}

// Tests that this trait is dyn-compatible
const _: Option<Box<dyn DynHash>> = None;

impl<T> DynHash for T
where
    T: DynEq + Hash + 'static,
{
    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        T::hash(self, &mut state);
        self.stable_type_id().hash(&mut state);
    }
}

/// Macro to define a new label trait
///
/// # Example
///
/// ```
/// # use bevy_ecs::define_label;
/// define_label!(
///     /// Documentation of label trait
///     MyNewLabelTrait,
///     MY_NEW_LABEL_TRAIT_INTERNER
/// );
///
/// define_label!(
///     /// Documentation of another label trait
///     MyNewExtendedLabelTrait,
///     MY_NEW_EXTENDED_LABEL_TRAIT_INTERNER,
///     extra_methods: {
///         // Extra methods for the trait can be defined here
///         fn additional_method(&self) -> i32;
///     },
///     extra_methods_impl: {
///         // Implementation of the extra methods for Interned<dyn MyNewExtendedLabelTrait>
///         fn additional_method(&self) -> i32 {
///             0
///         }
///     }
/// );
/// ```
#[macro_export]
macro_rules! define_label {
    (
        $(#[$label_attr:meta])*
        $label_trait_name:ident,
        $interner_name:ident
    ) => {
        $crate::define_label!(
            $(#[$label_attr])*
            $label_trait_name,
            $interner_name,
            extra_methods: {},
            extra_methods_impl: {}
        );
    };
    (
        $(#[$label_attr:meta])*
        $label_trait_name:ident,
        $interner_name:ident,
        extra_methods: { $($trait_extra_methods:tt)* },
        extra_methods_impl: { $($interned_extra_methods_impl:tt)* }
    ) => {

        $(#[$label_attr])*
        pub trait $label_trait_name: Send + Sync + ::core::fmt::Debug + $crate::label::DynEq + $crate::label::DynHash {

            $($trait_extra_methods)*

            /// Clones this `
            #[doc = stringify!($label_trait_name)]
            ///`.
            fn dyn_clone(&self) -> $crate::label::Box<dyn $label_trait_name>;

            /// Returns an [`Interned`] value corresponding to `self`.
            fn intern(&self) -> $crate::intern::Interned<dyn $label_trait_name>
            where Self: Sized {
                $interner_name.intern(self)
            }
        }

        #[diagnostic::do_not_recommend]
        impl $label_trait_name for $crate::intern::Interned<dyn $label_trait_name> {

            $($interned_extra_methods_impl)*

            fn dyn_clone(&self) -> $crate::label::Box<dyn $label_trait_name> {
                (**self).dyn_clone()
            }

            fn intern(&self) -> Self {
                *self
            }
        }

        impl PartialEq for dyn $label_trait_name {
            fn eq(&self, other: &Self) -> bool {
                self.dyn_eq(other)
            }
        }

        impl Eq for dyn $label_trait_name {}

        impl ::core::hash::Hash for dyn $label_trait_name {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.dyn_hash(state);
            }
        }

        impl $crate::intern::Internable for dyn $label_trait_name {
            fn leak(&self) -> &'static Self {
                $crate::label::Box::leak(self.dyn_clone())
            }

            fn ref_eq(&self, other: &Self) -> bool {
                self.dyn_eq(other)
            }

            fn ref_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.dyn_hash(state);
            }
        }

        static $interner_name: $crate::intern::Interner<dyn $label_trait_name> =
            $crate::intern::Interner::new();
    };
}
