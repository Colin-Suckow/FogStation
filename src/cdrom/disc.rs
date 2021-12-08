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
    (dec/10*16) + (dec%10)
}

impl DiscIndex {
    pub fn new(minutes: usize, seconds: usize, sectors: usize) -> Self {
        Self {
            minutes: bcd_to_dec(minutes),
            seconds: bcd_to_dec(seconds),
            sectors: bcd_to_dec(sectors)
        }
    }

    pub fn new_dec(minutes: usize, seconds: usize, sectors: usize) -> Self {
        Self {
            minutes: minutes,
            seconds: seconds,
            sectors: sectors
        }
    }

    pub fn as_address(&self) -> u32 {
        let total_seconds = (self.minutes * 60) +self.seconds;
        let total_frames = ((total_seconds * SECTORS_PER_SECOND) + self.sectors) - 150;
        //println!(">>>>>>> Self {:?} SECTOR {}", self, total_frames);
        (total_frames * BYTES_PER_SECTOR) as u32
    }

    pub fn plus_sector_offset(&self, offset_sectors: usize) -> DiscIndex {
        let sectors = (self.sectors + offset_sectors) % 75;
        let raw_seconds = self.seconds + ((self.sectors + offset_sectors) / SECTORS_PER_SECOND);
        let seconds = raw_seconds % 60;
        let minutes = self.minutes + (raw_seconds / 60);
        DiscIndex::new_dec(minutes, seconds, sectors)
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

    pub fn read_sector(&self, location: DiscIndex, sector_size: &SectorSize) -> &[u8] {
        let address = location.as_address() as usize;
        let (track, track_offset) = self.track_of_offset(address as usize);
        let sector_address = address - track_offset;
        let data = match sector_size {
            SectorSize::DataOnly => &track.data[(sector_address + 24)..sector_address + 24 + *sector_size as usize],
            SectorSize::WholeSector => &track.data[sector_address..sector_address + *sector_size as usize],
        };
        //println!("data Byte 0 {:#X}", data[0]);
        //println!("Reading sector from address {}. Sector mode: {} Sector size {:?}", address, track.data[address + 15], sector_size);
        //println!("According to the sector header, this is <BCD (DEC)> M: {} ({}) S: {} ({}) F: {} ({})", track.data[(address - track_offset) + 12], bcd_to_dec(track.data[(address - track_offset) + 12] as usize), track.data[(address - track_offset) + 13], bcd_to_dec(track.data[(address - track_offset) + 13] as usize), track.data[(address - track_offset) + 14], bcd_to_dec(track.data[(address - track_offset) + 14] as usize));
        data
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

