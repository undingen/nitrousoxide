# Nitrous oxide
Is a very early proof of concept JIT for C/C++.
It's similar to https://github.com/nitrousjit/nitrous but instead of LLVM it uses cranelift (and for easier FFI it's also written in rust) and is much less sophisticated for now.

The idea is that using a very fast (as in compile time) JIT backend we may be able to not have to implement deoptimization (because we can inline everything not only the executed path).
Instead the idea is to "trace" the execution by inspecting calls (e.g through patching like a debugger does, profiler info, intel cpu extension or interpreter...) and use this info to create a very large "trace" which just consist of every functioned encountered inlined into one big one.

Currently we first compile down the C/C++ codebase to LLVM IR and then use llvm2cranelift to lower it to the cranelift IR.
At runtime we only work on the cranelift IR (right now we still use LLVM IR on disk because cranelift does not have a bitcode but immediately translate it)

__This is a very early prototype and only work for very simple cases__

## Testing
- git clone --recursive https://github.com/undingen/nitrousoxide.git
- cargo build
- make test1

## Test with CPython
- make cpython *# fetch cpython and compile it*
- make pytest1


