use std::time::{Duration, Instant};

/// The frame interval while an animation is running (~30 FPS).
pub const FRAME_INTERVAL: Duration = Duration::from_millis(33);

#[derive(Clone, Copy, Debug, Default)]
pub struct AnimationSettings {
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationKind {
    Selection,
    CardMove,
    Drawer,
    Modal,
    Toast,
}

#[derive(Clone, Debug)]
pub struct Animation {
    kind: AnimationKind,
    started_at: Instant,
    duration: Duration,
}

impl Animation {
    pub fn new(kind: AnimationKind, duration: Duration, now: Instant) -> Self {
        Self {
            kind,
            started_at: now,
            duration,
        }
    }

    pub fn progress(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let elapsed = now.saturating_duration_since(self.started_at);
        (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
    }

    pub fn is_finished(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }

    pub fn kind(&self) -> AnimationKind {
        self.kind
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnimationEngine {
    animations: Vec<Animation>,
    settings: AnimationSettings,
}

impl AnimationEngine {
    pub fn new(settings: AnimationSettings) -> Self {
        Self {
            animations: Vec::new(),
            settings,
        }
    }

    pub fn start(&mut self, kind: AnimationKind, duration: Duration, now: Instant) {
        if self.settings.enabled {
            self.animations.push(Animation::new(kind, duration, now));
        }
    }

    /// How long to wait before drawing the next frame, or `None` when no
    /// animation is running. Frames are at most [`FRAME_INTERVAL`] apart,
    /// and the last one lands when the shortest animation finishes.
    pub fn next_frame_timeout(&self, now: Instant) -> Option<Duration> {
        if !self.settings.enabled {
            return None;
        }
        self.animations
            .iter()
            .filter_map(|animation| {
                let remaining = animation
                    .duration
                    .checked_sub(now.saturating_duration_since(animation.started_at));
                remaining.filter(|duration| !duration.is_zero())
            })
            .min()
            .map(|remaining| remaining.min(FRAME_INTERVAL))
    }

    pub fn progress(&self, kind: AnimationKind, now: Instant) -> Option<f32> {
        self.animations
            .iter()
            .rev()
            .find(|animation| animation.kind == kind)
            .map(|animation| animation.progress(now))
    }

    pub fn finish(&mut self, kind: AnimationKind) {
        self.animations.retain(|animation| animation.kind != kind);
    }

    pub fn tick(&mut self, now: Instant) {
        self.animations
            .retain(|animation| !animation.is_finished(now));
    }
}

pub fn ease_out_cubic(value: f32) -> f32 {
    1.0 - (1.0 - value).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_engine_does_not_schedule_work() {
        let mut engine = AnimationEngine::new(AnimationSettings { enabled: false });
        engine.start(
            AnimationKind::Modal,
            Duration::from_millis(200),
            Instant::now(),
        );
        assert_eq!(engine.next_frame_timeout(Instant::now()), None);
        assert_eq!(engine.progress(AnimationKind::Modal, Instant::now()), None);
    }

    #[test]
    fn enabled_engine_reports_progress() {
        let mut engine = AnimationEngine::new(AnimationSettings { enabled: true });
        let now = Instant::now();
        engine.start(AnimationKind::Drawer, Duration::from_millis(100), now);
        assert!(engine.next_frame_timeout(now).is_some());
        let progress = engine.progress(AnimationKind::Drawer, now).unwrap();
        assert!((0.0..=1.0).contains(&progress));
        engine.tick(now + Duration::from_millis(101));
        assert_eq!(
            engine.next_frame_timeout(now + Duration::from_millis(101)),
            None
        );
    }

    #[test]
    fn frames_are_at_most_one_interval_apart() {
        let mut engine = AnimationEngine::new(AnimationSettings { enabled: true });
        let now = Instant::now();
        assert_eq!(engine.next_frame_timeout(now), None);
        engine.start(AnimationKind::Modal, Duration::from_millis(180), now);
        assert_eq!(engine.next_frame_timeout(now), Some(FRAME_INTERVAL));
        let near_end = now + Duration::from_millis(170);
        assert_eq!(
            engine.next_frame_timeout(near_end),
            Some(Duration::from_millis(10))
        );
        assert_eq!(
            engine.next_frame_timeout(now + Duration::from_millis(200)),
            None
        );
    }

    #[test]
    fn easing_stays_within_unit_range() {
        for step in 0..=10 {
            let value = step as f32 / 10.0;
            assert!((0.0..=1.0).contains(&ease_out_cubic(value)));
        }
    }
}
