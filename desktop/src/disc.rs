use psx_emu::cdrom::disc::{Disc, DiscTrack};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use cue::cd::CD;


pub fn load_disc_from_cuesheet(cuesheet_path: PathBuf) -> Disc {
    let mut cue_dir = cuesheet_path.clone();

    let cd = match CD::parse_file(cuesheet_path) {
        Ok(cd) => cd,
        Err(e) => panic!("Unable to open cue sheet! Error: {}", e),
    };

    let mut disc = Disc::new(cue_dir.file_name().unwrap().to_str().unwrap());
    cue_dir.pop();

    for track in cd.tracks() {
        let mut track_path = cue_dir.clone();
        let track_name = track.get_filename();
        track_path.push(Path::new(&track_name));
        let mut file = File::open(track_path).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        disc.add_track(DiscTrack::new(data));
    }
    disc
}
