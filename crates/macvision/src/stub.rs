//! Every system without Vision: nothing is available.

use crate::{Error, Mask};

pub(crate) fn available() -> bool {
    false
}

pub(crate) fn subject_mask(_rgba: &[u8], _width: u32, _height: u32) -> Result<Mask, Error> {
    Err(Error::Unavailable)
}
