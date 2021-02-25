use super::{IntCause, PendingResponse};

pub(super) fn get_bios_date() -> PendingResponse {
    PendingResponse {
        cause: IntCause::INT3,
        response: vec![0, 0, 0, 0], //Date/version code is obviously invalid. Hopefully this doesn't break anything
        execution_cycles: 10000, //IDK what this should be yet
    }
}