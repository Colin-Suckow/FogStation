use super::{IntCause, PendingResponse};

const AVG_FIRST_RESPONSE_TIME: u32 =  0xc4e1;

pub(super) fn get_bios_date() -> PendingResponse {
    PendingResponse {
        cause: IntCause::INT3,
        response: vec![0x94, 0x09, 0x19, 0xC0], //PSX (PU-7) rev a
        execution_cycles: AVG_FIRST_RESPONSE_TIME,
    }
}