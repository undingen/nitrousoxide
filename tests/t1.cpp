#include <jit.h>
#include <stdio.h>
#include <stdlib.h>

int bar;

extern "C" __attribute__ ((__noinline__)) long fib(long n) {
    if (n < 2)
        return n;
    return fib(n - 1) + fib(n - 2);
}

extern "C" __attribute__ ((__noinline__)) int foo() {
    //puts("foo\n");
    return ++bar;
    //++bar;
    //return bar + 42;
}




extern "C" __attribute__ ((__noinline__)) int foo2() {
    typedef int (*funcptr)(void);
    
    char s[10];
    s[0] = '9';
    s[1] =0;
    return 1000 + fib(atoi((char*)s));
    
    //return 1000 + foo();
}


#ifndef WASM

int main() {
    initializeJIT(42);

    bar = -100;
    loadBitcode("target/debug/t1.ll");

#if 1
    JitTarget* jit_target = createJitTarget((void*)foo2, 0);

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
