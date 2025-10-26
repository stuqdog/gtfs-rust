use std::env;

use ::gtfs_poc::{helpers, types};
use anyhow::{anyhow, Result};
use gtfs_poc::helpers::stop_id_from_train_info;

#[tokio::main]
pub async fn main() -> Result<()> {
    let mut lat = None;
    let mut lon = None;

    // (TODO) this is brittle, use CLAP crate instead
    if env::args().len() == 3 {
        lat = env::args().nth(1).and_then(|s| s.parse::<f64>().ok());
        lon = env::args().nth(2).and_then(|s| s.parse::<f64>().ok());
    }

    // (TODO) lat and lon always go together, should probably be their own type
    let lat = lat.ok_or(anyhow!("no latitude provided"))?;
    let lon = lon.ok_or(anyhow!("no longitude provided"))?;

    let feed = helpers::get_feed().await?;
    let stops = helpers::get_stops()?;
    let train_info = helpers::train_info_from_feed(&feed)?;

    for (_, stop) in helpers::find_stops_near(&stops, lat, lon) {
        let nearby_trains = helpers::find_trains_near_stop(&train_info, &stop);
        for train in nearby_trains {
            let train_stop_id = stop_id_from_train_info(&train);
            let eta = match helpers::eta_from_train_info(&train, &train_stop_id) {
                Some(eta) => eta,
                None => continue,
            };
            let direction = types::Directionality::from_train_info(&train);
            let route = helpers::route_from_train_info(&train).unwrap_or("<UNKNOWN>".to_string());
            let stop = helpers::get_stop(&stops, &train_stop_id);
            if &stop == "<UNKNOWN>" {
                println!("unknown stop! stop_id is {train_stop_id}");
            }
            let direction = direction.to_string();

            // (TODO) this isn't exactly the prettiest way to share information but, it works!
            println!("nearby train: route {route} heading in {direction} direction, at or approaching station {stop}. ETA: {eta}")
        }
    }

    Ok(())
}
