pub(super) const SECTORS_PER_SECOND: usize = 75;
pub(super) const BYTES_PER_SECTOR: usize = 2048;

pub struct DiscIndex {
    minutes: usize,
    seconds: usize,
    sectors: usize,
}

impl DiscIndex {
    pub fn new(minutes: usize, seconds: usize, sectors: usize) -> Self {
        Self {
            minutes,
            seconds,
            sectors
        }
    }

    pub fn as_address(&self) -> u32 {
        let total_seconds = self.minutes * 60 + self.seconds;
        let total_frames = total_seconds * SECTORS_PER_SECOND + self.sectors;
        (total_frames * BYTES_PER_SECTOR) as u32
    }

    pub fn plus_sector_offset(&self, offset_sectors: usize) -> DiscIndex {
        let sectors = self.sectors.wrapping_add(offset_sectors);
        let raw_seconds = self.seconds + ((self.sectors + offset_sectors) / SECTORS_PER_SECOND);
        let seconds = raw_seconds % 60;
        let minutes = self.minutes + (raw_seconds / 60);
        DiscIndex::new(minutes, seconds, sectors)
    }
}

pub struct DiscTrack {
    data: Vec<u8>,
}

impl DiscTrack {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data
        }
    }
}

pub struct Disc {
    tracks: Vec<DiscTrack>,
    title: String,
}

impl Disc {
    pub fn new(title: &str) -> Self {
        Self {
            tracks: Vec::new(),
            title: String::from(title),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn add_track(&mut self, track: DiscTrack) {
        self.tracks.push(track);
    }
}

