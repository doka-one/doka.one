use std::fmt::{Display, Formatter};
use rand::Rng;

#[derive(Debug,Copy,Clone)]
pub struct TrackerId(Option<i32>);

impl TrackerId {
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let tracker = rng.gen_range(0..1_000_000);
        TrackerId(Some(tracker))
    }
    pub fn from_value(val : Option<i32>) -> Self {
        TrackerId(val)
    }
    pub fn value(&self) -> Option<i32> {
        self.0
    }
}

impl Display for TrackerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(t) => {
                // TODO format with thousand-group separator, Ex : 987_789
                write!(f, "{}", t)
            }
            None => {
                write!(f, "None")
            }
        }
    }
}

#[derive(Debug,Clone)]
pub struct TwinId {
    pub session_id : String,
    pub tracker_id : TrackerId,
}

impl Display for TwinId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO format with thousand-group separator, Ex : 987_789
        write!(f, "({} / {})", self.tracker_id, self.session_id)
    }
}