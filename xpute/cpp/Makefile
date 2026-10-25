# cpp/Makefile
#
# xpute's C++, built with the platform's compiler ($CXX, or c++), and the
# guest as a WebAssembly module with the WASI SDK's clang ($WASI_SDK_PATH,
# or /opt/wasi-sdk). The library is xpute/, included from this directory by
# its root ("xpute/kit/status/bug.hpp"); conformance/ is the guest the
# traces drive, beside it and never shipped. Output goes under
# ../target/cpp/, where the traces look for the guests; the tests open their
# golden records from the tree's root. GNU Make 3.81, as macOS ships it, is
# enough. Run it from cpp/: the paths below are this directory's.
#
#   make -C cpp check | test | guest | wasm | amalgamation      (-j for parallel compiles)
#
# The amalgamation is the library in two files, xpute.hpp and xpute.cpp
# (amalgamation.awk), under ../target/cpp/amalgamation/. The guests the
# traces drive are built from it, so what is traced is what ships; the
# tests build every file of the tree on its own.

ROOT := ..
OUT := $(ROOT)/target/cpp
AMALGAMATION := $(OUT)/amalgamation

ifeq ($(origin CXX),default)
CXX := c++
endif
WASI_SDK_PATH ?= /opt/wasi-sdk
WASI_CXX := $(WASI_SDK_PATH)/bin/clang++

# -ffp-contract=off: a fused multiply-add rounds once where Rust and JS
# round twice; lerp fails golden/math/scalar.tsv with contraction on.
# -Wno-missing-field-initializers: a designated initializer leaves the rest
# at their defaults, which GCC's -Wextra calls missing and clang's does not.
FLAGS := -std=c++20 -fno-exceptions -fno-rtti -ffp-contract=off -Wall -Wextra -Wno-missing-field-initializers -Werror -g
# A file names itself as the root sees it (cpp/xpute/kit/...), in __FILE__
# and what a failed test prints, though it is compiled from here, whether
# named as a source or found under -I . as a header; the amalgamation names
# its own two files, as a program that compiles them would.
PREFIX_MAP := -fmacro-prefix-map=./=cpp/ $(foreach d,xpute conformance,-fmacro-prefix-map=$(d)/=cpp/$(d)/)
AMALGAMATION_MAP := -fmacro-prefix-map=$(AMALGAMATION)/= -fmacro-prefix-map=$(OUT)/conformance/=cpp/conformance/
# The guest's stack (spec/conformance/guest.json STACK_BYTES).
STACK_BYTES := 1048576
WASM_LINK := --target=wasm32-wasip1 -nostartfiles -Wl,--no-entry -Wl,--import-memory -Wl,--stack-first \
	-Wl,-z,stack-size=$(STACK_BYTES) -Wl,--export=xpute_conformance_boot -Wl,--export=xpute_conformance_interrupt

SOURCES := $(sort $(shell find xpute conformance -name '*.cpp'))
# What only the tests build: their own files, the runner and the golden reader.
TEST_ONLY := xpute/kit/test.cpp xpute/kit/test.hpp xpute/kit/golden.cpp xpute/kit/golden.hpp
# The library, what the amalgamation holds: kit, then the guest end over it,
# then the GPU's calls over that, in the order the two files read.
LAYERS := kit guest gpu
LIBRARY_HEADERS := $(filter-out $(TEST_ONLY),$(foreach d,$(LAYERS),$(sort $(shell find xpute/$(d) -name '*.hpp'))))
LIBRARY_SOURCES := $(filter-out %.test.cpp $(TEST_ONLY),$(foreach d,$(LAYERS),$(filter xpute/$(d)/%,$(SOURCES))))

ifeq ($(OS),Windows_NT)
LIBRARY := xpute_conformance.dll
else ifeq ($(shell uname -s),Darwin)
LIBRARY := libxpute_conformance.dylib
else
LIBRARY := libxpute_conformance.so
endif

# The compilers named, kept in a stamp rewritten only when they change, so a
# different $CXX or SDK rebuilds every object as a change to this file does.
COMPILERS := $(OUT)/compilers
$(shell mkdir -p $(OUT) && { echo '$(CXX) | $(WASI_CXX)' | cmp -s - $(COMPILERS) || echo '$(CXX) | $(WASI_CXX)' > $(COMPILERS); })

TEST_OBJECTS := $(SOURCES:%.cpp=$(OUT)/test/%.o)
# The guest: the amalgamation, and the conformance guest's own source with
# its includes of the library made one of xpute.hpp.
GUEST_OBJECTS := $(OUT)/guest/xpute.o $(OUT)/guest/conformance.o
WASM_OBJECTS := $(OUT)/wasm/xpute.o $(OUT)/wasm/conformance.o

.PHONY: check test guest wasm wasi amalgamation

check: $(TEST_OBJECTS)
	@echo "cpp: $(words $(SOURCES)) files compile"

# The tests, and the amalgamation compiled alone, natively and as wasm, into
# the guests the traces drive.
test: $(OUT)/test/xpute-test guest wasm
	cd $(ROOT) && ./target/cpp/test/xpute-test

guest: $(OUT)/$(LIBRARY)
	@echo "cpp: target/cpp/$(LIBRARY)"

wasm: wasi $(OUT)/xpute_conformance.wasm
	@echo "cpp: target/cpp/xpute_conformance.wasm"

amalgamation: $(AMALGAMATION)/xpute.hpp $(AMALGAMATION)/xpute.cpp
	@echo "cpp: target/cpp/amalgamation/xpute.hpp, xpute.cpp"

wasi:
	@test -x "$(WASI_CXX)" || { echo "cpp: the WASI SDK's clang++ is not at $(WASI_CXX); install the SDK (github.com/WebAssembly/wasi-sdk) at /opt/wasi-sdk, or set WASI_SDK_PATH to where it is" >&2; exit 1; }

$(OUT)/test/xpute-test: $(TEST_OBJECTS)
	@$(CXX) $^ -o $@

$(OUT)/$(LIBRARY): $(GUEST_OBJECTS)
	@$(CXX) -shared $^ -o $@

$(OUT)/xpute_conformance.wasm: $(WASM_OBJECTS)
	@"$(WASI_CXX)" $(WASM_LINK) $^ -o $@

# Each written whole under a name of its own and then moved, so a make that
# reads it, or another writing it at once, never sees half of one.
$(AMALGAMATION)/xpute.hpp: amalgamation.awk $(LIBRARY_HEADERS) Makefile
	@mkdir -p $(@D)
	@awk -f amalgamation.awk -v part=hpp $(LIBRARY_HEADERS) > $@.$$$$ && mv $@.$$$$ $@ || { rm -f $@.$$$$; exit 1; }

$(AMALGAMATION)/xpute.cpp: amalgamation.awk $(LIBRARY_HEADERS) $(LIBRARY_SOURCES) Makefile
	@mkdir -p $(@D)
	@awk -f amalgamation.awk -v part=cpp $(LIBRARY_HEADERS) $(LIBRARY_SOURCES) > $@.$$$$ && mv $@.$$$$ $@ || { rm -f $@.$$$$; exit 1; }

$(OUT)/conformance/guest.cpp: conformance/guest.cpp amalgamation.awk $(LIBRARY_HEADERS) Makefile
	@mkdir -p $(@D)
	@awk -f amalgamation.awk -v part=program $(LIBRARY_HEADERS) $< > $@.$$$$ && mv $@.$$$$ $@ || { rm -f $@.$$$$; exit 1; }

# Each object names the headers it read (-MMD), so a header's change
# rebuilds what includes it, and depends on this file and the compilers'
# stamp, so a change to the flags or the compilers rebuilds every object and
# so relinks what they make. The amalgamation's are compiled without -I .,
# so nothing of the tree but conformance/'s own headers can stand in for
# what the two files lack. The commands run quietly; a compiler's errors
# still print.
$(OUT)/test/%.o: %.cpp Makefile $(COMPILERS)
	@mkdir -p $(@D)
	@$(CXX) $(FLAGS) -I . $(PREFIX_MAP) -MMD -MP -c $< -o $@

$(OUT)/guest/xpute.o: $(AMALGAMATION)/xpute.cpp $(AMALGAMATION)/xpute.hpp Makefile $(COMPILERS)
	@mkdir -p $(@D)
	@$(CXX) $(FLAGS) $(AMALGAMATION_MAP) -fPIC -MMD -MP -c $< -o $@

$(OUT)/guest/conformance.o: $(OUT)/conformance/guest.cpp $(AMALGAMATION)/xpute.hpp Makefile $(COMPILERS)
	@mkdir -p $(@D)
	@$(CXX) $(FLAGS) -iquote conformance -iquote $(AMALGAMATION) $(AMALGAMATION_MAP) -fPIC -MMD -MP -c $< -o $@

$(OUT)/wasm/xpute.o: $(AMALGAMATION)/xpute.cpp $(AMALGAMATION)/xpute.hpp Makefile $(COMPILERS) | wasi
	@mkdir -p $(@D)
	@"$(WASI_CXX)" $(FLAGS) $(AMALGAMATION_MAP) --target=wasm32-wasip1 -MMD -MP -c $< -o $@

$(OUT)/wasm/conformance.o: $(OUT)/conformance/guest.cpp $(AMALGAMATION)/xpute.hpp Makefile $(COMPILERS) | wasi
	@mkdir -p $(@D)
	@"$(WASI_CXX)" $(FLAGS) -iquote conformance -iquote $(AMALGAMATION) $(AMALGAMATION_MAP) --target=wasm32-wasip1 -MMD -MP -c $< -o $@

-include $(TEST_OBJECTS:.o=.d) $(GUEST_OBJECTS:.o=.d) $(WASM_OBJECTS:.o=.d)
