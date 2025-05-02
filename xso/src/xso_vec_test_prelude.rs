# use core::any::Any;
# use core::fmt::Debug;
# use xso::{derive_dyn_traits, dynxso::{XsoVec, TakeOneError}};
#
# trait Trait: Any + Debug {}
#
# derive_dyn_traits!(Trait);
