#include "hulk_runtime.h"

#include <math.h>
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

static const char* hulk_string_data_or_empty(HulkString* value) {
    if (value == NULL || value->data == NULL) {
        return "";
    }
    return value->data;
}

static long long hulk_string_len_or_zero(HulkString* value) {
    if (value == NULL || value->data == NULL || value->len < 0) {
        return 0;
    }
    return value->len;
}

static HulkString* hulk_string_concat_impl(HulkString* left, HulkString* right, int with_space) {
    const char* left_data = hulk_string_data_or_empty(left);
    const char* right_data = hulk_string_data_or_empty(right);
    size_t left_len = (size_t)hulk_string_len_or_zero(left);
    size_t right_len = (size_t)hulk_string_len_or_zero(right);
    size_t total_len = left_len + right_len + (with_space ? 1 : 0);

    HulkString* result = (HulkString*)malloc(sizeof(HulkString));
    if (result == NULL) {
        return NULL;
    }

    char* buffer = (char*)malloc(total_len + 1);
    if (buffer == NULL) {
        free(result);
        return NULL;
    }

    if (left_len > 0) {
        memcpy(buffer, left_data, left_len);
    }
    if (with_space) {
        buffer[left_len] = ' ';
    }
    if (right_len > 0) {
        memcpy(buffer + left_len + (with_space ? 1 : 0), right_data, right_len);
    }
    buffer[total_len] = '\0';

    result->len = (long long)total_len;
    result->data = buffer;
    return result;
}

HulkString* hulk_string_concat(HulkString* left, HulkString* right) {
    return hulk_string_concat_impl(left, right, 0);
}

HulkString* hulk_string_concat_space(HulkString* left, HulkString* right) {
    return hulk_string_concat_impl(left, right, 1);
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
