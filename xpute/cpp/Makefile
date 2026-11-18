# cpp/Makefile
#
# xpute's C++ is built with the platform's compiler ($CXX, or c++), and for
# a WebAssembly module with the WASI SDK's clang ($WASI_SDK_PATH, or
# /opt/wasi-sdk). The library is xpute/, included from this directory by its
# root ("xpute/kit/status/bug.hpp"); dev/test/ is the tests' runner, golden
# reader and memory, beside it and never shipped, and dev/guest.cpp the
# smallest guest. The amalgamation is the library in two files, xpute.hpp and
# xpute.cpp (amalgamation.awk), under target/amalgamation/.
#
#   make -C cpp check | test | wasm | amalgamation       (-j for parallel compiles)
#   make -C cpp module GUEST="<sources>" MODULE=<file> [STACK_SZ=<bytes>]
#
# GNU Make 3.81, as macOS ships it, is enough. Run it from cpp/, because the
# paths below are this directory's. Output goes under target/; the tests open
# their golden records from the tree's root. The tests build every file of
# the tree on its own; `wasm` compiles the amalgamation for a module, so what
# a module guest links is what ships, and links dev/guest.cpp over it as a
# module. `module` links a program's guest so: GUEST its sources and MODULE
# the module, each path absolute or from cpp/.

ROOT := ..
OUT := target
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
# A guest's module, as a Rust guest's is linked: no start files and no entry,
# so it exports only what the guest names, its memory and the link's three
# exports, and imports only `link_out` from `env`; its stack
# first, so that an overflow runs below address 0 and traps, of STACK_SZ
# bytes, which the program chooses. Its first export runs its static
# initializers (xpute/guest/link.hpp `XPUTE_LINK`).
STACK_SZ ?= 1048576
MODULE_FLAGS := -nostartfiles -Wl,--no-entry -Wl,--stack-first -Wl,-z,stack-size=$(STACK_SZ)
GUEST ?= dev/guest.cpp
MODULE ?= $(OUT)/wasm/guest.wasm
# A file names itself as the root sees it (cpp/xpute/kit/...), in __FILE__
# and what a failed test prints, though it is compiled from here, whether
# named as a source or found under -I . as a header; the amalgamation names
# its own two files, as a program that compiles them would.
PREFIX_MAP := -fmacro-prefix-map=./=cpp/ $(foreach d,xpute dev/test,-fmacro-prefix-map=$(d)/=cpp/$(d)/)
AMALGAMATION_MAP := -fmacro-prefix-map=$(AMALGAMATION)/=

SOURCES := $(sort $(shell find xpute dev/test -name '*.cpp'))
# The library is what the amalgamation holds: kit, then the link over it, then
# the guest's end, in the order the two files read.
LAYERS := kit link guest
# libc++'s abort (xpute/kit/status/verbose_abort.hpp) is read before any
# other header: the amalgamation's header opens with it, and each of the
# tree's files is compiled with it included first.
VERBOSE_ABORT := xpute/kit/status/verbose_abort.hpp
LIBRARY_HEADERS := $(VERBOSE_ABORT) $(filter-out $(VERBOSE_ABORT),$(foreach d,$(LAYERS),$(sort $(shell find xpute/$(d) -name '*.hpp'))))
LIBRARY_SOURCES := $(filter-out %.test.cpp,$(foreach d,$(LAYERS),$(filter xpute/$(d)/%,$(SOURCES))))

# The compilers' names are kept in a stamp rewritten only when they change,
# so a different $CXX or SDK rebuilds every object as a change to this file
# does.
COMPILERS := $(OUT)/compilers
$(shell mkdir -p $(OUT) && { echo '$(CXX) | $(WASI_CXX)' | cmp -s - $(COMPILERS) || echo '$(CXX) | $(WASI_CXX)' > $(COMPILERS); })

TEST_OBJECTS := $(SOURCES:%.cpp=$(OUT)/test/%.o)
# The tests' runner takes the library as an archive, so it links only what the
# tests reach: the guest's link imports `link_out`, which a host defines.
LIBRARY_OBJECTS := $(LIBRARY_SOURCES:%.cpp=$(OUT)/test/%.o)
WASM_OBJECTS := $(OUT)/wasm/xpute.o

.PHONY: check test wasm module wasi amalgamation

check: $(TEST_OBJECTS)
	@echo "cpp: $(words $(SOURCES)) files compile"

test: $(OUT)/test/xpute-test wasm
	cd $(ROOT) && ./cpp/target/test/xpute-test

wasm: wasi module
	@echo "cpp: the amalgamation compiles for wasm32, and a guest links over it as a module"

module: wasi $(MODULE)

amalgamation: $(AMALGAMATION)/xpute.hpp $(AMALGAMATION)/xpute.cpp
	@echo "cpp: cpp/target/amalgamation/xpute.hpp, xpute.cpp"

wasi:
	@test -x "$(WASI_CXX)" || { echo "cpp: the WASI SDK's clang++ is not at $(WASI_CXX); install the SDK (github.com/WebAssembly/wasi-sdk) at /opt/wasi-sdk, or set WASI_SDK_PATH to where it is" >&2; exit 1; }

$(OUT)/test/xpute-test: $(filter-out $(LIBRARY_OBJECTS),$(TEST_OBJECTS)) $(OUT)/test/libxpute.a
	@$(CXX) $^ -o $@

$(OUT)/test/libxpute.a: $(LIBRARY_OBJECTS)
	@rm -f $@ && ar rcs $@ $^

# Each file is written whole under a name of its own and then moved, so a make
# that reads it, or another writing it at once, never sees half of one.
# Each of the two files opens with the license, since a program takes them
# without the tree they came from.
LICENSE := $(ROOT)/LICENSE
LICENSE_COMMENT = sed -e 's|^|// |' -e 's| *$$||' $(LICENSE) && echo

$(AMALGAMATION)/xpute.hpp: amalgamation.awk $(LIBRARY_HEADERS) $(LICENSE) Makefile
	@mkdir -p $(@D)
	@{ $(LICENSE_COMMENT) && awk -f amalgamation.awk -v part=hpp $(LIBRARY_HEADERS); } > $@.$$$$ && mv $@.$$$$ $@ || { rm -f $@.$$$$; exit 1; }

$(AMALGAMATION)/xpute.cpp: amalgamation.awk $(LIBRARY_HEADERS) $(LIBRARY_SOURCES) $(LICENSE) Makefile
	@mkdir -p $(@D)
	@{ $(LICENSE_COMMENT) && awk -f amalgamation.awk -v part=cpp $(LIBRARY_HEADERS) $(LIBRARY_SOURCES); } > $@.$$$$ && mv $@.$$$$ $@ || { rm -f $@.$$$$; exit 1; }

# Each object names the headers it read (-MMD), so a header's change rebuilds
# what includes it. Each depends on this file and the compilers' stamp, so a
# change to the flags or the compilers rebuilds every object and so relinks what
# they make. The amalgamation's object is compiled without -I ., so nothing of
# the tree can stand in for what the two files lack. The commands run quietly;
# a compiler's errors still print.
$(OUT)/test/%.o: %.cpp Makefile $(COMPILERS)
	@mkdir -p $(@D)
	@$(CXX) $(FLAGS) -I . -include $(VERBOSE_ABORT) $(PREFIX_MAP) -MMD -MP -c $< -o $@

$(OUT)/wasm/xpute.o: $(AMALGAMATION)/xpute.cpp $(AMALGAMATION)/xpute.hpp Makefile $(COMPILERS) | wasi
	@mkdir -p $(@D)
	@"$(WASI_CXX)" $(FLAGS) $(AMALGAMATION_MAP) --target=wasm32-wasip1 -MMD -MP -c $< -o $@

# The guest's sources are compiled over the amalgamation's header alone, as a
# program's are, and linked with its object in one step.
$(MODULE): $(GUEST) $(WASM_OBJECTS) $(AMALGAMATION)/xpute.hpp Makefile $(COMPILERS) | wasi
	@mkdir -p $(@D)
	@"$(WASI_CXX)" $(FLAGS) --target=wasm32-wasip1 -I $(AMALGAMATION) $(MODULE_FLAGS) $(GUEST) $(WASM_OBJECTS) -o $@

-include $(TEST_OBJECTS:.o=.d) $(WASM_OBJECTS:.o=.d)
