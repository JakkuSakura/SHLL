/// A macro to generate a common set of derives for a struct.
/// especially Clone, Debug, PartialEq, Eq, Hash
#[macro_export]
macro_rules! common_struct {
    (
        no_debug
        $(#[$attr:meta])*
        pub struct $name:ident { $($t:tt)* }
    ) => {
        #[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq, Hash)]
        $(#[$attr])*
        pub struct $name {
            $($t)*
        }
    };
    (

        $(#[$attr:meta])*
        pub struct $name:ident { $($t:tt)* }
    ) => {
        crate::common_struct!(
            no_debug
            $(#[$attr])*
            #[derive(Debug)]
            pub struct $name { $($t)* }
        );
    };

    (
        $(#[$attr:meta])*
        pub struct $name:ident;
    ) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
        pub struct $name;
    };
}
/// A macro to generate a common enum with a set of common traits.
/// especially From<Variant> for Enum
#[macro_export]
macro_rules! common_enum {
    (
        no_debug
        $(#[$attr:meta])*
        pub enum $name:ident { $($t:tt)* }
    ) => {
        #[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq, Hash, derive_more::From, derive_more::TryInto)]
        $(#[$attr])*
        pub enum $name {
            $($t)*
        }

    };
    (
        $(#[$attr:meta])*
        pub enum $name:ident { $($t:tt)* }
    ) => {
        crate::common_enum!(
            no_debug
            $(#[$attr])*
            #[derive(Debug)]
            pub enum $name { $($t)* }
        );
    };
}
