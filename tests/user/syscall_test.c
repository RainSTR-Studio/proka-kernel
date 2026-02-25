/*
 * User-mode System Call Test Program
 *
 * This program tests the system call mechanism from Ring 3.
 * It should be compiled as a freestanding binary and loaded
 * by the kernel for testing.
 */

#include <stdint.h>
#include <stddef.h>

/* System call numbers */
#define SYS_EXIT     0
#define SYS_PUTC     1
#define SYS_IPC_SEND 2
#define SYS_IPC_RECV 3
#define SYS_GET_PID  4

/*
 * System call wrapper using inline assembly
 * Follows System V AMD64 ABI:
 *   rax = syscall number
 *   rdi = arg1
 *   rsi = arg2
 *   rdx = arg3
 *   r10 = arg4
 *   r8  = arg5
 *   r9  = arg6
 */
static inline uint64_t syscall0(uint64_t n) {
    uint64_t ret;
    __asm__ volatile (
        "syscall"
        : "=a"(ret)
        : "a"(n)
        : "rcx", "r11", "memory"
    );
    return ret;
}

static inline uint64_t syscall1(uint64_t n, uint64_t arg1) {
    uint64_t ret;
    __asm__ volatile (
        "syscall"
        : "=a"(ret)
        : "a"(n), "D"(arg1)
        : "rcx", "r11", "memory"
    );
    return ret;
}

static inline uint64_t syscall2(uint64_t n, uint64_t arg1, uint64_t arg2) {
    uint64_t ret;
    __asm__ volatile (
        "syscall"
        : "=a"(ret)
        : "a"(n), "D"(arg1), "S"(arg2)
        : "rcx", "r11", "memory"
    );
    return ret;
}

static inline uint64_t syscall3(uint64_t n, uint64_t arg1, uint64_t arg2, uint64_t arg3) {
    uint64_t ret;
    __asm__ volatile (
        "syscall"
        : "=a"(ret)
        : "a"(n), "D"(arg1), "S"(arg2), "d"(arg3)
        : "rcx", "r11", "memory"
    );
    return ret;
}

/* System call wrappers */
static inline void sys_exit(int code) {
    syscall1(SYS_EXIT, (uint64_t)code);
    __builtin_unreachable();
}

static inline uint64_t sys_putc(char c) {
    return syscall1(SYS_PUTC, (uint64_t)c);
}

static inline uint64_t sys_get_pid(void) {
    return syscall0(SYS_GET_PID);
}

/*
 * Simple string output using sys_putc
 */
static void print_string(const char* str) {
    while (*str) {
        sys_putc(*str);
        str++;
    }
}

/*
 * Convert integer to string
 */
static void print_int(uint64_t n) {
    char buf[32];
    int i = 0;

    if (n == 0) {
        sys_putc('0');
        return;
    }

    while (n > 0) {
        buf[i++] = '0' + (n % 10);
        n /= 10;
    }

    while (i > 0) {
        sys_putc(buf[--i]);
    }
}

/*
 * Entry point for the test program
 */
void _start(void) {
    /* Print a greeting message */
    print_string("Hello from user mode!\n");

    /* Test sys_get_pid */
    print_string("My PID: ");
    uint64_t pid = sys_get_pid();
    print_int(pid);
    print_string("\n");

    /* Test sys_putc */
    print_string("Testing sys_putc: ");
    sys_putc('A');
    sys_putc('B');
    sys_putc('C');
    print_string("\n");

    /* Print success message */
    print_string("All tests passed!\n");

    /* Exit */
    sys_exit(0);
}
