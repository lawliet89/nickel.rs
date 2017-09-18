// HACK: Need cargo support to run `#[test]`s witin examples, this imitates it.

#![allow(dead_code)]
#![allow(unused_attributes)]
#![allow(resolve_trait_on_defaulted_unit)]
include!("../../examples/integration_testing.rs");
