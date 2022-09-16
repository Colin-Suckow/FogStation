use super::MdecCommand;

#[derive(Clone, Copy)]
pub(crate) struct SetScaleTableCommand;

impl SetScaleTableCommand {
    pub(crate) fn new(command_word: u32) -> Self {
        if command_word >> 29 != 3 {
            panic!(
                "Not a set_scale_table command! Command number = {}",
                command_word >> 29
            );
        };
        Self
    }
}

impl MdecCommand for SetScaleTableCommand {
    fn parameter_words(&self) -> usize {
        32
    }

    fn execute(&self, ctx: &mut super::MDEC) {
        ctx.scale_table.clear();
        for i in 0..32 {
            ctx.scale_table.push((ctx.parameter_buffer[i] & 0xFFFF) as i16);
            ctx.scale_table.push((ctx.parameter_buffer[i] >> 16) as i16);
        }
    }

    fn box_clone(&self) -> Box<dyn MdecCommand> {
        Box::new((*self).clone())
    }

    fn name(&self) -> &str {
        "SetScaleTable"
    }

    fn set_status(&self, _status: &mut u32) {
    }
}