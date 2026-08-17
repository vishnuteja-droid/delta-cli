//! The ASCII/braille sprite: a small animated character with three
//! states — idle, working, done — cycled at 10fps. Frame data lives
//! here purely as static lookup tables so `tui::app`'s dirty-tracking
//! only needs "did the frame index change", never "what does frame N
//! look like" — that question stays fully contained to this module.

use std::time::Duration;

/// One frame advance per this interval — 10fps, per `PLAN.md`.
pub const FRAME_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpriteState {
    Idle,
    Working,
    Done,
}

const IDLE_FRAMES: &[&str] = &["⠁", "⠂", "⠄", "⠂"];
const WORKING_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const DONE_FRAMES: &[&str] = &["(•‿•)", "(-‿-)"];

/// The glyph for `state` at `index`, wrapping around that state's own
/// frame count — callers don't need to know how many frames a state has.
pub fn frame(state: SpriteState, index: usize) -> &'static str {
    let frames = match state {
        SpriteState::Idle => IDLE_FRAMES,
        SpriteState::Working => WORKING_FRAMES,
        SpriteState::Done => DONE_FRAMES,
    };
    frames[index % frames.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_index_wraps_around_within_each_state() {
        assert_eq!(
            frame(SpriteState::Working, 0),
            frame(SpriteState::Working, WORKING_FRAMES.len())
        );
        assert_eq!(
            frame(SpriteState::Idle, 0),
            frame(SpriteState::Idle, IDLE_FRAMES.len())
        );
        assert_eq!(
            frame(SpriteState::Done, 0),
            frame(SpriteState::Done, DONE_FRAMES.len())
        );
    }

    #[test]
    fn every_state_has_at_least_one_nonempty_frame() {
        for state in [SpriteState::Idle, SpriteState::Working, SpriteState::Done] {
            assert!(!frame(state, 0).is_empty());
        }
    }

    #[test]
    fn working_has_more_frames_than_idle_for_a_visibly_busier_animation() {
        assert!(WORKING_FRAMES.len() > IDLE_FRAMES.len());
    }
}
