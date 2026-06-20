#include "hulk_runtime.h"

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void hulk_print_number(double value) {
    printf("%.15g\n", value);
}

void hulk_print_bool(unsigned char value) {
    printf("%s\n", value ? "true" : "false");
}

void hulk_print_string(HulkString* value) {
    if (value == NULL || value->data == NULL) {
        puts("<null>");
        return;
    }
    fwrite(value->data, 1, (size_t)value->len, stdout);
    putchar('\n');
}

void hulk_print_object(void) {
    puts("<object>");
}

double hulk_sqrt(double value)                  { return sqrt(value); }
double hulk_sin(double value)                   { return sin(value); }
double hulk_cos(double value)                   { return cos(value); }
double hulk_exp(double value)                   { return exp(value); }
double hulk_log(double base, double value)      { return log(value) / log(base); }
double hulk_pow(double value, double exponent)  { return pow(value, exponent); }
double hulk_rand(void)                          { return (double)rand() / (double)RAND_MAX; }

HulkString* hulk_string_concat(HulkString* a, HulkString* b) {
    long long len = a->len + b->len;
    char* data = (char*)malloc((size_t)(len + 1));
    memcpy(data, a->data, (size_t)a->len);
    memcpy(data + a->len, b->data, (size_t)b->len);
    data[len] = '\0';
    HulkString* result = (HulkString*)malloc(sizeof(HulkString));
    result->len  = len;
    result->data = data;
    return result;
}

int hulk_string_eq(HulkString* a, HulkString* b) {
    if (a->len != b->len) return 0;
    return memcmp(a->data, b->data, (size_t)a->len) == 0;
}

int8_t hulk_string_equals(HulkString* a, HulkString* b) {
    return (int8_t)hulk_string_eq(a, b);
}

HulkString* hulk_string_from_number(double value) {
    char buf[64];
    snprintf(buf, sizeof(buf), "%.15g", value);
    size_t len = strlen(buf);
    char* data = (char*)malloc(len + 1);
    memcpy(data, buf, len + 1);
    HulkString* s = (HulkString*)malloc(sizeof(HulkString));
    s->len  = (long long)len;
    s->data = data;
    return s;
}

HulkString* hulk_string_from_bool(int8_t value) {
    const char* str = value ? "true" : "false";
    size_t len = strlen(str);
    char* data = (char*)malloc(len + 1);
    memcpy(data, str, len + 1);
    HulkString* s = (HulkString*)malloc(sizeof(HulkString));
    s->len  = (long long)len;
    s->data = data;
    return s;
}

void hulk_runtime_error(HulkString* msg) {
    if (msg && msg->data) {
        fprintf(stderr, "hulk runtime error: %.*s\n", (int)msg->len, msg->data);
    } else {
        fprintf(stderr, "hulk runtime error\n");
    }
    exit(1);
}

/* ── Legacy type descriptor ───────────────────────────────────────────── */

int hulk_type_test(HulkTypeDesc* desc, int target_id) {
    for (int i = 0; i < desc->ancestor_count; i++) {
        if (desc->ancestors[i] == target_id) return 1;
    }
    return 0;
}

void hulk_type_cast_check(void* obj, int target_id) {
    if (obj == NULL) {
        fprintf(stderr, "hulk runtime error: type cast failed (null object)\n");
        exit(1);
    }
    /* Legacy objects store HulkTypeDesc* as first field */
    HulkTypeDesc* desc = *(HulkTypeDesc**)obj;
    if (!hulk_type_test(desc, target_id)) {
        fprintf(stderr, "hulk runtime error: type cast failed\n");
        exit(1);
    }
}

/* ── Object model (vtable-based) ─────────────────────────────────────── */

/* Layout: [ HulkVTable* | int64_t attrs[attr_count] ] */
static int64_t* obj_attrs(void* obj) {
    return (int64_t*)((char*)obj + sizeof(HulkVTable*));
}

void* hulk_alloc_object(int64_t type_id, int64_t attr_count, HulkVTable* vtable) {
    (void)type_id;
    size_t size = sizeof(HulkVTable*) + (size_t)attr_count * sizeof(int64_t);
    void* obj = calloc(1, size);
    *(HulkVTable**)obj = vtable;
    return obj;
}

void* hulk_object_method(void* obj, int64_t slot) {
    HulkVTable* vtable = *(HulkVTable**)obj;
    return vtable->methods[(size_t)slot];
}

int8_t hulk_object_is(void* obj, int64_t target_type_id) {
    HulkVTable* vtable = *(HulkVTable**)obj;
    while (vtable != NULL) {
        if (vtable->type_id == target_type_id) return 1;
        vtable = vtable->parent;
    }
    return 0;
}

void* hulk_object_as(void* obj, int64_t target_type_id) {
    if (!hulk_object_is(obj, target_type_id)) {
        fprintf(stderr, "hulk runtime error: invalid type cast\n");
        exit(1);
    }
    return obj;
}

void hulk_object_set_number(void* obj, int64_t attr_id, double value) {
    int64_t* attrs = obj_attrs(obj);
    memcpy(&attrs[attr_id], &value, sizeof(double));
}

void hulk_object_set_bool(void* obj, int64_t attr_id, int8_t value) {
    int64_t* attrs = obj_attrs(obj);
    attrs[attr_id] = (int64_t)(uint8_t)value;
}

void hulk_object_set_string(void* obj, int64_t attr_id, HulkString* value) {
    int64_t* attrs = obj_attrs(obj);
    memcpy(&attrs[attr_id], &value, sizeof(void*));
}

void hulk_object_set_object(void* obj, int64_t attr_id, void* value) {
    int64_t* attrs = obj_attrs(obj);
    memcpy(&attrs[attr_id], &value, sizeof(void*));
}

double hulk_object_get_number(void* obj, int64_t attr_id) {
    int64_t* attrs = obj_attrs(obj);
    double d;
    memcpy(&d, &attrs[attr_id], sizeof(double));
    return d;
}

int8_t hulk_object_get_bool(void* obj, int64_t attr_id) {
    int64_t* attrs = obj_attrs(obj);
    return (int8_t)(attrs[attr_id] & 0xFF);
}

HulkString* hulk_object_get_string(void* obj, int64_t attr_id) {
    int64_t* attrs = obj_attrs(obj);
    HulkString* s;
    memcpy(&s, &attrs[attr_id], sizeof(void*));
    return s;
}

void* hulk_object_get_object(void* obj, int64_t attr_id) {
    int64_t* attrs = obj_attrs(obj);
    void* p;
    memcpy(&p, &attrs[attr_id], sizeof(void*));
    return p;
}

/* ── Vectors ──────────────────────────────────────────────────────────── */

/* Forward declarations so we can reference them in the vtable */
static int    hulk_vector_next_impl(HulkVector* v);
static double hulk_vector_current_impl(HulkVector* v);

static void* hulk_vector_vtable_methods[] = {
    (void*)hulk_vector_next_impl,
    (void*)hulk_vector_current_impl,
};

/* Static HulkVTable for all HulkVector instances.
   type_id = -1, no parent (pure iterable, not in user hierarchy). */
static HulkVTable hulk_vector_vtable_obj = {
    -1,
    NULL,
    2,
    hulk_vector_vtable_methods,
};

HulkVector* hulk_vector_new(void) {
    HulkVector* v = (HulkVector*)malloc(sizeof(HulkVector));
    v->vtable = &hulk_vector_vtable_obj;
    v->data   = NULL;
    v->len    = 0;
    v->cap    = 0;
    v->cursor = -1;
    return v;
}

void hulk_vector_push(HulkVector* v, int64_t elem) {
    if (v->len == v->cap) {
        int64_t new_cap = v->cap == 0 ? 8 : v->cap * 2;
        v->data = (int64_t*)realloc(v->data, (size_t)(new_cap * (int64_t)sizeof(int64_t)));
        v->cap  = new_cap;
    }
    v->data[v->len++] = elem;
}

int64_t hulk_vector_get(HulkVector* v, int64_t idx) {
    return v->data[idx];
}

void hulk_vector_set(HulkVector* v, int64_t idx, int64_t elem) {
    v->data[idx] = elem;
}

int64_t hulk_vector_len(HulkVector* v) {
    return v->len;
}

static int hulk_vector_next_impl(HulkVector* v) {
    v->cursor++;
    return v->cursor < v->len;
}

static double hulk_vector_current_impl(HulkVector* v) {
    int64_t bits = v->data[v->cursor];
    double d;
    __builtin_memcpy(&d, &bits, sizeof(d));
    return d;
}

/* Public aliases (used in hulk_runtime.h declarations) */
int    hulk_vector_next(HulkVector* v)    { return hulk_vector_next_impl(v); }
double hulk_vector_current(HulkVector* v) { return hulk_vector_current_impl(v); }

/* ── Ranges ───────────────────────────────────────────────────────────── */

static int    hulk_range_next_impl(HulkRange* r);
static double hulk_range_current_impl(HulkRange* r);

static void* hulk_range_vtable_methods[] = {
    (void*)hulk_range_next_impl,
    (void*)hulk_range_current_impl,
};

/* type_id = -2 */
static HulkVTable hulk_range_vtable_obj = {
    -2,
    NULL,
    2,
    hulk_range_vtable_methods,
};

HulkRange* hulk_range(double start, double end) {
    HulkRange* r = (HulkRange*)malloc(sizeof(HulkRange));
    r->vtable      = &hulk_range_vtable_obj;
    r->current_val = start - 1.0;
    r->end_val     = end;
    return r;
}

static int hulk_range_next_impl(HulkRange* r) {
    r->current_val += 1.0;
    return r->current_val < r->end_val;
}

static double hulk_range_current_impl(HulkRange* r) {
    return r->current_val;
}

int    hulk_range_next(HulkRange* r)    { return hulk_range_next_impl(r); }
double hulk_range_current(HulkRange* r) { return hulk_range_current_impl(r); }

/* ── Closures ─────────────────────────────────────────────────────────── */

HulkClosure* hulk_closure_alloc(void* fn_ptr, int64_t num_captures) {
    HulkClosure* c = (HulkClosure*)malloc(
        sizeof(HulkClosure) + (size_t)(num_captures * (int64_t)sizeof(void*)));
    c->fn_ptr       = fn_ptr;
    c->num_captures = num_captures;
    return c;
}

void hulk_closure_set_capture(HulkClosure* c, int64_t idx, void* val) {
    c->captures[idx] = val;
}

void* hulk_closure_get_capture(HulkClosure* c, int64_t idx) {
    return c->captures[idx];
}
