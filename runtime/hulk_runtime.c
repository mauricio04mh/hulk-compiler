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
