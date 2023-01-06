use psx_emu::cdrom::disc::{Disc, DiscTrack};
use rcue::parser::parse_from_file;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn load_disc_from_cuesheet(cuesheet_path: PathBuf) -> Disc {
    let mut cue_dir = cuesheet_path.clone();

    let cue = parse_from_file(cuesheet_path.to_str().unwrap(), true).unwrap();

    let mut disc = Disc::new(cue_dir.file_name().unwrap().to_str().unwrap());
    cue_dir.pop();

    for file in &cue.files {
        let mut track_path = cue_dir.clone();
        let track_name = file.file.clone();
        track_path.push(Path::new(&track_name));
        let mut file = File::open(track_path).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        disc.add_track(DiscTrack::new(data));
    }
    disc
}
