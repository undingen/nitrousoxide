PYVER=3.7.4
PYNAME=Python-$(PYVER)
PYDIR=$(shell pwd)/tests/$(PYNAME)/
PYINST=$(PYDIR)/inst
PYLIB=$(PYDIR)/inst/lib/

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
	clang++ -I $(PYINST)/include/python3.7m/ -c -S -Os -emit-llvm -DWASM tests/py1.cpp -o target/debug/py1.ll -I.
	llvm-link $(PYDIR)/extracted.bc target/debug/py1.ll -S -o target/debug/py1.ll
	opt -lowerswitch target/debug/py1.ll -S -o target/debug/py1.ll
	clang++ -I $(PYINST)/include/python3.7m/ -g -fPIC tests/py1.cpp -o target/debug/py1 -I. -Ltarget/debug/ -Wl,--export-dynamic -lnitrousoxide -L$(PYLIB) -lpython3.7m
	LD_LIBRARY_PATH="target/debug/:$(PYLIB)" perf stat ./target/debug/py1

pytest1r:
	clang++ -I $(PYINST)/include/python3.7m/ -c -S -Os -emit-llvm -DWASM tests/py1.cpp -o target/release/py1.ll -I.
	llvm-link $(PYDIR)/extracted.bc target/release/py1.ll -S -o target/release/py1.ll
	opt -lowerswitch target/release/py1.ll -S -o target/release/py1.ll
	clang++ -I $(PYINST)/include/python3.7m/ -g -fPIC tests/py1.cpp -o target/release/py1 -I. -Ltarget/release/ -Wl,--export-dynamic -lnitrousoxide -L$(PYLIB) -lpython3.7m
	LD_LIBRARY_PATH="target/release/:$(PYLIB)" perf stat ./target/release/py1

cpython:
	cd tests && wget -c https://www.python.org/ftp/python/$(PYVER)/$(PYNAME).tar.xz && tar -xf $(PYNAME).tar.xz
	cd $(PYDIR) && patch -p1 < ../$(PYNAME).patch
	cd $(PYDIR) && CC=clang ./configure --prefix=$(PYINST) --with-lto --enable-shared && make -j4 && make install
	cd $(PYDIR) && llvm-link -o linked_orig.bc ./Python/*.o  ./Objects/*.o ./Modules/*.o
	cd $(PYDIR) && opt -lowerswitch -o linked.bc linked_orig.bc
	cd $(PYDIR) && llvm-extract linked.bc -func=PyLong_FromLong -func=PyLong_AsLong -o extracted.bc
