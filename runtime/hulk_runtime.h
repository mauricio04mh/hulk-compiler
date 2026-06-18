#ifndef HULK_RUNTIME_H
#define HULK_RUNTIME_H

#include <stddef.h>

typedef struct HulkString {
    long long len;
    const char* data;
} HulkString;

typedef enum HulkValueTag {
    HULK_VALUE_EMPTY = 0,
    HULK_VALUE_NUMBER,
    HULK_VALUE_BOOL,
    HULK_VALUE_STRING,
    HULK_VALUE_OBJECT,
} HulkValueTag;

typedef struct HulkObject HulkObject;
typedef struct HulkVTable HulkVTable;

struct HulkVTable {
    long long type_id;
    HulkVTable* parent;
    long long method_count;
    void** methods;
};

typedef struct HulkValue {
    HulkValueTag tag;
    union {
        double number;
        unsigned char boolean;
        HulkString* string;
        HulkObject* object;
    } as;
} HulkValue;

struct HulkObject {
    long long type_id;
    HulkVTable* vtable;
    long long attr_count;
    HulkValue* attrs;
};

void hulk_print_number(double value);
void hulk_print_bool(unsigned char value);
void hulk_print_string(HulkString* value);

HulkString* hulk_string_concat(HulkString* left, HulkString* right);
unsigned char hulk_string_equals(HulkString* left, HulkString* right);
HulkString* hulk_string_from_number(double value);
HulkString* hulk_string_from_bool(unsigned char value);
void hulk_runtime_error(const char* message);

HulkObject* hulk_alloc_object(long long type_id, long long attr_count, HulkVTable* vtable);
void* hulk_object_method(HulkObject* object, long long slot);
unsigned char hulk_object_is(HulkObject* object, long long target_type_id);
HulkObject* hulk_object_as(HulkObject* object, long long target_type_id);

void hulk_object_set_number(HulkObject* object, long long attr_id, double value);
void hulk_object_set_bool(HulkObject* object, long long attr_id, unsigned char value);
void hulk_object_set_string(HulkObject* object, long long attr_id, HulkString* value);
void hulk_object_set_object(HulkObject* object, long long attr_id, HulkObject* value);

double hulk_object_get_number(HulkObject* object, long long attr_id);
unsigned char hulk_object_get_bool(HulkObject* object, long long attr_id);
HulkString* hulk_object_get_string(HulkObject* object, long long attr_id);
HulkObject* hulk_object_get_object(HulkObject* object, long long attr_id);

double hulk_sqrt(double value);
double hulk_sin(double value);
double hulk_cos(double value);
double hulk_exp(double value);
double hulk_log(double base, double value);
double hulk_pow(double value, double exponent);
double hulk_rand(void);

#endif
