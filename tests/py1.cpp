#include <jit.h>
#include <stdio.h>
#include <stdlib.h>

#include <Python.h>

#if 0
long
_pytest2_target() {
    return PyLong_AsLong(PyLong_FromLong(400));
}
#else
long
_pytest2_target() {
    return PyLong_Check(PyLong_FromLong(600));
}
#endif

extern "C" __attribute__ ((__noinline__)) int foo() {
    return (int)_pytest2_target();
}


#ifndef WASM

int main() {
    initializeJIT(42);

    loadBitcode("target/debug/py1.ll");

#if 1
    JitTarget* jit_target = createJitTarget((void*)foo, 0);

    for (int i=0;i<3; ++i) {
        int t = runJitTarget0(jit_target);
        printf("ret %d\n", t);
    }
#else
    JitTarget* jit_target = createJitTarget((void*)fib, 1);

    for (int i=0;i<3; ++i) {
        int t = runJitTarget1(jit_target, 42);
        printf("ret %d\n", t);
    }
#endif
    return 0;
}
#endif
