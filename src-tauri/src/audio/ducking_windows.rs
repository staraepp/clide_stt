//! Windows audio ducking is unavailable in this initial port.
#[derive(Debug)]
pub struct Ducked;
impl Ducked {
    pub fn engage() -> Option<Self> {
        None
    }
}
