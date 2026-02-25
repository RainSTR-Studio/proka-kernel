; Syscall Entry and Exit Assembly Code
;
; This file contains the low-level assembly routines for handling
; system calls via the syscall/sysret instructions.
;
; Calling Convention (System V AMD64 ABI):
;   rax = syscall number (input) / return value (output)
;   rdi = arg1
;   rsi = arg2
;   rdx = arg3
;   r10 = arg4 (r10 instead of rcx to avoid conflict)
;   r8  = arg5
;   r9  = arg6
;   rcx = saved rip (by hardware)
;   r11 = saved rflags (by hardware)
;
; Note: syscall instruction clobbers rcx and r11

BITS 64

; External symbols
extern syscall_handler

; ============================================
; Syscall Entry Point
; ============================================
; This is where execution lands when a user program
; executes the 'syscall' instruction.
;
; At entry:
;   - CS = kernel code segment (from IA32_STAR)
;   - SS = kernel data segment (CS + 8)
;   - RIP = IA32_LSTAR (this function)
;   - RFLAGS = RFLAGS & ~IA32_FMASK
;   - RCX = user RIP (return address)
;   - R11 = user RFLAGS
;   - Interrupts are disabled
;
; We need to:
;   1. Switch to kernel stack
;   2. Save all registers
;   3. Call Rust handler
;   4. Restore registers
;   5. Return to user space
; ============================================

global syscall_entry
syscall_entry:
    ; Step 1: Switch to kernel stack
    ; For now, use a fixed kernel stack area
    ; In a real implementation, this should get the current thread's kernel stack
    ; from per-CPU data (via swapgs or similar)

    ; Save user RSP temporarily in r15 (callee-saved)
    mov r15, rsp

    ; Load kernel stack
    mov rsp, syscall_kernel_stack_top

    ; Step 2: Push all registers onto the kernel stack
    ; Order matches SyscallArgs struct in Rust
    push r15          ; user_rsp (saved in r15)
    push r11          ; user_rflags (saved by hardware)
    push rcx          ; user_rip (saved by hardware)
    push r9           ; arg6
    push r8           ; arg5
    push r10          ; arg4
    push rdx          ; arg3
    push rsi          ; arg2
    push rdi          ; arg1
    push rax          ; syscall number

    ; Save callee-saved registers
    push r15
    push r14
    push r13
    push r12
    push rbx
    push rbp

    ; Step 3: Call the Rust syscall handler
    ; First argument (rdi) = pointer to saved registers
    mov rdi, rsp
    call syscall_handler

    ; Step 4: Restore callee-saved registers
    pop rbp
    pop rbx
    pop r12
    pop r13
    pop r14
    pop r15

    ; Step 5: Restore syscall arguments (but we need user_rip and user_rflags)
    ; Skip rax (return value)
    add rsp, 8        ; skip rax (syscall number)
    pop rdi           ; arg1
    pop rsi           ; arg2
    pop rdx           ; arg3
    pop r10           ; arg4
    pop r8            ; arg5
    pop r9            ; arg6
    pop rcx           ; user_rip (for sysret)
    pop r11           ; user_rflags (for sysret)
    pop rsp           ; user_rsp

    ; Step 6: Return to user space
    ; sysretq loads:
    ;   RIP = RCX
    ;   CS = IA32_STAR[63:48] + 16 (64-bit code segment)
    ;   RFLAGS = R11
    ;   SS = IA32_STAR[63:48] + 8 (data segment)
    o64 sysret

; ============================================
; Kernel Stack for Syscall Handler
; ============================================
section .bss
align 4096

syscall_kernel_stack:
    resb 8192         ; 8KB kernel stack for syscall handling
syscall_kernel_stack_top:
