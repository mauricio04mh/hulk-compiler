#ifndef HULK_RUNTIME_H
#define HULK_RUNTIME_H

#include <stddef.h>

typedef struct HulkString {
    long long len;
    const char* data;
} HulkString;

void hulk_print_number(double value);
void hulk_print_bool(unsigned char value);
void hulk_print_string(HulkString* value);
HulkString* hulk_string_concat(HulkString* left, HulkString* right);
HulkString* hulk_string_concat_space(HulkString* left, HulkString* right);

double hulk_sqrt(double value);
double hulk_sin(double value);
double hulk_cos(double value);
double hulk_exp(double value);
double hulk_log(double base, double value);
double hulk_pow(double value, double exponent);
double hulk_rand(void);

#endif
