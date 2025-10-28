use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use geoutils::{Distance, Location};
use humanize_duration::{prelude::DurationExt, Truncate};
use prost::Message;
use std::collections::HashMap;
use std::env;

use crate::{
    gen::gtfs_pb::{FeedEntity, FeedMessage},
    types::{Stop, Stops, TrainInfo},
};

// (TODO) MTA gives multiple endpoints, each only serving a subset of the train data. this
// should combine all of them instead of being limited to only the trains that I care about!
async fn get_feed() -> Result<Vec<FeedEntity>> {
    let res =
        reqwest::get("https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/nyct%2Fgtfs-nqrw")
            .await?
            .bytes()
            .await?;

    Ok(FeedMessage::decode(res)?.entity)
}

// unfortunately, MTA gives us the FeedEntity stream in a pretty unhelpful form. Every
// entry contains either a vehicle position or a trip update, but they are consistently
// sent in pairs that match a single train. This function combines the feed into a vec
// of entries containing both the vehicle position and trip update, and tests to ensure
// that our invariant (of two messages per train) holds.
pub async fn get_train_info() -> Result<Vec<TrainInfo>> {
    let feed = get_feed().await?;
    if feed.len() % 2 != 0 {
        return Err(anyhow!(
            "train feed invariant not met: expected two entries per train"
        ));
    }

    let mut ret = vec![];
    let mut feed = feed.into_iter();

    // (TODO) some pretty non-graceful error handling here, probably better to skip bad
    // cases than to fail entirely
    while let Some(fe) = feed.next() {
        // we use a while loop here rather than the normally-cleaner `fold` because we're
        // creating one `TrainInfo` entry out of a combination of two `FeedEntry`s.
        let fe2 = feed.next().ok_or(anyhow!("unexpected feed count"))?;

        let trip_update = match fe.trip_update {
            Some(tu) => tu,
            None => fe2.trip_update.ok_or(anyhow!("no trip update found"))?,
        };

        let vehicle_position = match fe.vehicle {
            Some(v) => v,
            None => fe2.vehicle.ok_or(anyhow!("no vehicle position found"))?,
        };

        if Some(&trip_update.trip) != vehicle_position.trip.as_ref() {
            return Err(anyhow!(
                "train feed invariant not met: trip updates of companion entries were not equal"
            ));
        }

        ret.push(TrainInfo {
            trip_update,
            vehicle_position,
        });
    }

    Ok(ret)
}

// (TODO) make these part of `TrainInfo` impl.
pub fn route_from_train_info(ti: &TrainInfo) -> Option<String> {
    ti.vehicle_position
        .trip
        .as_ref()
        .map(|t| t.route_id())
        .map(|ri| ri.to_string())
}

// given train info and a stop ID, determine when the train is expected to arrive
pub fn eta_from_train_info(ti: &TrainInfo, stop_id: &String) -> Option<String> {
    let arrival_info = ti
        .trip_update
        .stop_time_update
        .iter()
        .find(|stu| stu.stop_id.as_ref() == Some(stop_id))
        .and_then(|stu| stu.arrival.as_ref());

    let mut eta = arrival_info.map(|ste| ste.time()).unwrap_or(0);
    if eta != 0 {
        let delay = arrival_info.and_then(|ste| ste.delay).unwrap_or(0);
        eta += delay as i64;
    }
    // used to filter out trains that are a long time off (defined as more than 30min away)
    // (TODO) 30min default is fine, but making this customizable would be cool
    let long_duration = Duration::new(60 * 30, 0)?;
    match time_until_ts(eta) {
        Some(duration) => {
            // if ETA is negative then it's in the past so not useful to provide info on.
            // if ETA is too long out, then it's probably not useful information
            if duration < Duration::zero() || duration > long_duration {
                None
            } else {
                Some(duration.human(Truncate::Second).to_string())
            }
        }
        None => Some("<UNKNOWN>".to_string()),
    }
}

pub fn get_stop(stops: &Stops, stop_id: &String) -> String {
    stops
        .get(stop_id)
        .map(|s| s.stop_name.clone())
        .unwrap_or("<UNKNOWN>".to_string())
}

// iterates over stop_time_updates to find ID of the next stop, inferred based on which
// stop has the closest arrival time that is in the future. Falls back to vehicle position stop_id
pub fn stop_id_from_train_info(ti: &TrainInfo) -> String {
    let stop_time_updates = &ti.trip_update.stop_time_update;
    let now = Utc::now().timestamp();
    // (TODO) clean up a bit, this is a little awkward. also it's the second place we iter over
    // `stop_time_updates` (see also `eta_from_train_info`), which could probably be cleaned up
    let (_, next_stop) =
        stop_time_updates
            .iter()
            .fold((i64::MAX, None), |(to_closest, next_stop), stop| {
                if let Some(arrival) = &stop.arrival {
                    let time_to_arrival = arrival.time() - now;
                    if time_to_arrival > 0 && time_to_arrival < to_closest {
                        (time_to_arrival, Some(stop.stop_id().to_string()))
                    } else {
                        (to_closest, next_stop)
                    }
                } else {
                    (to_closest, next_stop)
                }
            });
    // (TODO) fallback to `vehicle_position.stop_id()` is almost certainly wrong based on
    // testing (it seems to always give the first stop of the route)
    next_stop.unwrap_or_else(|| ti.vehicle_position.stop_id().to_string())
}

fn time_until_ts(ts: i64) -> Option<Duration> {
    let now = Utc::now().to_utc();
    DateTime::from_timestamp(ts, 0).map(|ts| ts - now)
}

// (q) better to have this live here or in types.rs? I think it's better here because we
// aren't introducing error types and env parsing to types.rs, and because this is a bit
// complicated for a constructor. but I could see the argument going the other way.
pub fn get_stops() -> Result<Stops> {
    // (TODO) this is brittle, especially if building for different architectures
    let mut binary_dir = env::current_exe()?
        .parent()
        .ok_or(anyhow!("couldn't find parent directory"))?
        .to_path_buf();
    binary_dir.push("../../gtfs_subway/stops.txt");
    let mut reader = csv::Reader::from_path(binary_dir)?;
    let mut ret = HashMap::new();
    for result in reader.deserialize() {
        let record: Stop = result?; // type annotation needed for unknown reason here
        ret.insert(record.stop_id.clone(), record);
    }

    Ok(ret)
}

// filters a train feed to find trains at or approaching the given stop
pub fn find_trains_near_stop(feed: &[TrainInfo], stop: &Stop) -> Vec<TrainInfo> {
    feed.iter()
        .filter(|ti| stop_id_from_train_info(ti) == stop.stop_id)
        .cloned()
        .collect()
}

// tells if a given stop is within one mile of the provided latitude/longitude
fn stop_is_near(stop: &Stop, lat: f64, lon: f64) -> bool {
    let stop_loc = Location::new(stop.stop_lat, stop.stop_lon);
    let loc = Location::new(lat, lon);
    stop_loc
        // (TODO) `Distance::from_meters` isn't a const function but it would be nice to have
        // this value be shared in the long term, as more functionality would probably be nice
        // to add
        .is_in_circle(&loc, Distance::from_meters(1610))
        .unwrap_or(false)
}

// filter stops based on proximity to given lat/lon
pub fn find_stops_near(stops: &Stops, lat: f64, lon: f64) -> Stops {
    stops
        .iter()
        // separating filter and map is more legible than a `filter_map` call here IMO
        .filter(|(_, stop)| stop_is_near(stop, lat, lon))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
