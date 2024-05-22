use crate::{impl_r1cs_input_lc_conversions, input_range, poly::field::JoltField};

use super::{
    builder::{R1CSBuilder, R1CSConstraintBuilder},
    ops::{ConstraintInput, Variable, LC},
};

// TODO(sragss): JoltInputs -> JoltIn
#[allow(non_camel_case_types)]
#[derive(strum_macros::EnumIter, strum_macros::EnumCount, Clone, Copy, Debug, PartialEq)]
#[repr(usize)]
pub enum JoltInputs {
    PcIn,

    Bytecode_A, // Virtual address
    // Bytecode_V
    Bytecode_ELFAddress,
    Bytecode_Opcode,
    Bytecode_RS1,
    Bytecode_RS2,
    Bytecode_RD,
    Bytecode_Imm,

    RAM_A,
    // Ram_V
    RAM_Read_RS1,
    RAM_Read_RS2,
    RAM_Read_RD, // TODO(sragss): Appears to be unused?
    RAM_Read_Byte0,
    RAM_Read_Byte1,
    RAM_Read_Byte2,
    RAM_Read_Byte3,
    RAM_Write_RD,
    RAM_Write_Byte0,
    RAM_Write_Byte1,
    RAM_Write_Byte2,
    RAM_Write_Byte3,

    ChunksX_0,
    ChunksX_1,
    ChunksX_2,
    ChunksX_3,

    ChunksY_0,
    ChunksY_1,
    ChunksY_2,
    ChunksY_3,

    ChunksQ_0,
    ChunksQ_1,
    ChunksQ_2,
    ChunksQ_3,

    LookupOutput,

    // TODO(sragss): Dedupe OpFlags / CircuitFlags
    // TODO(sragss): Explicit unit test for comparing OpFlags and InstructionFlags
    // TODO(sragss): Better names for first 2.
    // Should match rv_trace.to_circuit_flags()
    OpFlags0,
    OpFlags_IsImm,
    OpFlags_IsLoad,
    OpFlags_IsStore,
    OpFlags_IsJmp,
    OpFlags_IsBranch,
    OpFlags_LookupOutToRd,
    OpFlags_SignImm,
    OpFlags_IsConcat,
    OpFlags_IsVirtualSequence, // TODO(moodlezoup / arasuarun): Better name?
    OpFlags_IsVirtual,

    // Instruction Flags
    // Should match JoltInstructionSet
    IF_Add,
    IF_Sub,
    IF_And,
    IF_Or,
    IF_Xor,
    IF_Lb,
    IF_Lh,
    IF_Sb,
    IF_Sh,
    IF_Sw,
    IF_Beq,
    IF_Bge,
    IF_Bgeu,
    IF_Bne,
    IF_Slt,
    IF_Sltu,
    IF_Sll,
    IF_Sra,
    IF_Srl,
    IF_Movsign,
    IF_Mul,
    IF_MulU,
    IF_MulHu,
}
impl_r1cs_input_lc_conversions!(JoltInputs);
impl ConstraintInput for JoltInputs {}

const PC_START_ADDRESS: i64 = 0x80000000;
const PC_NOOP_SHIFT: i64 = 4;
const MEMORY_START: i64 = 128; // TODO(sragss): Non constant.
const LOG_M: usize = 16;
const OPERAND_SIZE: usize = LOG_M / 2;

pub struct JoltConstraints();
impl<F: JoltField> R1CSConstraintBuilder<F> for JoltConstraints {
    type Inputs = JoltInputs;
    fn build_constraints(&self, cs: &mut R1CSBuilder<F, Self::Inputs>) {
        let flags = input_range!(JoltInputs::OpFlags0, JoltInputs::IF_MulHu);
        for flag in flags {
            cs.constrain_binary(flag);
        }

        cs.constrain_eq(JoltInputs::PcIn, JoltInputs::Bytecode_A);

        // TODO(sragss): I don't understand how this constraint is possibly true unless Bytecode_Opcode is mis-labeled.
        cs.constrain_pack_be(flags.to_vec(), JoltInputs::Bytecode_Opcode, 1);


        let real_pc = LC::sum2(4i64 * JoltInputs::PcIn, PC_START_ADDRESS - PC_NOOP_SHIFT);
        let x = cs.allocate_if_else(JoltInputs::OpFlags0, real_pc, JoltInputs::RAM_Read_RS1);
        let y = cs.allocate_if_else(
            JoltInputs::OpFlags_IsImm,
            JoltInputs::Bytecode_Imm,
            JoltInputs::RAM_Read_RS2
        );

        let signed_output = LC::sub2(JoltInputs::Bytecode_Imm, 0xffffffffi64 - 1i64); // TODO(sragss): Comment about twos-complement.
        let imm_signed = cs.allocate_if_else(
            JoltInputs::OpFlags_SignImm,
            JoltInputs::Bytecode_Imm,
            signed_output,
        );

        let flag_0_or_1_condition = LC::sum2(JoltInputs::OpFlags0, JoltInputs::OpFlags_IsImm);
        // TODO(sragss): MEMORY_START is not a constant. Store on JoltConstraints.
        // cs.constrain_eq_conditional(
        //     flag_0_or_1_condition,
        //     LC::sum2(JoltInputs::RAM_Read_RS1, imm_signed),
        //     LC::sum2(JoltInputs::RAM_A, MEMORY_START),
        // );

        cs.constrain_eq_conditional(
            JoltInputs::OpFlags_IsLoad,
            JoltInputs::RAM_Read_Byte0,
            JoltInputs::RAM_Write_Byte0,
        );
        cs.constrain_eq_conditional(
            JoltInputs::OpFlags_IsLoad,
            JoltInputs::RAM_Read_Byte1,
            JoltInputs::RAM_Write_Byte1,
        );
        cs.constrain_eq_conditional(
            JoltInputs::OpFlags_IsLoad,
            JoltInputs::RAM_Read_Byte2,
            JoltInputs::RAM_Write_Byte2,
        );
        cs.constrain_eq_conditional(
            JoltInputs::OpFlags_IsLoad,
            JoltInputs::RAM_Read_Byte3,
            JoltInputs::RAM_Write_Byte3,
        );

        let ram_writes = input_range!(JoltInputs::RAM_Write_Byte0, JoltInputs::RAM_Write_Byte3);
        let packed_load_store = cs.allocate_pack_le(ram_writes.to_vec(), 8);
        cs.constrain_eq_conditional(
            JoltInputs::OpFlags_IsStore,
            packed_load_store,
            JoltInputs::LookupOutput,
        );

        let packed_query = cs.allocate_pack_be(
            input_range!(JoltInputs::ChunksQ_0, JoltInputs::ChunksQ_3).to_vec(),
            LOG_M,
        );
        // TODO(sragss): I think packed_query is borked.
        // Packed query: 2147487744
        // x + y: 4088 + 4096 
        // Would be good to know ChunksQ[0..3] at step 0 to know if the packing is working
        // Chunks Query at step 0: BigInt([0, 0, 0, 0])
        // Chunks Query at step 0: BigInt([0, 0, 0, 0])
        // Chunks Query at step 0: BigInt([32768, 0, 0, 0])
        // Chunks Query at step 0: BigInt([4096, 0, 0, 0])
        // 32768 * 2^16 + 4096 = 2147487744
        // Actual instruction: ADDInstruction(2147483648, 4096) (AUIPC)
        // Raw_trace[0] RVTraceRow { instruction: ELFInstruction { address: 2147483648, opcode: AUIPC, rs1: None, rs2: None, rd: Some(2), imm: Some(4096), virtual_sequence_index: None }, register_state: RegisterState { rs1_val: None, rs2_val: None, rd_post_val: Some(2147487744) }, memory_state: None }

        cs.constrain_eq_conditional(JoltInputs::IF_Add, packed_query, x + y);
        cs.constrain_eq_conditional(JoltInputs::IF_Sub, packed_query, x - y + (0xffffffffi64 + 1).into()); // TODO(sragss): Comment on twos complement
        cs.constrain_eq_conditional(JoltInputs::OpFlags_IsLoad, packed_query, packed_load_store);
        cs.constrain_eq_conditional(
            JoltInputs::OpFlags_IsStore,
            packed_query,
            JoltInputs::RAM_Read_RS2,
        );

        // TODO(sragss): Uses 2 excess constraints for condition gating. Could make constrain_pack_be_conditional... Or make everything conditional...
        let chunked_x = cs.allocate_pack_be(
            input_range!(JoltInputs::ChunksX_0, JoltInputs::ChunksX_3).to_vec(),
            OPERAND_SIZE,
        );
        let chunked_y = cs.allocate_pack_be(
            input_range!(JoltInputs::ChunksY_0, JoltInputs::ChunksY_3).to_vec(),
            OPERAND_SIZE,
        );
        cs.constrain_eq_conditional(JoltInputs::OpFlags_IsConcat, chunked_x, x);
        cs.constrain_eq_conditional(JoltInputs::OpFlags_IsConcat, chunked_y, y);

        // TODO(sragss): Missing some concat shit here.

        // if (rd != 0 && if_update_rd_with_lookup_output == 1) constrain(rd_val == LookupOutput)
        // if (rd != 0 && is_jump_instr == 1) constrain(rd_val == 4 * PC)
        let rd_nonzero_and_lookup_to_rd =
            cs.allocate_prod(JoltInputs::Bytecode_RD, JoltInputs::OpFlags_LookupOutToRd);
        cs.constrain_eq_conditional(
            rd_nonzero_and_lookup_to_rd,
            JoltInputs::RAM_Write_RD,
            JoltInputs::LookupOutput,
        );
        let rd_nonzero_and_jmp =
            cs.allocate_prod(JoltInputs::Bytecode_RD, JoltInputs::OpFlags_IsJmp);
        let lhs = LC::sum2(JoltInputs::PcIn, PC_START_ADDRESS - PC_NOOP_SHIFT);
        let rhs = JoltInputs::RAM_Write_RD;
        cs.constrain_eq_conditional(rd_nonzero_and_jmp, lhs, rhs);

        // TODO(sragss): PC incrementing constraints. Next PC: Check if it's a branch and the lookup output is 1. Check if it's a jump.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Fr;
    use strum::EnumCount;

    #[test]
    fn single_instruction_jolt() {
        let mut builder = R1CSBuilder::<Fr, JoltInputs>::new();

        let jolt_constraints = JoltConstraints();
        jolt_constraints.build_constraints(&mut builder);

        let num_steps = 1;
        let mut inputs = vec![vec![Fr::zero(); num_steps]; JoltInputs::COUNT];

        // ADD instruction
        inputs[JoltInputs::PcIn as usize][0] = Fr::from(10);
        inputs[JoltInputs::Bytecode_A as usize][0] = Fr::from(10);
        inputs[JoltInputs::Bytecode_Opcode as usize][0] = Fr::from(0);
        inputs[JoltInputs::Bytecode_RS1 as usize][0] = Fr::from(2);
        inputs[JoltInputs::Bytecode_RS2 as usize][0] = Fr::from(3);
        inputs[JoltInputs::Bytecode_RD as usize][0] = Fr::from(4);

        inputs[JoltInputs::RAM_Read_RD as usize][0] = Fr::from(0);
        inputs[JoltInputs::RAM_Read_RS1 as usize][0] = Fr::from(100);
        inputs[JoltInputs::RAM_Read_RS2 as usize][0] = Fr::from(200);
        inputs[JoltInputs::RAM_Write_RD as usize][0] = Fr::from(300);
        // remainder RAM == 0

        // rv_trace::to_circuit_flags
        // all zero for ADD
        inputs[JoltInputs::OpFlags0 as usize][0] = Fr::zero(); // first_operand = rs1
        inputs[JoltInputs::OpFlags_IsImm as usize][0] = Fr::zero(); // second_operand = rs2 => immediate

        let aux = builder.compute_aux(&inputs);
        let (az, bz, cz) = builder.compute_spartan(&inputs, &aux, &vec![]);

        builder.assert_valid(&az, &bz, &cz, &vec![], num_steps);
    }
}
