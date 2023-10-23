
#define cease .insn i 0x73, 0x0, x0, x0, 0x305

.section .text._start,"ax",@progbits
.global _start
_start:
        csrr            t0, mhartid
        li              t1, 1
        bne             t0, t1, _ap_wait
.option push
.option norelax
        lla             gp, __global_pointer$
.option pop
        lla             t0, __bss
        lla             t1, __ebss
2:      sb              zero, (t0)
        addi            t0, t0, 1
        bltu            t0, t1, 2b
        lla             sp, __boot_stackp
        call            _relocate_firmware
        lla             t0, _trap_vector
        csrw            mtvec, t0
        j               chainload_start

_ap_wait:
        cease
        j       .

.p2align 2
_trap_vector:
        csrw            mscratch, sp
        andi            sp, sp, ~0xf
        addi            sp, sp, -(8 * (32 + 5))
        sd               x0, (8 *  0)(sp)
        sd               x1, (8 *  1)(sp)
        sd               x2, (8 *  2)(sp)
        sd               x3, (8 *  3)(sp)
        sd               x4, (8 *  4)(sp)
        sd               x5, (8 *  5)(sp)
        sd               x6, (8 *  6)(sp)
        sd               x7, (8 *  7)(sp)
        sd               x8, (8 *  8)(sp)
        sd               x9, (8 *  9)(sp)
        sd              x10, (8 * 10)(sp)
        sd              x11, (8 * 11)(sp)
        sd              x12, (8 * 12)(sp)
        sd              x13, (8 * 13)(sp)
        sd              x14, (8 * 14)(sp)
        sd              x15, (8 * 15)(sp)
        sd              x16, (8 * 16)(sp)
        sd              x17, (8 * 17)(sp)
        sd              x18, (8 * 18)(sp)
        sd              x19, (8 * 19)(sp)
        sd              x20, (8 * 20)(sp)
        sd              x21, (8 * 21)(sp)
        sd              x22, (8 * 22)(sp)
        sd              x23, (8 * 23)(sp)
        sd              x24, (8 * 24)(sp)
        sd              x25, (8 * 25)(sp)
        sd              x26, (8 * 26)(sp)
        sd              x27, (8 * 27)(sp)
        sd              x28, (8 * 28)(sp)
        sd              x29, (8 * 29)(sp)
        sd              x30, (8 * 30)(sp)
        sd              x31, (8 * 31)(sp)

        csrr            t0, mstatus
        csrr            t1, mcause
        csrr            t2, mepc
        csrr            t3, mtval
        csrr            t4, mscratch
        sd              t0, (8 * 32)(sp)
        sd              t1, (8 * 33)(sp)
        sd              t2, (8 * 34)(sp)
        sd              t3, (8 * 35)(sp)
        sd              t4, (8 * 36)(sp)

        mv              a0, sp
        call            trap_handler
        cease
        j               .

#define DT_RELA             7
#define DT_RELASZ           8
#define DT_RELAENT          9
#define DT_RELRSZ           35
#define DT_RELR             36
#define DT_RELRENT          37
#define DYN_tag             0
#define DYN_val             8
#define DYN_size            16
#define R_RISCV_RELATIVE    3
#define RELA_offset         0
#define RELA_info           8
#define RELA_addend         16
#define RELA_size           24

.section .text._relocate_firmware,"ax",@progbits
.p2align 2
_relocate_firmware:
        lla     a0, __image_base        // get the base address we got loaded at
        lla     a1, _DYNAMIC            // get pointer to the dynamic table

        mv      s1, zero                // RELA table pointer
        mv      s2, zero                // RELA table size (bytes)
        mv      s3, zero                // RELA entry size (bytes)
        mv      s4, zero                // RELR table pointer
        mv      s5, zero                // RELR table size (bytes)
        mv      s6, zero                // RELR entry size (bytes)

1:      ld      t0, DYN_tag(a1)
        li      t1, DT_RELA
        bne     t0, t1, 2f
        ld      s1, DYN_val(a1)
        j       3f
2:      li      t1, DT_RELASZ
        bne     t0, t1, 2f
        ld      s2, DYN_val(a1)
        j       3f
2:      li      t1, DT_RELAENT
        bne     t0, t1, 2f
        ld      s3, DYN_val(a1)
        j       3f
2:      li      t1, DT_RELR
        bne     t0, t1, 2f
        ld      s1, DYN_val(a1)
        j       3f
2:      li      t1, DT_RELRSZ
        bne     t0, t1, 2f
        ld      s2, DYN_val(a1)
        j       3f
2:      li      t1, DT_RELRENT
        bne     t0, t1, 3f
        ld      s3, DYN_val(a1)
3:      addi    a1, a1, DYN_size
        bnez    t0, 1b  // DT_NULL

        // Do RELA type relocations.

        beqz    s1, 8f                  // return ok if no DT_RELA
        beqz    s2, 9f                  // return err if no DT_RELASZ
        beqz    s3, 9f                  // return err if no DT_RELAENT
        li      t0, RELA_size           // return err if entry size isn't what we expect
        bne     t0, s3, 9f              //

        add     s1, s1, a0              // relocate table pointer

        add     s2, s1, s2              // calculate pointer to (one past) the end of table
        j       1f
1:      add     s1, s1, s3              // move to next entry
2:      bgeu    s1, s2, 3f              // break if we've reached the end
        ld      t0, RELA_info(s1)       // relocation type
        beqz    t0, 1b  // R_RISCV_NONE
        li      t1, R_RISCV_RELATIVE
        bne     t0, t1, 1b
        ld      t0, RELA_offset(s1)
        ld      t1, RELA_addend(s1)
        add     t0, t0, a0
        add     t1, a0, t1
        sd      t1, (t0)
        j       1b
3:

        // TODO: Do RELR type relocations.
        bnez    s4, 9f

        // Exit OK
8:      mv      a0, zero                // return ok (0)
        ret
        // Exit ERR
9:      li      a0, 1                   // return error (1)
        ret
