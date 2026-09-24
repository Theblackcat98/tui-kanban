use std::time::{Duration, Instant};

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
    pub fn new(kind: AnimationKind, duration: Duration) -> Self {
        Self {
            kind,
            started_at: Instant::now(),
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

    pub fn start(&mut self, kind: AnimationKind, duration: Duration) {
        if self.settings.enabled {
            self.animations.push(Animation::new(kind, duration));
        }
    }

    pub fn is_active(&self, now: Instant) -> bool {
        self.settings.enabled
            && self
                .animations
                .iter()
                .any(|animation| !animation.is_finished(now))
    }

    pub fn next_frame_timeout(&self, now: Instant) -> Duration {
        if !self.settings.enabled {
            return Duration::from_millis(100);
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
            .unwrap_or_else(|| Duration::from_millis(100))
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

pub fn ease_in_out_cubic(value: f32) -> f32 {
    if value < 0.5 {
        4.0 * value * value * value
    } else {
        1.0 - ((-2.0 * value + 2.0).powi(3) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_engine_does_not_schedule_work() {
        let mut engine = AnimationEngine::new(AnimationSettings { enabled: false });
        engine.start(AnimationKind::Modal, Duration::from_millis(200));
        assert!(!engine.is_active(Instant::now()));
        assert_eq!(engine.progress(AnimationKind::Modal, Instant::now()), None);
    }

    #[test]
    fn enabled_engine_reports_progress() {
        let mut engine = AnimationEngine::new(AnimationSettings { enabled: true });
        let now = Instant::now();
        engine.start(AnimationKind::Drawer, Duration::from_millis(100));
        assert!(engine.is_active(now));
        let progress = engine.progress(AnimationKind::Drawer, now).unwrap();
        assert!((0.0..=1.0).contains(&progress));
        engine.tick(now + Duration::from_millis(101));
        assert!(!engine.is_active(now + Duration::from_millis(101)));
    }

    #[test]
    fn easing_stays_within_unit_range() {
        for step in 0..=10 {
            let value = step as f32 / 10.0;
            assert!((0.0..=1.0).contains(&ease_out_cubic(value)));
            assert!((0.0..=1.0).contains(&ease_in_out_cubic(value)));
        }
    }
}
