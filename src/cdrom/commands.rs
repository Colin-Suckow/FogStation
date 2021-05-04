use super::{CDDrive, DriveState, IntCause, MotorState, Packet, Block};
use crate::cdrom::disc::DiscIndex;

pub(super) const AVG_FIRST_RESPONSE_TIME: u32 = 0xc4e1;
pub(super) const AVG_SECOND_RESPONSE_TIME: u32 = 0x1000;

pub(super) fn get_bios_date() -> Packet {
    Packet {
        cause: IntCause::INT3,
        response: vec![0x94, 0x09, 0x19, 0xC0], //PSX (PU-7) rev a
        execution_cycles: AVG_FIRST_RESPONSE_TIME,
        extra_response: None,
        command: 0x19,
    }
}

fn stat(state: &CDDrive, command: u8) -> Packet {
    //TODO: Error handling

    Packet {
        cause: IntCause::INT3,
        response: vec![state.get_stat()],
        execution_cycles: AVG_FIRST_RESPONSE_TIME,
        extra_response: None,
        command
    }
}

pub(super) fn get_stat(state: &CDDrive) -> Packet {
    stat(state, 0x1)
}

pub(super) fn get_id(state: &CDDrive) -> Packet {
    //Only handles 'No Disk' and 'Licensed Game' states
    if state.disc.is_some() {
        let mut first_response = stat(state, 0x1a);
        let second_response = Packet {
            cause: IntCause::INT2,
            response: vec![state.get_stat(), 0x00, 0x20, 0x00, 0x53, 0x43, 0x45, 0x41], //SCEA disk inserted
            execution_cycles: AVG_SECOND_RESPONSE_TIME,
            extra_response: None,
            command: 0x1a,
        };
        first_response.extra_response = Some(Box::new(second_response));
        first_response
    } else {
        let mut first_response = stat(state, 0x1a);
        let second_response = Packet {
            cause: IntCause::INT5,
            response: vec![0x08, 0x40, 0, 0, 0, 0, 0, 0], //No disk
            execution_cycles: AVG_SECOND_RESPONSE_TIME,
            extra_response: None,
            command: 0x1a
        };
        first_response.extra_response = Some(Box::new(second_response));
        first_response
    }
}

pub(super) fn init(state: &mut CDDrive) -> Packet {
    state.motor_state = MotorState::On;
    let mut first_response = stat(state, 0x0a);
    let mut second_response = stat(state, 0x0a);
    second_response.cause = IntCause::INT2;
    first_response.extra_response = Some(Box::new(second_response));
    first_response
}

pub(super) fn set_loc(state: &mut CDDrive, minutes: u8, seconds: u8, frames: u8) -> Packet {
    state.seek_target = DiscIndex::new(minutes as usize, seconds as usize, frames as usize);
    state.seek_complete = false;
    stat(state, 0x2)
}

//Listed in psx-spx as SeekL
pub(super) fn seek_data(state: &mut CDDrive) -> Packet {
    state.drive_state = DriveState::Idle;
    let mut second_response = stat(state, 0x15);
    second_response.execution_cycles = AVG_FIRST_RESPONSE_TIME;

    state.drive_state = DriveState::Seek;
    let mut first_response = stat(state, 0x15);
    second_response.cause = IntCause::INT2;
    second_response.execution_cycles = 120000;
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
    let mut response_packet = Packet {
        cause: IntCause::INT1,
        response: vec![state.get_stat()],
        execution_cycles: 3_000_000,
        extra_response: None,
        command: 0x6,
    };


    initial_response.execution_cycles = AVG_FIRST_RESPONSE_TIME;
    initial_response.extra_response = Some(Box::new(response_packet));

    initial_response
}

//Pause
pub(super) fn stop_read(state: &mut CDDrive) -> Packet {
    println!("stop read (pause)");
    let mut initial_response = stat(state, 0x9);
    state.drive_state = DriveState::Idle;
    state.read_enabled = false;
    // match &state.pending_responses[0] {
    //     Response::Packet(packet) => {
    //         if packet.command == 0x6 {
    //             //state.pending_responses.clear();
    //         }
    //     }
    //     Response::Datablock(_) => (),//state.pending_responses.clear(),
    // };
    
    let response_packet = Packet {
        cause: IntCause::INT2,
        response: vec![state.get_stat()],
        execution_cycles: 1_000_000,
        extra_response: None,
        command: 0x9,
    };
    initial_response.execution_cycles = AVG_FIRST_RESPONSE_TIME;

    initial_response.extra_response = Some(Box::new(response_packet));
    initial_response
}
