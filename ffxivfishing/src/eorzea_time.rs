use std::{
    fmt,
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

pub const EORZEA_WEATHER_PERIOD: EorzeaDuration = EorzeaDuration {
    timestamp: BELL_IN_ESEC * 8,
};

const EORZEA_TIME_CONST: f64 = 3600.0 / 175.0;

const YEAR_IN_ESEC: u64 = 12 * MOON_IN_ESEC;
const MOON_IN_ESEC: u64 = 32 * SUN_IN_ESEC;
const SUN_IN_ESEC: u64 = 24 * BELL_IN_ESEC;
const BELL_IN_ESEC: u64 = 60 * MINUTE_IN_ESEC;
const MINUTE_IN_ESEC: u64 = 60;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct EorzeaTime {
    timestamp: u64,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct EorzeaDuration {
    timestamp: u64,
}

#[derive(Debug, PartialEq)]
pub enum EorzeaTimeCreationError {
    ValueOutOfBounds,
}

impl EorzeaTime {
    pub fn year(&self) -> u16 {
        (1 + self.timestamp / YEAR_IN_ESEC) as u16
    }
    pub fn moon(&self) -> u8 {
        (1 + self.timestamp / MOON_IN_ESEC % 12) as u8
    }
    pub fn sun(&self) -> u8 {
        (1 + self.timestamp / SUN_IN_ESEC % 32) as u8
    }
    pub fn bell(&self) -> u8 {
        (self.timestamp / BELL_IN_ESEC % 24) as u8
    }
    pub fn minute(&self) -> u8 {
        (self.timestamp / MINUTE_IN_ESEC % 60) as u8
    }
    pub fn second(&self) -> u8 {
        (self.timestamp % 60) as u8
    }

    pub fn new(
        year: u16,
        moon: u8,
        sun: u8,
        bell: u8,
        minute: u8,
        second: u8,
    ) -> Result<EorzeaTime, EorzeaTimeCreationError> {
        match (year, moon, sun, bell, minute, second) {
            (0, _, _, _, _, _) => Err(EorzeaTimeCreationError::ValueOutOfBounds),
            (_, m, _, _, _, _) if m == 0 || m > 12 => {
                Err(EorzeaTimeCreationError::ValueOutOfBounds)
            }
            (_, _, s, _, _, _) if s == 0 || s > 32 => {
                Err(EorzeaTimeCreationError::ValueOutOfBounds)
            }
            (_, _, _, b, _, _) if b >= 24 => Err(EorzeaTimeCreationError::ValueOutOfBounds),
            (_, _, _, _, m, _) if m >= 60 => Err(EorzeaTimeCreationError::ValueOutOfBounds),
            (_, _, _, _, _, s) if s >= 60 => Err(EorzeaTimeCreationError::ValueOutOfBounds),
            (y, m, s, b, min, sec) => Ok(EorzeaTime {
                timestamp: (y as u64 - 1) * YEAR_IN_ESEC
                    + (m as u64 - 1) * MOON_IN_ESEC
                    + (s as u64 - 1) * SUN_IN_ESEC
                    + b as u64 * BELL_IN_ESEC
                    + min as u64 * MINUTE_IN_ESEC
                    + sec as u64,
            }),
        }
    }

    pub fn from_time(time: &SystemTime) -> Result<EorzeaTime, SystemTimeError> {
        let eorzea_time = (time.duration_since(UNIX_EPOCH)?.as_secs() as f64) * EORZEA_TIME_CONST;
        Ok(EorzeaTime {
            timestamp: eorzea_time.round() as u64,
        })
    }

    pub fn from_esecs(secs: u64) -> EorzeaTime {
        EorzeaTime { timestamp: secs }
    }

    pub fn to_system_time(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH
            + Duration::from_secs((self.timestamp as f64 / EORZEA_TIME_CONST).round() as u64)
    }

    pub fn round(&mut self, sec: u64) {
        self.timestamp -= self.timestamp % sec;
    }
}

impl std::fmt::Display for EorzeaTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:0>4}-{:0>2}-{:0>2} {:0>2}:{:0>2}:{:0>2}",
            self.year(),
            self.moon(),
            self.sun(),
            self.bell(),
            self.minute(),
            self.second()
        )?;
        Ok(())
    }
}

impl std::ops::Add<EorzeaDuration> for EorzeaTime {
    type Output = Self;

    fn add(self, rhs: EorzeaDuration) -> Self::Output {
        EorzeaTime {
            timestamp: self.timestamp + rhs.timestamp,
        }
    }
}

impl std::ops::Sub<EorzeaDuration> for EorzeaTime {
    type Output = Self;

    fn sub(self, rhs: EorzeaDuration) -> Self::Output {
        if self.timestamp < rhs.timestamp {
            return EorzeaTime { timestamp: 0 };
        }
        EorzeaTime {
            timestamp: self.timestamp - rhs.timestamp,
        }
    }
}

impl std::ops::AddAssign<EorzeaDuration> for EorzeaTime {
    fn add_assign(&mut self, rhs: EorzeaDuration) {
        self.timestamp += rhs.timestamp;
    }
}

impl std::ops::SubAssign<EorzeaDuration> for EorzeaTime {
    fn sub_assign(&mut self, rhs: EorzeaDuration) {
        if self.timestamp < rhs.timestamp {
            return;
        }
        self.timestamp -= rhs.timestamp;
    }
}

impl EorzeaDuration {
    pub fn new_ext(
        year: u16,
        moon: u8,
        sun: u8,
        bell: u8,
        minute: u8,
        second: u8,
    ) -> Result<EorzeaDuration, EorzeaTimeCreationError> {
        EorzeaTime::new(year, moon, sun, bell, minute, second).map(|et| EorzeaDuration {
            timestamp: et.timestamp,
        })
    }

    pub fn new(
        bell: u8,
        minute: u8,
        second: u8,
    ) -> Result<EorzeaDuration, EorzeaTimeCreationError> {
        EorzeaTime::new(1, 1, 1, bell, minute, second).map(|et| EorzeaDuration {
            timestamp: et.timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{time::Duration, vec};

    use super::*;
    #[test]
    pub fn new_eorzea_time() {
        assert_eq!(
            EorzeaTime::new(0, 1, 1, 0, 0, 0),
            Err(EorzeaTimeCreationError::ValueOutOfBounds)
        );
        assert_eq!(
            EorzeaTime::new(1, 0, 1, 0, 0, 0),
            Err(EorzeaTimeCreationError::ValueOutOfBounds)
        );
        assert_eq!(
            EorzeaTime::new(1, 13, 1, 0, 0, 0),
            Err(EorzeaTimeCreationError::ValueOutOfBounds)
        );
        assert_eq!(
            EorzeaTime::new(1, 1, 0, 0, 0, 0),
            Err(EorzeaTimeCreationError::ValueOutOfBounds)
        );
        assert_eq!(
            EorzeaTime::new(1, 1, 33, 0, 0, 0),
            Err(EorzeaTimeCreationError::ValueOutOfBounds)
        );
        assert_eq!(
            EorzeaTime::new(1, 1, 1, 24, 0, 0),
            Err(EorzeaTimeCreationError::ValueOutOfBounds)
        );
        assert_eq!(
            EorzeaTime::new(1, 1, 1, 0, 60, 0),
            Err(EorzeaTimeCreationError::ValueOutOfBounds)
        );
        assert_eq!(
            EorzeaTime::new(1, 1, 1, 0, 0, 60),
            Err(EorzeaTimeCreationError::ValueOutOfBounds)
        );
        assert_eq!(
            EorzeaTime::new(1, 1, 1, 0, 0, 0),
            Ok(EorzeaTime { timestamp: 0 })
        );
        assert_eq!(
            EorzeaTime::new(1, 1, 1, 0, 0, 1),
            Ok(EorzeaTime { timestamp: 1 })
        );
        assert_eq!(
            EorzeaTime::new(1, 12, 32, 23, 59, 59),
            Ok(EorzeaTime {
                timestamp: YEAR_IN_ESEC - 1
            })
        );
    }

    #[test]
    pub fn systemtime_to_eorzeatime() {
        assert_eq!(
            EorzeaTime::from_time(&SystemTime::UNIX_EPOCH).unwrap(),
            EorzeaTime::new(1, 1, 1, 0, 0, 0).unwrap()
        );
        assert_eq!(
            EorzeaTime::from_time(&(SystemTime::UNIX_EPOCH + Duration::from_secs(60 * 70)))
                .unwrap(),
            EorzeaTime::new(1, 1, 2, 0, 0, 0).unwrap()
        );
        assert_eq!(
            EorzeaTime::from_time(&(SystemTime::UNIX_EPOCH + Duration::from_secs(60 * 60 * 24)))
                .unwrap(),
            EorzeaTime::new(1, 1, 21, 13, 42, 51).unwrap()
        );
    }

    #[test]
    pub fn to_system_time() {
        let scenarios = vec![
            0,
            MINUTE_IN_ESEC,
            BELL_IN_ESEC,
            MOON_IN_ESEC,
            YEAR_IN_ESEC,
            YEAR_IN_ESEC * 1000 - 10,
        ];
        for sec in scenarios {
            let time = SystemTime::UNIX_EPOCH + Duration::from_secs(sec);
            let et = EorzeaTime::from_time(&time);
            assert!(et.is_ok());
            assert_eq!(et.unwrap().to_system_time(), time)
        }
    }
}
