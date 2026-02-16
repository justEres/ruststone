use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct Timing(Option<Instant>);

impl Timing {
    #[inline]
    pub fn start() -> Self {
        #[cfg(feature = "perf_timing")]
        {
            Self(Some(Instant::now()))
        }
        #[cfg(not(feature = "perf_timing"))]
        {
            Self(None)
        }
    }

    #[inline]
    pub fn ms(&self) -> f32 {
        self.0
            .map(|t| t.elapsed().as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
    }
}
