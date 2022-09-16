use std::fmt::Display;

use super::SectorSize;

pub(super) const SECTORS_PER_SECOND: usize = 75;
pub(super) const BYTES_PER_SECTOR: usize = 2352;
// Sector format is Mode2/Form1 CD-XA

#[derive(Debug, Clone, Copy)]
pub struct DiscIndex {
    minutes: usize,
    seconds: usize,
    sectors: usize,
}

pub fn bcd_to_dec(hex: usize) -> usize {
    ((hex & 0xF0) >> 4) * 10 + (hex & 0x0F)
}

pub fn dec_to_bcd(dec: usize) -> usize {
    (dec / 10 * 16) + (dec % 10)
}

impl DiscIndex {
    pub fn new_bcd(minutes: usize, seconds: usize, sectors: usize) -> Self {
        Self {
            minutes: bcd_to_dec(minutes),
            seconds: bcd_to_dec(seconds),
            sectors: bcd_to_dec(sectors),
        }
    }

    pub fn new_dec(minutes: usize, seconds: usize, sectors: usize) -> Self {
        Self {
            minutes: minutes,
            seconds: seconds,
            sectors: sectors,
        }
    }

    pub fn sector_number(&self) -> usize {
        let total_seconds = (self.minutes * 60) + self.seconds;
        ((total_seconds * SECTORS_PER_SECOND) + self.sectors) - 150
    }

    pub fn as_address(&self) -> u32 {
        (self.sector_number() * BYTES_PER_SECTOR) as u32
    }

    pub fn plus_sector_offset(&self, offset_sectors: usize) -> DiscIndex {
        let sectors = (self.sectors + offset_sectors) % 75;
        let raw_seconds = self.seconds + ((self.sectors + offset_sectors) / SECTORS_PER_SECOND);
        let seconds = raw_seconds % 60;
        let minutes = self.minutes + (raw_seconds / 60);
        DiscIndex::new_dec(minutes, seconds, sectors)
    }
}

impl Display for DiscIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.minutes, self.seconds, self.sectors)
    }
}

pub struct DiscTrack {
    data: Vec<u8>,
}

impl DiscTrack {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
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

    pub fn read_sector(&self, location: DiscIndex) -> Sector {
        let address = location.as_address() as usize;
        let (track, track_offset) = self.track_of_offset(address as usize);
        let sector_address = address - track_offset;
        let data = &track.data[sector_address..sector_address + SectorSize::WholeSector as usize];
        Sector::new(data.to_vec())
    }

    fn track_of_offset(&self, offset: usize) -> (&DiscTrack, usize) {
        let mut total_size = 0;
        for track in &self.tracks {
            if offset >= total_size && offset < total_size + track.data.len() {
                return (&track, total_size);
            }
            total_size += track.data.len();
        }
        panic!("Unable to locate track at offset {}!", offset);
    }

    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }
}

pub struct Sector {
    data: Vec<u8>,
}

impl Sector {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn index(&self) -> DiscIndex {
        DiscIndex::new_bcd(
            self.data[12].into(),
            self.data[13].into(),
            self.data[14].into(),
        )
    }

    pub fn full_sector_data(&self) -> &[u8] {
        &self.data[0xC..]
    }

    pub fn data_only(&self) -> &[u8] {
        &self.data[24..24 + 0x800]
    }

    pub fn consume(self, sector_size: &SectorSize) -> Vec<u8> {
        match sector_size {
            SectorSize::DataOnly => self.data[24..24 + 0x800].to_vec(),
            SectorSize::WholeSector => self.data[0xC..].to_vec(),
        }
    }
}
