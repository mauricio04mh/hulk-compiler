#ifndef HULK_RUNTIME_H
#define HULK_RUNTIME_H

#include <stddef.h>
#include <stdint.h>

/* ── Strings ─────────────────────────────────────────────────────────── */

typedef struct HulkString {
    long long len;
    const char* data;
} HulkString;

void hulk_print_number(double value);
void hulk_print_bool(unsigned char value);
void hulk_print_string(HulkString* value);
void hulk_print_object(void);

double hulk_sqrt(double value);
double hulk_sin(double value);
double hulk_cos(double value);
double hulk_exp(double value);
double hulk_log(double base, double value);
double hulk_pow(double value, double exponent);
double hulk_rand(void);

HulkString* hulk_string_concat(HulkString* a, HulkString* b);
int         hulk_string_eq(HulkString* a, HulkString* b);
int8_t      hulk_string_equals(HulkString* a, HulkString* b);
HulkString* hulk_string_from_number(double value);
HulkString* hulk_string_from_bool(int8_t value);
void        hulk_runtime_error(HulkString* msg);

/* ── Object model (vtable-based) ─────────────────────────────────────── */

/* Matches %HulkVTable = type { i64, ptr, i64, ptr } in generated LLVM IR.
   ALL runtime objects (including ranges and vectors) must start with a
   HulkVTable* as their very first field so that hulk_object_method can
   retrieve the right vtable slot without knowing the concrete type. */
typedef struct HulkVTable {
    int64_t            type_id;
    struct HulkVTable* parent;
    int64_t            method_count;
    void**             methods;
} HulkVTable;

/* Object lifecycle */
void*   hulk_alloc_object(int64_t type_id, int64_t attr_count, HulkVTable* vtable);
void*   hulk_object_method(void* obj, int64_t slot);
int8_t  hulk_object_is(void* obj, int64_t target_type_id);
void*   hulk_object_as(void* obj, int64_t target_type_id);

/* Attribute setters */
void hulk_object_set_number(void* obj, int64_t attr_id, double value);
void hulk_object_set_bool(void* obj, int64_t attr_id, int8_t value);
void hulk_object_set_string(void* obj, int64_t attr_id, HulkString* value);
void hulk_object_set_object(void* obj, int64_t attr_id, void* value);

/* Attribute getters */
double      hulk_object_get_number(void* obj, int64_t attr_id);
int8_t      hulk_object_get_bool(void* obj, int64_t attr_id);
HulkString* hulk_object_get_string(void* obj, int64_t attr_id);
void*       hulk_object_get_object(void* obj, int64_t attr_id);

/* Legacy (used internally by vectors/ranges) */
typedef struct HulkTypeDesc {
    int      type_id;
    void**   vtable;
    int      ancestor_count;
    int*     ancestors;
} HulkTypeDesc;

int hulk_type_test(HulkTypeDesc* desc, int target_id);
void hulk_type_cast_check(void* obj, int target_id);

/* ── Vectors ─────────────────────────────────────────────────────────── */

/* HulkVector first field MUST be HulkVTable* so hulk_object_method works */
typedef struct HulkVector {
    HulkVTable* vtable;    /* slot 0 = next, slot 1 = current */
    int64_t*    data;
    int64_t     len;
    int64_t     cap;
    int64_t     cursor;
} HulkVector;

HulkVector* hulk_vector_new(void);
void        hulk_vector_push(HulkVector* v, int64_t elem);
int64_t     hulk_vector_get(HulkVector* v, int64_t idx);
void        hulk_vector_set(HulkVector* v, int64_t idx, int64_t elem);
int64_t     hulk_vector_len(HulkVector* v);
int         hulk_vector_next(HulkVector* v);
double      hulk_vector_current(HulkVector* v);

/* ── Ranges ──────────────────────────────────────────────────────────── */

/* HulkRange first field MUST be HulkVTable* */
typedef struct HulkRange {
    HulkVTable* vtable;    /* slot 0 = next, slot 1 = current */
    double      current_val;
    double      end_val;
} HulkRange;

HulkRange* hulk_range(double start, double end);
int        hulk_range_next(HulkRange* r);
double     hulk_range_current(HulkRange* r);

/* ── Closures ────────────────────────────────────────────────────────── */

typedef struct HulkClosure {
    void*   fn_ptr;
    int64_t num_captures;
    void*   captures[];
} HulkClosure;

HulkClosure* hulk_closure_alloc(void* fn_ptr, int64_t num_captures);
void         hulk_closure_set_capture(HulkClosure* c, int64_t idx, void* val);
void*        hulk_closure_get_capture(HulkClosure* c, int64_t idx);

#endif
