test1:
	clang++ -c -S -Os -emit-llvm -DWASM tests/t1.cpp -o target/debug/t1.ll -I.
	opt -lowerswitch target/debug/t1.ll -S -o target/debug/t1.ll
	clang++ -g -fPIC tests/t1.cpp -o target/debug/t1 -I. -Ltarget/debug/ -Wl,--export-dynamic -Wl,--dynamic-list-data -lnitrousoxide
	LD_LIBRARY_PATH=target/debug/ perf stat ./target/debug/t1

test1r:
	clang++ -c -S -Os -emit-llvm -DWASM tests/t1.cpp -o target/release/t1.ll -I.
	opt -lowerswitch target/release/t1.ll -S -o target/release/t1.ll
	clang++ -g -fPIC tests/t1.cpp -o target/release/t1 -I. -Ltarget/release/ -Wl,--export-dynamic -lnitrousoxide
	LD_LIBRARY_PATH=target/release/ perf stat ./target/release/t1


pytest1:
	clang++ -I /usr/include/python3.6m/ -c -S -Os -emit-llvm -DWASM tests/py1.cpp -o target/debug/py1.ll -I.
	opt -lowerswitch target/debug/py1.ll -S -o target/debug/py1.ll
	clang++ -I /usr/include/python3.6m/ -g -fPIC tests/py1.cpp -o target/debug/py1 -I. -Ltarget/debug/ -Wl,--export-dynamic -lnitrousoxide -lpython3.6m
	LD_LIBRARY_PATH=target/debug/ perf stat ./target/debug/py1