#include "hulk_runtime.h"

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void hulk_runtime_error(const char* message) {
    fprintf(stderr, "HULK runtime error: %s\n", message == NULL ? "unknown error" : message);
    exit(1);
}

static HulkString* hulk_string_copy_from_cstr(const char* text) {
    if (text == NULL) {
        hulk_runtime_error("cannot create string from NULL C string");
    }

    size_t len = strlen(text);
    char* data = malloc(len + 1);
    if (data == NULL) {
        hulk_runtime_error("out of memory while allocating string data");
    }
    memcpy(data, text, len + 1);

    HulkString* string = malloc(sizeof(HulkString));
    if (string == NULL) {
        free(data);
        hulk_runtime_error("out of memory while allocating string");
    }

    string->len = (long long)len;
    string->data = data;
    return string;
}

static HulkValue* hulk_object_attr(HulkObject* object, long long attr_id) {
    if (object == NULL) {
        hulk_runtime_error("object is NULL");
    }
    if (attr_id < 0 || attr_id >= object->attr_count) {
        hulk_runtime_error("object attribute index out of range");
    }
    if (object->attrs == NULL) {
        hulk_runtime_error("object attribute storage is NULL");
    }
    return &object->attrs[attr_id];
}

static void hulk_expect_attr_tag(HulkValue* value, HulkValueTag expected) {
    if (value->tag != expected) {
        hulk_runtime_error("object attribute has incorrect type");
    }
}

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

HulkString* hulk_string_concat(HulkString* left, HulkString* right) {
    if (left == NULL || left->data == NULL) {
        hulk_runtime_error("left operand of string concatenation is NULL");
    }
    if (right == NULL || right->data == NULL) {
        hulk_runtime_error("right operand of string concatenation is NULL");
    }
    if (left->len < 0 || right->len < 0) {
        hulk_runtime_error("string length is negative");
    }

    size_t left_len = (size_t)left->len;
    size_t right_len = (size_t)right->len;
    size_t len = left_len + right_len;
    if (len < left_len || len + 1 < len) {
        hulk_runtime_error("string concatenation length overflow");
    }

    char* data = malloc(len + 1);
    if (data == NULL) {
        hulk_runtime_error("out of memory while concatenating strings");
    }
    memcpy(data, left->data, left_len);
    memcpy(data + left_len, right->data, right_len);
    data[len] = '\0';

    HulkString* string = malloc(sizeof(HulkString));
    if (string == NULL) {
        free(data);
        hulk_runtime_error("out of memory while allocating concatenated string");
    }
    string->len = (long long)len;
    string->data = data;
    return string;
}

HulkString* hulk_string_from_number(double value) {
    char buffer[64];
    int written = snprintf(buffer, sizeof(buffer), "%.15g", value);
    if (written < 0 || (size_t)written >= sizeof(buffer)) {
        hulk_runtime_error("failed to convert number to string");
    }
    return hulk_string_copy_from_cstr(buffer);
}

HulkString* hulk_string_from_bool(unsigned char value) {
    return hulk_string_copy_from_cstr(value ? "true" : "false");
}

HulkObject* hulk_alloc_object(long long type_id, long long attr_count, HulkVTable* vtable) {
    if (attr_count < 0) {
        hulk_runtime_error("object attribute count is negative");
    }
    if (vtable == NULL) {
        hulk_runtime_error("object vtable is NULL");
    }
    if ((unsigned long long)attr_count > SIZE_MAX / sizeof(HulkValue)) {
        hulk_runtime_error("object attribute allocation size overflow");
    }

    HulkObject* object = malloc(sizeof(HulkObject));
    if (object == NULL) {
        hulk_runtime_error("out of memory while allocating object");
    }

    object->type_id = type_id;
    object->vtable = vtable;
    object->attr_count = attr_count;
    object->attrs = calloc((size_t)attr_count, sizeof(HulkValue));
    if (attr_count > 0 && object->attrs == NULL) {
        free(object);
        hulk_runtime_error("out of memory while allocating object attributes");
    }
    return object;
}

void* hulk_object_method(HulkObject* object, long long slot) {
    if (object == NULL) {
        hulk_runtime_error("object is NULL");
    }
    if (object->vtable == NULL) {
        hulk_runtime_error("object vtable is NULL");
    }
    if (slot < 0 || slot >= object->vtable->method_count) {
        hulk_runtime_error("object method slot out of range");
    }
    if (object->vtable->methods == NULL) {
        hulk_runtime_error("object method table is NULL");
    }
    if (object->vtable->methods[slot] == NULL) {
        hulk_runtime_error("object method slot is NULL");
    }
    return object->vtable->methods[slot];
}

void hulk_object_set_number(HulkObject* object, long long attr_id, double value) {
    HulkValue* attr = hulk_object_attr(object, attr_id);
    attr->tag = HULK_VALUE_NUMBER;
    attr->as.number = value;
}

void hulk_object_set_bool(HulkObject* object, long long attr_id, unsigned char value) {
    HulkValue* attr = hulk_object_attr(object, attr_id);
    attr->tag = HULK_VALUE_BOOL;
    attr->as.boolean = value;
}

void hulk_object_set_string(HulkObject* object, long long attr_id, HulkString* value) {
    HulkValue* attr = hulk_object_attr(object, attr_id);
    attr->tag = HULK_VALUE_STRING;
    attr->as.string = value;
}

void hulk_object_set_object(HulkObject* object, long long attr_id, HulkObject* value) {
    HulkValue* attr = hulk_object_attr(object, attr_id);
    attr->tag = HULK_VALUE_OBJECT;
    attr->as.object = value;
}

double hulk_object_get_number(HulkObject* object, long long attr_id) {
    HulkValue* attr = hulk_object_attr(object, attr_id);
    hulk_expect_attr_tag(attr, HULK_VALUE_NUMBER);
    return attr->as.number;
}

unsigned char hulk_object_get_bool(HulkObject* object, long long attr_id) {
    HulkValue* attr = hulk_object_attr(object, attr_id);
    hulk_expect_attr_tag(attr, HULK_VALUE_BOOL);
    return attr->as.boolean;
}

HulkString* hulk_object_get_string(HulkObject* object, long long attr_id) {
    HulkValue* attr = hulk_object_attr(object, attr_id);
    hulk_expect_attr_tag(attr, HULK_VALUE_STRING);
    return attr->as.string;
}

HulkObject* hulk_object_get_object(HulkObject* object, long long attr_id) {
    HulkValue* attr = hulk_object_attr(object, attr_id);
    hulk_expect_attr_tag(attr, HULK_VALUE_OBJECT);
    return attr->as.object;
}

double hulk_sqrt(double value) {
    return sqrt(value);
}

double hulk_sin(double value) {
    return sin(value);
}

double hulk_cos(double value) {
    return cos(value);
}

double hulk_exp(double value) {
    return exp(value);
}

double hulk_log(double base, double value) {
    return log(value) / log(base);
}

double hulk_pow(double value, double exponent) {
    return pow(value, exponent);
}

double hulk_rand(void) {
    return (double)rand() / (double)RAND_MAX;
}
