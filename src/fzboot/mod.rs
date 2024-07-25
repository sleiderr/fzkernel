mod err;
#[cfg(feature = "x86_64")]
pub mod exceptions;
#[cfg(feature = "alloc")]
pub mod irq;
#[cfg(feature = "x86_64")]
pub mod scheduler;
pub mod time;

pub mod errors {
    pub use crate::fzboot::err::*;
}
