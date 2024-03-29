use super::{disc::dec_to_bcd, CDDrive, DriveState, IntCause, MotorState, Packet};
use crate::cdrom::{disc::DiscIndex, DriveSpeed};

pub(super) const AVG_FIRST_RESPONSE_TIME: u32 = 0xc4e1;

pub(super) fn get_bios_date(state: &mut CDDrive) -> Packet {
    Packet {
        internal_id: state.next_packet_id(),
        cause: IntCause::INT3,
        response: vec![0x94, 0x09, 0x19, 0xC0], //PSX (PU-7) rev a
        execution_cycles: AVG_FIRST_RESPONSE_TIME,
        extra_response: None,
        command: 0x19,
        need_irq: false,
    }
}

fn stat(state: &mut CDDrive, command: u8) -> Packet {
    //TODO: Error handling

    Packet {
        internal_id: state.next_packet_id(),
        cause: IntCause::INT3,
        response: vec![state.get_stat()],
        execution_cycles: AVG_FIRST_RESPONSE_TIME,
        extra_response: None,
        command,
        need_irq: false,
    }
}

pub(super) fn get_stat(state: &mut CDDrive) -> Packet {
    stat(state, 0x19)
}

pub(super) fn get_id(state: &mut CDDrive) -> Packet {
    //Only handles 'No Disk' and 'Licensed Game' states
    if state.disc.is_some() {
        let mut first_response = stat(state, 0x1a);
        let second_response = Packet {
            internal_id: state.next_packet_id(),
            cause: IntCause::INT2,
            response: vec![state.get_stat(), 0x00, 0x20, 0x00, 0x53, 0x43, 0x45, 0x41], //SCEA disk inserted
            execution_cycles: 0x4a00,
            extra_response: None,
            command: 0x1a,
            need_irq: false,
        };
        first_response.extra_response = Some(Box::new(second_response));
        first_response
    } else {
        let mut first_response = stat(state, 0x1a);
        let second_response = Packet {
            internal_id: state.next_packet_id(),
            cause: IntCause::INT5,
            response: vec![0x08, 0x40, 0, 0, 0, 0, 0, 0], //No disk
            execution_cycles: 0x4a00,
            extra_response: None,
            command: 0x1a,
            need_irq: false,
        };
        first_response.extra_response = Some(Box::new(second_response));
        first_response
    }
}

pub(super) fn init(state: &mut CDDrive) -> Packet {
    let mut first_response = stat(state, 0x0a);
    state.motor_state = MotorState::On;
    state.drive_mode = 0;
    let mut second_response = stat(state, 0x0a);
    second_response.cause = IntCause::INT2;
    //second_response.execution_cycles = 2500000;
    first_response.execution_cycles = 0x13cce;
    first_response.extra_response = Some(Box::new(second_response));

    first_response
}

pub(super) fn set_loc(state: &mut CDDrive, minutes: u8, seconds: u8, frames: u8) -> Packet {
    state.next_seek_target =
        DiscIndex::new_bcd(minutes as usize, seconds as usize, frames as usize);
    state.seek_complete = false;
    state.data_queue.clear();
    //println!("set_loc to {}", state.next_seek_target);

    //println!("set_loc to {:?}, total sectors: {}", state.seek_target, state.seek_target.as_address() / BYTES_PER_SECTOR as u32);
    let main_response = stat(state, 0x2);

    main_response
}

//Listed in psx-spx as SeekL
pub(super) fn seek_data(state: &mut CDDrive) -> Packet {
    state.drive_state = DriveState::Idle;
    let mut second_response = stat(state, 0x15);
    second_response.execution_cycles = AVG_FIRST_RESPONSE_TIME;

    state.read_offset = 0;
    state.current_seek_target = state.next_seek_target.clone();
    state.seek_complete = true;

    state.drive_state = DriveState::Seek;
    let mut first_response = stat(state, 0x15);
    second_response.cause = IntCause::INT2;
    second_response.execution_cycles = 10000;
    first_response.extra_response = Some(Box::new(second_response));
    first_response
}

pub(super) fn set_mode(state: &mut CDDrive, mode: u8) -> Packet {
    state.drive_mode = mode;
    stat(state, 0xE)
}

//ReadN
//This is only the initial return. All of the reading is handled in the post condition
//It's messy, but it works for now
pub(super) fn read_with_retry(state: &mut CDDrive) -> Packet {
    let mut initial_response = stat(state, 0x6);
    state.drive_state = DriveState::Read;
    state.read_enabled = true;
    state.data_queue.clear();

    // let cycles = match state.drive_speed() {
    //     DriveSpeed::Single => 0x686da,
    //     DriveSpeed::Double => 0x322df,
    // } * 5;

    let cycles = 0x35CA8;

    if !state.seek_complete {
        state.read_offset = 0;
        state.current_seek_target = state.next_seek_target.clone();
        state.seek_complete = true;
    }

    let response_packet = Packet {
        internal_id: state.next_packet_id(),
        cause: IntCause::INT1,
        response: vec![state.get_stat()],
        execution_cycles: cycles,
        extra_response: None,
        command: 0x6,
        need_irq: false,
    };
    initial_response.execution_cycles = 42430;
    initial_response.extra_response = Some(Box::new(response_packet));

    initial_response
}

//Pause
pub(super) fn pause_read(state: &mut CDDrive) -> Packet {
    //println!("stop read (pause)");
    let mut initial_response = stat(state, 0x9);

    let cycles = if state.drive_state == DriveState::Idle {
        0x1df2 // pausing is much faster when already paused
    } else {
        match state.drive_speed() {
            DriveSpeed::Double => 0x10bd93,
            DriveSpeed::Single => 0x21181c,
        }
    };

    state.drive_state = DriveState::Idle;
    //state.read_offset = 0;
    state.read_enabled = false;

    let response_packet = Packet {
        internal_id: state.next_packet_id(),
        cause: IntCause::INT2,
        response: vec![state.get_stat()],
        execution_cycles: cycles * 6,
        extra_response: None,
        command: 0x9,
        need_irq: false,
    };
    initial_response.execution_cycles = 28648;

    initial_response.extra_response = Some(Box::new(response_packet));
    initial_response
}

pub(super) fn demute(state: &mut CDDrive) -> Packet {
    stat(state, 0xC)
}

// Get number of tracks in session
// Assumes theres only one session
pub(super) fn get_tn(state: &mut CDDrive) -> Packet {
    let first_track = 0x1;
    let last_track = dec_to_bcd(
        state
            .disc
            .as_ref()
            .expect("Tried to read non-existent disc!")
            .track_count()
            + 1,
    );

    let mut initial_response = stat(state, 0x13);

    initial_response.response.push(first_track);
    initial_response.response.push(last_track as u8);

    initial_response
}

// Get starting index of given track
// Because I'm lazy I'm just going to return the start of the first track, 00:02
// In practice this will probably send code instead of music to the SPU, and play some crazy audio
// Future colin, you have been warned
pub(super) fn get_td(state: &mut CDDrive, _track: u8) -> Packet {
    let mut initial_response = stat(state, 0x14);
    initial_response.response.push(0x0);
    initial_response.response.push(0x2);

    initial_response
}

pub(super) fn play(state: &mut CDDrive) -> Packet {
    state.drive_state = DriveState::Play;
    stat(state, 0x3)
}

pub(super) fn mute(state: &mut CDDrive) -> Packet {
    stat(state, 0xB)
}

// Slams the brakes on the drive completely
pub(super) fn stop(state: &mut CDDrive) -> Packet {
    let mut pre_stop_packet = stat(state, 0x8);
    state.drive_state = DriveState::Idle;
    state.motor_state = MotorState::Off;
    let mut second_packet = stat(state, 0x8);

    second_packet.cause = IntCause::INT2;
    pre_stop_packet.extra_response = Some(Box::new(second_packet));
    pre_stop_packet
}

// Filters out some sectors for playing music. We don't care about that here
pub(super) fn set_filter(state: &mut CDDrive) -> Packet {
    stat(state, 0xD)
}

// Command is to reread the table of contents but we don't do this anyways so just return the expected responses
pub(super) fn get_toc(state: &mut CDDrive) -> Packet {
    let mut resp1 = stat(state, 0x1e);
    let mut resp2 = stat(state, 0x1e);

    resp2.cause = IntCause::INT2;
    
    resp1.extra_response = Some(Box::new(resp2));

    resp1
}

// Subcommand 0x4: Start SCEx reading and reset counters
// Tells the cd drive to start looking for "SCEx" copy protection strings
// We don't need to do anything
pub(super) fn start_sce(state: &mut CDDrive) -> Packet {
    state.motor_state = MotorState::On;
    stat(state, 0x19)
}

// Subcommand 0x5: Stop SCEx reading and get counters
// Returns the results of the scex check started by start_sce
// Just return a successful check
pub(super) fn end_sce(state: &mut CDDrive) -> Packet {
    Packet {
        internal_id: state.next_packet_id(),
        cause: IntCause::INT3,
        // PSX-SPX says [0x1,0x1] for licensed disks. [0x0,0x0] for unlicensed
        // However yugioh only boots with [0x0, 0x0]
        response: vec![0x0, 0x0],
        execution_cycles: AVG_FIRST_RESPONSE_TIME,
        extra_response: None,
        command: 0x19,
        need_irq: false,
    }
}