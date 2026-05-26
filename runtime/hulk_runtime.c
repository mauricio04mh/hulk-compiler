#include "hulk_runtime.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

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
