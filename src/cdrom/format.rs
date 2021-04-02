const FRAMES_PER_SECOND: usize = 75; //Frames as in disc frames, not video frames. Very different
const BYTES_PER_FRAME: usize = 2048;

pub struct DiscIndex {
    minutes: usize,
    seconds: usize,
    frames: usize,
}

impl DiscIndex {
    pub fn new(minutes: usize, seconds: usize, frames: usize) -> Self {
        Self {
            minutes,
            seconds,
            frames,
        }
    }

    pub fn as_address(&self) -> u32 {
        let total_seconds = self.minutes * 60 + self.seconds;
        let total_frames = total_seconds * FRAMES_PER_SECOND + self.frames;
        (total_frames * BYTES_PER_FRAME) as u32
    }
}
