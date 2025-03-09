pub mod eorzea_time;
pub mod weather;

// struct FishingHole {
//     name: String,
//     region: String,
// }

// pub enum Tug {
//     Light,
//     Medium,
//     Heavy,
// }
//
// pub enum Hookset {
//     Precision,
//     Powerful,
// }
//
// pub struct Fish<'a> {
//     name: String,
//     location: &'a FishingHole,
//     start_hour: u8,
//     end_hour: u8,
//     bait: String,
//     previous_weather_set: Vec<Weather>,
//     weather_set: Vec<Weather>,
//     best_catch_path: Vec<Fish<'a>>,
//     tug: Tug,
//     hookset: Hookset,
//     snagging: bool,
//     gig: bool,
//     folklore: bool,
//     fish_eyes: bool,
//     patch: (u8, u8),
// }

// impl Fish<'_> {
//     pub fn next_window(time: &SystemTime) -> Result<eorzea_time::EorzeaTime, Box<dyn Error>> {
//         Ok(EorzeaTime::from_time(time)?)
//     }
// }
