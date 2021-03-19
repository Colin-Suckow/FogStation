use super::{CDDrive, DriveState, IntCause, MotorState, PendingResponse};

const AVG_FIRST_RESPONSE_TIME: u32 =  0xc4e1;
const AVG_SECOND_RESPONSE_TIME: u32 =  0xc4e1;

pub(super) fn get_bios_date() -> PendingResponse {
    PendingResponse {
        cause: IntCause::INT3,
        response: vec![0x94, 0x09, 0x19, 0xC0], //PSX (PU-7) rev a
        execution_cycles: AVG_FIRST_RESPONSE_TIME,
        extra_response: None,
    }
}

pub(super) fn get_stat(state: &CDDrive) -> PendingResponse {
    //TODO: Error handling

    PendingResponse {
        cause: IntCause::INT3,
        response: vec![state.get_stat()],
        execution_cycles: AVG_FIRST_RESPONSE_TIME,
        extra_response: None,
    }
}

pub(super) fn get_id(state: &CDDrive) -> PendingResponse {
    //Only handles 'No Disk' and 'Licensed Game' states
    if state.disk_inserted {
        let mut first_response = get_stat(state);
        let second_response = PendingResponse {
            cause: IntCause::INT2,
            response: vec![state.get_stat(),0x00, 0x20,0x00, 0x53,0x43,0x45,0x41], //SCEA disk inserted
            execution_cycles: AVG_SECOND_RESPONSE_TIME,
            extra_response: None,
        };
        first_response.extra_response = Some(Box::new(second_response));
        first_response
    } else {
        let mut first_response = get_stat(state);
        let second_response = PendingResponse {
            cause: IntCause::INT5,
            response: vec![0x08, 0x40, 0, 0, 0, 0, 0, 0], //No disk
            execution_cycles: AVG_SECOND_RESPONSE_TIME,
            extra_response: None,
        };
        first_response.extra_response = Some(Box::new(second_response));
        first_response
    }
}

pub(super) fn init(state: &mut CDDrive) -> PendingResponse {
    state.motor_state = MotorState::On;
    let mut first_response = get_stat(state);
    let second_response = get_stat(state);
    first_response.extra_response = Some(Box::new(second_response));
    first_response
}