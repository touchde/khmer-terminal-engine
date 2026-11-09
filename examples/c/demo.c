/*
 * Minimal C program using the KTE C API. Built and run in CI:
 *
 *   cargo build -p kte-capi --release
 *   cc examples/c/demo.c -Iinclude target/release/libkte_capi.a -o kte-demo   # + -lpthread -ldl -lm on Linux
 *   ./kte-demo
 */
#include <stdio.h>
#include <string.h>

#include "kte.h"

#define CHECK(cond)                                                    \
    do {                                                               \
        if (!(cond)) {                                                 \
            fprintf(stderr, "check failed: %s (line %d)\n", #cond, __LINE__); \
            return 1;                                                  \
        }                                                              \
    } while (0)

int main(void) {
    const char *text = "សួស្តី ពិភពលោក!";
    size_t len = strlen(text);

    printf("KTE spec %s\n", kte_spec_version());
    size_t wide = kte_str_width((const uint8_t *)text, len, KTE_MODEL_WIDE);
    size_t compact = kte_str_width((const uint8_t *)text, len, KTE_MODEL_COMPACT);
    printf("\"%s\": %zu cells (wide), %zu cells (compact)\n", text, wide, compact);
    CHECK(wide == 21 && compact == 13);

    KteTerminal *term = kte_terminal_new(4, 40, KTE_MODEL_WIDE);
    kte_terminal_feed(term, (const uint8_t *)text, len);

    uint32_t row, col;
    kte_terminal_cursor(term, &row, &col);
    CHECK(row == 0 && col == 21);

    uint8_t buf[256];
    size_t n = kte_terminal_row_text(term, 0, buf, sizeof buf);
    CHECK(n == len && memcmp(buf, text, len) == 0);

    uint8_t width = 99;
    n = kte_terminal_cell(term, 0, 3, buf, sizeof buf, &width);
    printf("cell (0,3): \"%.*s\", width %u\n", (int)n, buf, width);
    CHECK(width == 3);

    kte_terminal_feed(term, (const uint8_t *)"\x1b[6n", 4);
    n = kte_terminal_take_responses(term, buf, sizeof buf);
    CHECK(n == 7 && memcmp(buf, "\x1b[1;22R", 7) == 0);

    kte_terminal_free(term);
    printf("ok\n");
    return 0;
}
