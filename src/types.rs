use std::collections::HashMap;

use crate::gen::gtfs_pb::{TripUpdate, VehiclePosition};

#[derive(Clone)]
pub struct TrainInfo {
    pub trip_update: TripUpdate,
    pub vehicle_position: VehiclePosition,
}

// (TODO) `impl fmt::Display` so we can easily string print
pub enum Directionality {
    N,
    S,
    Unknown,
}

impl Directionality {
    // (TODO) the `trip_descriptor` has a `direction_id` field, but (1) it seems to be
    // consistently `None` for MTA data, and (2) the semantics aren't entirely clear to me.
    // If clarified, we could add parsing as a first-step effort at least.
    pub fn from_train_info(ti: &TrainInfo) -> Self {
        // (TODO) brittle string parsing but `direction_id` seems consistently `None`, see
        // if there's a better way
        match &ti.trip_update.trip.trip_id {
            None => Directionality::Unknown,
            Some(trip_id) => {
                if trip_id.contains("..N") {
                    Directionality::N
                } else if trip_id.contains("..S") {
                    Directionality::S
                } else {
                    Directionality::Unknown
                }
            }
        }
    }

    pub fn to_string(&self) -> &str {
        match &self {
            Self::N => "north",
            Self::S => "south",
            Self::Unknown => "<UNKNOWN>",
        }
    }
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct Stop {
    pub stop_id: String,
    pub stop_name: String,
    pub stop_lat: f64,
    pub stop_lon: f64,
    pub location_type: String,
    pub parent_station: String,
}

// key is stop ID
pub type Stops = HashMap<String, Stop>;
