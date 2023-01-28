use cranelift::{prelude::{FunctionBuilderContext, FunctionBuilder}, codegen};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataContext, Linkage, Module};

use crate::bus::MainBus;

use super::{R3000, instruction::Instruction};

struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    data_ctx: DataContext,
    module: JITModule
}

impl Jit {
    fn new() -> Self {
        let builder = JITBuilder::new(cranelift_module::default_libcall_names());
        let module = JITModule::new(builder.unwrap());
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_ctx: DataContext::new(),
            module
        }
    }

    fn execute_from_addr(&mut self, cpu: &mut R3000, bus: &mut MainBus, addr: u32) {

    }

    fn compile_block(&mut self, bus: &mut MainBus, addr: u32) {

    }
}

struct BlockTranslator<'a> {
    builder: FunctionBuilder<'a>,
    module: &'a mut JITModule
}

impl<'a> BlockTranslator<'a> {
    fn translate_inst(&mut self, opcode: Instruction) {
        match opcode {
            Instruction::SLL { rt, rd, sa } => todo!(),
            Instruction::SRL { rt, rd, sa } => todo!(),
            Instruction::SRA { rt, rd, sa } => todo!(),
            Instruction::SLLV { rd, rt, rs } => todo!(),
            Instruction::SRLV { rd, rt, rs } => todo!(),
            Instruction::SRAV { rd, rt, rs } => todo!(),
            Instruction::JR { rs } => todo!(),
            Instruction::JALR { rd, rs } => todo!(),
            Instruction::SYSCALL { code } => todo!(),
            Instruction::BREAK { code } => todo!(),
            Instruction::MFHI { rd } => todo!(),
            Instruction::MTHI { rs } => todo!(),
            Instruction::MFLO { rd } => todo!(),
            Instruction::MTLO { rs } => todo!(),
            Instruction::DIV { rs, rt } => todo!(),
            Instruction::DIVU { rs, rt } => todo!(),
            Instruction::ADD { rd, rs, rt } => todo!(),
            Instruction::SUB { rd, rs, rt } => todo!(),
            Instruction::SLTU { rd, rs, rt } => todo!(),
            Instruction::SUBU { rd, rs, rt } => todo!(),
            Instruction::AND { rd, rs, rt } => todo!(),
            Instruction::OR { rd, rs, rt } => todo!(),
            Instruction::XOR { rd, rs, rt } => todo!(),
            Instruction::NOR { rd, rs, rt } => todo!(),
            Instruction::ADDU { rd, rs, rt } => todo!(),
            Instruction::MULT { rs, rt } => todo!(),
            Instruction::MULTU { rs, rt } => todo!(),
            Instruction::SLT { rd, rs, rt } => todo!(),
            Instruction::BLTZ { rs, offset, opcode } => todo!(),
            Instruction::BGEZ { rs, offset, opcode } => todo!(),
            Instruction::BLTZAL { rs, offset, opcode } => todo!(),
            Instruction::BGEZAL { rs, offset, opcode } => todo!(),
            Instruction::MALBRCH { rs, offset, opcode } => todo!(),
            Instruction::J { target } => todo!(),
            Instruction::JAL { target } => todo!(),
            Instruction::BEQ { rs, rt, offset } => todo!(),
            Instruction::BNE { rs, rt, offset } => todo!(),
            Instruction::BLEZ { rs, offset } => todo!(),
            Instruction::BGTZ { rs, offset } => todo!(),
            Instruction::ADDI { rt, rs, immediate } => todo!(),
            Instruction::ADDIU { rt, rs, immediate } => todo!(),
            Instruction::SLTI { rt, rs, immediate } => todo!(),
            Instruction::SLTIU { rt, rs, immediate } => todo!(),
            Instruction::ANDI { rt, rs, immediate } => todo!(),
            Instruction::ORI { rt, rs, immediate } => todo!(),
            Instruction::XORI { rt, rs, immediate } => todo!(),
            Instruction::LUI { rt, immediate } => todo!(),
            Instruction::MTC0 { rt, rd } => todo!(),
            Instruction::MFC0 { rt, rd } => todo!(),
            Instruction::RFE => todo!(),
            Instruction::MFC2 { rt, rd } => todo!(),
            Instruction::CTC2 { rt, rd } => todo!(),
            Instruction::MTC2 { rt, rd } => todo!(),
            Instruction::CFC2 { rt, rd } => todo!(),
            Instruction::IMM25 { command } => todo!(),
            Instruction::LB { rt, offset, base } => todo!(),
            Instruction::LH { rt, offset, base } => todo!(),
            Instruction::LW { rt, offset, base } => todo!(),
            Instruction::LBU { rt, offset, base } => todo!(),
            Instruction::LHU { rt, offset, base } => todo!(),
            Instruction::SB { rt, offset, base } => todo!(),
            Instruction::SH { rt, offset, base } => todo!(),
            Instruction::LWL { rt, offset, base } => todo!(),
            Instruction::LWR { rt, offset, base } => todo!(),
            Instruction::SWL { rt, offset, base } => todo!(),
            Instruction::SWR { rt, offset, base } => todo!(),
            Instruction::SW { rt, offset, base } => todo!(),
            Instruction::LWC2 { rt, offset, base } => todo!(),
            Instruction::SWC2 { rt, offset, base } => todo!(),
        }
    }
}