/*
 * Khmer Terminal Engine (KTE): C API.
 *
 * Implemented by the `kte-capi` crate. Link against libkte_capi (static or
 * shared). On Linux, a static link also needs -lpthread -ldl -lm.
 *
 * Conventions:
 *  - Text is UTF-8, passed as pointer + byte length (no NUL terminator needed).
 *    Functions taking text return KTE_INVALID for invalid UTF-8.
 *  - Functions returning text copy at most `buf_len` bytes into `buf` (no NUL
 *    terminator) and return the full length. Pass buf = NULL, buf_len = 0 to
 *    query the length first.
 *  - Cell model: KTE_MODEL_WIDE (2 cells per typical consonant, default) or
 *    KTE_MODEL_COMPACT (1 cell per consonant). See spec/kte-width-spec.md.
 *
 * SPDX-License-Identifier: MIT
 */
#ifndef KTE_H
#define KTE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define KTE_INVALID ((size_t)-1)
#define KTE_MODEL_WIDE 0u
#define KTE_MODEL_COMPACT 1u

typedef struct KteTerminal KteTerminal;

/* Version of the KTE Width Spec implemented (static string, do not free). */
const char *kte_spec_version(void);

/* ---- Text measurement ---------------------------------------------------- */

/* Width in cells of a UTF-8 string. */
size_t kte_str_width(const uint8_t *text, size_t len, uint32_t model);

/* Half-units of the first cluster, or -1 if it is not a Khmer cluster. */
int32_t kte_half_units(const uint8_t *text, size_t len);

/* Byte offset of the next / previous cluster boundary (cursor right / left). */
size_t kte_next_boundary(const uint8_t *text, size_t len, size_t offset);
size_t kte_prev_boundary(const uint8_t *text, size_t len, size_t offset);

/* ---- Terminal model -------------------------------------------------------- */

KteTerminal *kte_terminal_new(uint32_t rows, uint32_t cols, uint32_t model);
void kte_terminal_free(KteTerminal *term);

/* Feed application output (PTY output side). */
void kte_terminal_feed(KteTerminal *term, const uint8_t *bytes, size_t len);
void kte_terminal_resize(KteTerminal *term, uint32_t rows, uint32_t cols);

/* Zero-based cursor position. Either pointer may be NULL. */
void kte_terminal_cursor(const KteTerminal *term, uint32_t *row, uint32_t *col);

/* Text of one cell; *width receives the cell width (0 = continuation of a
 * wider cluster). Returns KTE_INVALID when out of range. */
size_t kte_terminal_cell(const KteTerminal *term, uint32_t row, uint32_t col,
                         uint8_t *buf, size_t buf_len, uint8_t *width);

/* Text of a screen row, trailing blanks removed. */
size_t kte_terminal_row_text(const KteTerminal *term, uint32_t row,
                             uint8_t *buf, size_t buf_len);

/* Selected text between two cells (inclusive), snapped to whole clusters. */
size_t kte_terminal_selection_text(const KteTerminal *term,
                                   uint32_t from_row, uint32_t from_col,
                                   uint32_t to_row, uint32_t to_col,
                                   uint8_t *buf, size_t buf_len);

/* Move up to buf_len bytes of pending replies (to write to the PTY input
 * side) into buf. Returns the number of bytes written. */
size_t kte_terminal_take_responses(KteTerminal *term, uint8_t *buf, size_t buf_len);

/* ---- Memory helpers (for WebAssembly hosts) -------------------------------- */

uint8_t *kte_alloc(size_t len);
void kte_free(uint8_t *ptr, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* KTE_H */
