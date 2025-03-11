use std::{
    cmp::{max, min},
    fmt,
    ops::Rem,
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};

pub const EORZEA_WEATHER_PERIOD: EorzeaDuration = EorzeaDuration {
    esec: BELL_IN_ESEC * 8,
};
pub const EORZEA_SUN: EorzeaDuration = EorzeaDuration { esec: SUN_IN_ESEC };

const EORZEA_TIME_CONST: f64 = 3600.0 / 175.0;

pub const YEAR_IN_ESEC: u64 = 12 * MOON_IN_ESEC;
pub const MOON_IN_ESEC: u64 = 32 * SUN_IN_ESEC;
pub const SUN_IN_ESEC: u64 = 24 * BELL_IN_ESEC;
pub const BELL_IN_ESEC: u64 = 60 * MINUTE_IN_ESEC;
pub const MINUTE_IN_ESEC: u64 = 60;

pub const EORZEA_ZERO_TIME: EorzeaTime = EorzeaTime { timestamp: 0 };
pub const EORZEA_ZERO_TIMESPAN: EorzeaTimeSpan = EorzeaTimeSpan {
    start: EORZEA_ZERO_TIME,
    duration: EorzeaDuration { esec: 0 },
};

#[derive(Debug, PartialEq, Clone, Copy, PartialOrd, Eq, Ord)]
pub struct EorzeaTime {
    timestamp: u64,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct EorzeaDuration {
    esec: u64,
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

    pub fn round(&mut self, d: EorzeaDuration) {
        self.timestamp -= self.timestamp % d.esec;
    }

    fn duration_since(&self, other: EorzeaTime) -> Result<EorzeaDuration, EorzeaDurationError> {
        if other.timestamp > self.timestamp {
            return Err(EorzeaDurationError);
        }
        Ok(EorzeaDuration::from_esecs(self.timestamp - other.timestamp))
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
            timestamp: self.timestamp + rhs.esec,
        }
    }
}

impl std::ops::Sub<EorzeaDuration> for EorzeaTime {
    type Output = Self;

    fn sub(self, rhs: EorzeaDuration) -> Self::Output {
        if self.timestamp < rhs.esec {
            return EorzeaTime { timestamp: 0 };
        }
        EorzeaTime {
            timestamp: self.timestamp - rhs.esec,
        }
    }
}

impl std::ops::AddAssign<EorzeaDuration> for EorzeaTime {
    fn add_assign(&mut self, rhs: EorzeaDuration) {
        self.timestamp += rhs.esec;
    }
}

impl std::ops::SubAssign<EorzeaDuration> for EorzeaTime {
    fn sub_assign(&mut self, rhs: EorzeaDuration) {
        if self.timestamp < rhs.esec {
            return;
        }
        self.timestamp -= rhs.esec;
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
        EorzeaTime::new(year + 1, moon + 1, sun + 1, bell, minute, second)
            .map(|et| EorzeaDuration { esec: et.timestamp })
    }

    pub fn new(
        bell: u8,
        minute: u8,
        second: u8,
    ) -> Result<EorzeaDuration, EorzeaTimeCreationError> {
        EorzeaTime::new(1, 1, 1, bell, minute, second)
            .map(|et| EorzeaDuration { esec: et.timestamp })
    }

    pub fn from_esecs(esec: u64) -> EorzeaDuration {
        EorzeaDuration { esec }
    }

    pub fn total_seconds(&self) -> u64 {
        self.esec
    }

    pub fn year(&self) -> u16 {
        (1 + self.esec / YEAR_IN_ESEC) as u16
    }
    pub fn moon(&self) -> u8 {
        (1 + self.esec / MOON_IN_ESEC % 12) as u8
    }
    pub fn sun(&self) -> u8 {
        (1 + self.esec / SUN_IN_ESEC % 32) as u8
    }
    pub fn bell(&self) -> u8 {
        (self.esec / BELL_IN_ESEC % 24) as u8
    }
    pub fn minute(&self) -> u8 {
        (self.esec / MINUTE_IN_ESEC % 60) as u8
    }
    pub fn second(&self) -> u8 {
        (self.esec % 60) as u8
    }
}

#[derive(Debug, PartialEq)]
pub struct EorzeaDurationError;

#[derive(Debug, PartialEq)]
pub struct EorzeaTimeSpan {
    start: EorzeaTime,
    duration: EorzeaDuration,
}

impl EorzeaTimeSpan {
    pub fn new(start: EorzeaTime, duration: EorzeaDuration) -> EorzeaTimeSpan {
        EorzeaTimeSpan { start, duration }
    }
    pub fn new_start_end(
        start: EorzeaTime,
        end: EorzeaTime,
    ) -> Result<EorzeaTimeSpan, EorzeaDurationError> {
        end.duration_since(start)
            .map(|d| EorzeaTimeSpan { start, duration: d })
    }

    pub fn start(&self) -> EorzeaTime {
        self.start
    }

    pub fn duration(&self) -> EorzeaDuration {
        self.duration
    }

    pub fn end(&self) -> EorzeaTime {
        self.start + self.duration
    }

    pub fn overlap(&self, other: &EorzeaTimeSpan) -> Result<EorzeaTimeSpan, EorzeaDurationError> {
        let max_start = max(self.start, other.start);
        let min_end = min(self.end(), other.end());
        EorzeaTimeSpan::new_start_end(max_start, min_end)
    }
}
impl std::fmt::Display for EorzeaDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:0>2}:{:0>2}:{:0>2}",
            self.esec / BELL_IN_ESEC,
            (self.esec % BELL_IN_ESEC) / MINUTE_IN_ESEC,
            self.esec % MINUTE_IN_ESEC
        )
    }
}

impl std::fmt::Display for EorzeaTimeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} for {}", self.start, self.duration)
    }
}

impl Rem for EorzeaDuration {
    type Output = Self;
    fn rem(self, rhs: Self) -> Self {
        EorzeaDuration {
            esec: self.esec % rhs.esec,
        }
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
    pub fn eorzea_time_to_system_time() {
        let scenarios = vec![
            0,
            MINUTE_IN_ESEC,
            BELL_IN_ESEC,
            MOON_IN_ESEC,
            YEAR_IN_ESEC,
            YEAR_IN_ESEC * 1000 - 10,
            2000 * YEAR_IN_ESEC - 1,
        ];
        for sec in scenarios {
            let time = SystemTime::UNIX_EPOCH + Duration::from_secs(sec);
            let et = EorzeaTime::from_time(&time);
            assert!(et.is_ok());
            assert_eq!(et.unwrap().to_system_time(), time)
        }
    }

    #[test]
    pub fn eorzea_time_span() {
        let time_span =
            EorzeaTimeSpan::new(EorzeaTime::from_esecs(0), EorzeaDuration::from_esecs(1));
        assert_eq!(time_span.end(), EorzeaTime::from_esecs(1));
    }

    #[test]
    pub fn eorzea_time_span_new_start_end() {
        let start = EorzeaTime::from_esecs(0);
        let end = EorzeaTime::from_esecs(1);
        let result = EorzeaTimeSpan::new_start_end(start, end);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().duration, EorzeaDuration::from_esecs(1));

        assert!(EorzeaTimeSpan::new_start_end(end, start).is_err())
    }

    #[test]
    pub fn eorzea_time_span_overlap() {
        let span1 = EorzeaTimeSpan::new(EorzeaTime::from_esecs(0), EorzeaDuration::from_esecs(1));
        let span2 = EorzeaTimeSpan::new(EorzeaTime::from_esecs(0), EorzeaDuration::from_esecs(2));
        assert_eq!(span1.overlap(&span2), span2.overlap(&span1));
        let overlap = span1.overlap(&span2);
        assert!(overlap.is_ok());
        let o = overlap.unwrap();
        assert_eq!(o.start(), EorzeaTime::from_esecs(0));
        assert_eq!(o.end(), EorzeaTime::from_esecs(1));

        let span3 = EorzeaTimeSpan::new(EorzeaTime::from_esecs(1), EorzeaDuration::from_esecs(2));
        let overlap = span1.overlap(&span3);
        assert!(overlap.is_ok());
        let o = overlap.unwrap();
        assert_eq!(o.start(), EorzeaTime::from_esecs(1));
        assert_eq!(o.end(), EorzeaTime::from_esecs(1));

        let span4 = EorzeaTimeSpan::new(EorzeaTime::from_esecs(2), EorzeaDuration::from_esecs(1));
        assert!(span1.overlap(&span4).is_err());
    }
}
