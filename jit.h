#ifndef _JIT_H
#define _JIT_H

extern "C" {

typedef struct _JitTarget {
    void* target_function;
    int num_args;
} JitTarget;


void initializeJIT(int verbosity);
void loadBitcode(const char* file_name);
JitTarget* createJitTarget(void* target_function, int num_args);

long runJitTarget0(JitTarget* jit_target);
long runJitTarget1(JitTarget* jit_target, long arg0);

}

#endif

