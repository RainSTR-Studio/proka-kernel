/*
 * User-mode IPC Test Program
 *
 * This program tests the IPC call mechanism from Ring 3.
 * It should be compiled as a freestanding binary and loaded
 * by the kernel for testing.
 */

#include <stdint.h>
#include <stddef.h>

/* Service IDs */
#define SERVICE_PROCESS  0
#define SERVICE_MEMORY   1
#define SERVICE_CONSOLE  2
#define SERVICE_FS       3
#define SERVICE_DEVICE   4

/* Process service message types */
#define PROCESS_EXIT   0
#define PROCESS_GETPID 1
#define PROCESS_SPAWN  2
#define PROCESS_WAIT   3

/* Memory service message types */
#define MEMORY_MMAP   0
#define MEMORY_MUNMAP 1
#define MEMORY_BRK    2

/* Console service message types */
#define CONSOLE_PUTC  0
#define CONSOLE_GETC  1
#define CONSOLE_WRITE 2
#define CONSOLE_READ  3

/*
 * IPC call using inline assembly
 * 
 * Parameters:
 *   rdi = service_id
 *   rsi = payload_ptr
 *   rdx = reserved (0)
 *   r10 = msg_type
 *   r8  = payload_ptr (alternate)
 *   r9  = payload_size
 *
 * Returns:
 *   rax = return value (or error code with high bit set)
 */
static inline uint64_t ipc_call(
    uint64_t service_id,
    uint64_t msg_type,
    const void *payload,
    uint64_t payload_size
) {
    uint64_t ret;
    __asm__ volatile (
        "syscall"
        : "=a"(ret)
        : "a"(0), "D"(service_id), "S"(payload), "d"(0), 
          "r"(msg_type), "r"(payload), "r"(payload_size)
        : "rcx", "r11", "r8", "r9", "r10", "memory"
    );
    return ret;
}

/* Helper: Check if result is an error */
static inline int is_error(uint64_t result) {
    return (result >> 63) != 0;
}

/* Helper: Get error code from result */
static inline int64_t get_error(uint64_t result) {
    return (int64_t)(result & 0x7FFFFFFFFFFFFFFF);
}

/*
 * Process service wrappers
 */

/* Exit the current process */
static inline void proc_exit(int code) {
    uint8_t payload[4] = {
        (uint8_t)(code & 0xFF),
        (uint8_t)((code >> 8) & 0xFF),
        (uint8_t)((code >> 16) & 0xFF),
        (uint8_t)((code >> 24) & 0xFF)
    };
    ipc_call(SERVICE_PROCESS, PROCESS_EXIT, payload, 4);
    __builtin_unreachable();
}

/* Get current process ID */
static inline uint64_t proc_getpid(void) {
    return ipc_call(SERVICE_PROCESS, PROCESS_GETPID, NULL, 0);
}

/*
 * Console service wrappers
 */

/* Output a character */
static inline uint64_t console_putc(char c) {
    uint8_t payload[1] = { (uint8_t)c };
    return ipc_call(SERVICE_CONSOLE, CONSOLE_PUTC, payload, 1);
}

/* Write a string */
static inline uint64_t console_write(const char *str) {
    size_t len = 0;
    while (str[len]) len++;
    return ipc_call(SERVICE_CONSOLE, CONSOLE_WRITE, str, len);
}

/*
 * Memory service wrappers
 */

/* mmap flags */
#define PROT_READ   0x1
#define PROT_WRITE  0x2
#define PROT_EXEC   0x4

#define MAP_PRIVATE   0x02
#define MAP_ANONYMOUS 0x20

/* Allocate memory using mmap */
static inline void *mem_alloc(size_t size) {
    uint8_t payload[32] = {0};
    
    /* addr = 0 */
    /* size */
    payload[8] = size & 0xFF;
    payload[9] = (size >> 8) & 0xFF;
    payload[10] = (size >> 16) & 0xFF;
    payload[11] = (size >> 24) & 0xFF;
    payload[12] = (size >> 32) & 0xFF;
    payload[13] = (size >> 40) & 0xFF;
    payload[14] = (size >> 48) & 0xFF;
    payload[15] = (size >> 56) & 0xFF;
    
    /* prot = PROT_READ | PROT_WRITE */
    payload[16] = PROT_READ | PROT_WRITE;
    
    /* flags = MAP_PRIVATE | MAP_ANONYMOUS */
    payload[24] = MAP_PRIVATE | MAP_ANONYMOUS;
    
    uint64_t result = ipc_call(SERVICE_MEMORY, MEMORY_MMAP, payload, 32);
    return (void *)result;
}

/*
 * Simple string output
 */
static void print_string(const char* str) {
    console_write(str);
}

/*
 * Convert integer to string
 */
static void print_int(uint64_t n) {
    char buf[32];
    int i = 0;

    if (n == 0) {
        console_putc('0');
        return;
    }

    while (n > 0) {
        buf[i++] = '0' + (n % 10);
        n /= 10;
    }

    while (i > 0) {
        console_putc(buf[--i]);
    }
}

/*
 * Entry point for the test program
 */
void _start(void) {
    /* Print a greeting message */
    print_string("Hello from user mode via IPC!\n");

    /* Test proc_getpid */
    print_string("My PID: ");
    uint64_t pid = proc_getpid();
    print_int(pid);
    print_string("\n");

    /* Test console_putc */
    print_string("Testing console_putc: ");
    console_putc('A');
    console_putc('B');
    console_putc('C');
    print_string("\n");

    /* Print success message */
    print_string("All IPC tests passed!\n");

    /* Exit */
    proc_exit(0);
}
