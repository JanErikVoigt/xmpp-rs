# use core::any::Any;
# use core::fmt::Debug;
# use xso::{dynxso::{XsoVec, TakeOneError, DynXso, MayContain}, derive_dyn_traits};
#
# trait Trait: Any + Debug {}
#
# derive_dyn_traits!(Trait use () = ());
