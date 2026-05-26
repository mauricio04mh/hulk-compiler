#ifndef HULK_RUNTIME_H
#define HULK_RUNTIME_H

void hulk_print_number(double value);
void hulk_print_bool(unsigned char value);

double hulk_sqrt(double value);
double hulk_sin(double value);
double hulk_cos(double value);
double hulk_exp(double value);
double hulk_log(double base, double value);
double hulk_pow(double value, double exponent);
double hulk_rand(void);

#endif
