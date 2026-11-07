# The PS3 port has two separate build paths: host-testable Rust logic and a
# no_std Rust/PPU archive. The full Horizon desktop client is not PS3-ready.

CARGO ?= cargo
HOST_CC ?= cc
TARGET ?= powerpc64-sony-ps3
# Legacy-only link experiment; this is not the PS3 OS target.
RUST_TARGET ?= powerpc64-unknown-linux-gnu
RUSTFLAGS_PPU ?= -C panic=abort
PS3_RUST_TARGET ?= powerpc64-sony-ps3
PS3_RUST_TOOLCHAIN ?= nightly
PS3_RUSTFLAGS ?= -C panic=abort -C relocation-model=static -C code-model=small -C target-feature=+altivec
PS3_LINKER ?= mold
PS3_ENTRY_MANIFEST ?= rust-entry/Cargo.toml
PROFILE ?= release
OUT ?= build
HOST_TARGET_DIR ?= $(OUT)/host-target
PS3_CHECK_TARGET_DIR ?= $(OUT)/ps3-check-target
PS3_STATICLIB_TARGET_DIR ?= $(OUT)/ps3-staticlib-target
PS3_RUST_LINK_TARGET_DIR ?= $(OUT)/rust-ps3-link
PS3_RUST_ARCHIVE ?= $(PS3_STATICLIB_TARGET_DIR)/$(PS3_RUST_TARGET)/release/libhorizon_ps3_entry.a
PPU_CC ?= ppu-gcc
PSL1GHT ?= $(PS3DEV)
PPU_RUN ?=
MAKE_SELF ?= $(PS3DEV)/bin/make_self
SFO ?= $(PS3DEV)/bin/sfo
MOLDIER ?= moldier
RSX_CG_COMPILER ?= $(PS3DEV)/bin/rsx-cg-compiler
CGCOMP ?= $(PS3DEV)/bin/cgcomp
SHADER_BLOB_MODE ?= auto
TITLE ?= Horizon PS3 Bring-up
TITLE_ID ?= HZNPS3000
PS3_LIBS ?= -lrsx -lgcm_sys -lio -lsysutil -lrt -llv2 -lm

.PHONY: check host-test abi-contract-probe libc-official-check ffi-check rust-ps3-check rust-ps3-staticlib rust-ppu-lib \
	rust-ps3-link-probe rust-ps3-link-package \
	rust-ps3-world-probe rust-ps3-world-package \
	legacy-ffi-check legacy-rust-ppu-lib rust-ppu-link-probe rust-ppu-link-probe-package \
	rust-world-link rust-world-package bridge shader-objects smoke-objects smoke-elf \
	smoke-package software-raster-test package clean

# The entire desktop workspace is not a PS3 target; check only the no_std platform crate.
check: rust-ps3-check

host-test:
	CARGO_TARGET_DIR="$(HOST_TARGET_DIR)" $(CARGO) test --manifest-path ../../Cargo.toml -p horizon-ps3-platform --no-default-features --features host-stub
	$(MAKE) software-raster-test

# Compile-only ABI contract probe against the installed PSL1GHT headers. This
# deliberately does not provide POSIX aliases or claim runtime execution.
abi-contract-probe:
	@test -n "$(PSL1GHT)" || (echo "error: set PSL1GHT or PS3DEV" >&2; exit 1)
	@test -x "$(PPU_CC)" || (echo "error: $(PPU_CC) is not installed" >&2; exit 1)
	@mkdir -p "$(OUT)"
	$(PPU_RUN) $(PPU_CC) -std=gnu11 -Wall -Wextra -Werror -I$(PSL1GHT)/ppu/include -c abi_contract_probe.c -o $(OUT)/abi_contract_probe.o
	$(PSL1GHT)/ppu/bin/powerpc64-ps3-elf-nm -u $(OUT)/abi_contract_probe.o || true
	@echo "PSL1GHT ABI contract probe compiled: $(OUT)/abi_contract_probe.o"

# Reproducible libc-only check against Rust's official PS3 target. This keeps
# std out of the test so a future PAL change cannot hide an ABI regression.
libc-official-check:
	$(CARGO) +$(PS3_RUST_TOOLCHAIN) check -Z build-std=core,alloc --manifest-path libc-check/Cargo.toml --target $(PS3_RUST_TARGET)

# Official Rust target: nightly + rust-src + build-std(core, alloc).
ffi-check: rust-ps3-check

rust-ps3-check:
	RUSTFLAGS="$(PS3_RUSTFLAGS)" CARGO_TARGET_POWERPC64_SONY_PS3_LINKER="$(PS3_LINKER)" CARGO_TARGET_DIR="$(PS3_CHECK_TARGET_DIR)" $(CARGO) +$(PS3_RUST_TOOLCHAIN) check -Z build-std=core,alloc --manifest-path ../../Cargo.toml -p horizon-ps3-platform --no-default-features --features psl1ght-ffi --target $(PS3_RUST_TARGET)

# A separate staticlib wrapper keeps the shared platform crate as an rlib,
# preserving the ordinary host test harness and its panic runtime.
rust-ps3-staticlib:
	RUSTFLAGS="$(PS3_RUSTFLAGS)" CARGO_TARGET_POWERPC64_SONY_PS3_LINKER="$(PS3_LINKER)" CARGO_TARGET_DIR="$(PS3_STATICLIB_TARGET_DIR)" $(CARGO) +$(PS3_RUST_TOOLCHAIN) build -Z build-std=core,alloc --manifest-path $(PS3_ENTRY_MANIFEST) --target $(PS3_RUST_TARGET) --release --lib
		@test -f "$(PS3_STATICLIB_TARGET_DIR)/$(PS3_RUST_TARGET)/release/libhorizon_ps3_entry.a" || (echo "error: PS3 Rust staticlib was not produced" >&2; exit 1)
		@echo "Built PS3 Rust staticlib: $(PS3_STATICLIB_TARGET_DIR)/$(PS3_RUST_TARGET)/release/libhorizon_ps3_entry.a"

# First official Rust/PSL1GHT ABI probe. This is not the Horizon client.
rust-ps3-link-probe: rust-ps3-staticlib bridge shader-objects
	@test -f "$(PS3_RUST_ARCHIVE)" || (echo "error: Rust archive not found at $(PS3_RUST_ARCHIVE)" >&2; exit 1)
	@mkdir -p "$(PS3_RUST_LINK_TARGET_DIR)"
	$(PPU_RUN) $(PPU_CC) -std=c11 -Wall -Wextra -Werror -I. -I$(PSL1GHT)/ppu/include -c rust_abi_probe.c -o $(PS3_RUST_LINK_TARGET_DIR)/rust_abi_probe.o
	$(PPU_RUN) $(PPU_CC) -o $(PS3_RUST_LINK_TARGET_DIR)/rust_abi_probe.elf $(PS3_RUST_LINK_TARGET_DIR)/rust_abi_probe.o $(OUT)/psl1ght_bridge.o $(OUT)/bringup_vertex.vpo.o $(OUT)/bringup_fragment.fpo.o $(PS3_RUST_ARCHIVE) -L$(PSL1GHT)/ppu/lib $(PS3_LIBS)
	@echo "Linked official Rust/PSL1GHT ABI probe (not the Horizon client): $(PS3_RUST_LINK_TARGET_DIR)/rust_abi_probe.elf"

rust-ps3-link-package: rust-ps3-link-probe
	@test -x "$(MOLDIER)" || (echo "error: moldier not found at $(MOLDIER)" >&2; exit 1)
	@test -x "$(MAKE_SELF)" || (echo "error: make_self not found at $(MAKE_SELF)" >&2; exit 1)
	@test -x "$(SFO)" || (echo "error: sfo tool not found at $(SFO)" >&2; exit 1)
	$(MOLDIER) patch $(PS3_RUST_LINK_TARGET_DIR)/rust_abi_probe.elf
	@mkdir -p $(PS3_RUST_LINK_TARGET_DIR)/rust-abi/USRDIR
	$(MAKE_SELF) $(PS3_RUST_LINK_TARGET_DIR)/rust_abi_probe.elf $(PS3_RUST_LINK_TARGET_DIR)/rust-abi/USRDIR/EBOOT.BIN
	$(SFO) --fromxml smoke-sfo.xml $(PS3_RUST_LINK_TARGET_DIR)/rust-abi/PARAM.SFO --title="Horizon Rust ABI Probe" --appid=HZNPS3001
	@echo "Created official Rust ABI probe package at $(PS3_RUST_LINK_TARGET_DIR)/rust-abi (not the Horizon client)."

# Experimental generated-world client. It is not hardware-validated yet.
rust-ps3-world-probe: rust-ps3-staticlib bridge shader-objects
	@test -f "$(PS3_RUST_ARCHIVE)" || (echo "error: Rust archive not found at $(PS3_RUST_ARCHIVE)" >&2; exit 1)
	@mkdir -p "$(PS3_RUST_LINK_TARGET_DIR)"
	$(PPU_RUN) $(PPU_CC) -std=c11 -Wall -Wextra -Werror -I. -I$(PSL1GHT)/ppu/include -c rust_world_probe.c -o $(PS3_RUST_LINK_TARGET_DIR)/rust_world_probe.o
	$(PPU_RUN) $(PPU_CC) -o $(PS3_RUST_LINK_TARGET_DIR)/rust_world_probe.elf $(PS3_RUST_LINK_TARGET_DIR)/rust_world_probe.o $(OUT)/psl1ght_bridge.o $(OUT)/bringup_vertex.vpo.o $(OUT)/bringup_fragment.fpo.o $(PS3_RUST_ARCHIVE) -L$(PSL1GHT)/ppu/lib $(PS3_LIBS)
	@echo "Linked experimental Rust world client (not hardware-validated): $(PS3_RUST_LINK_TARGET_DIR)/rust_world_probe.elf"

rust-ps3-world-package: rust-ps3-world-probe
	@test -x "$(MOLDIER)" || (echo "error: moldier not found at $(MOLDIER)" >&2; exit 1)
	@test -x "$(MAKE_SELF)" || (echo "error: make_self not found at $(MAKE_SELF)" >&2; exit 1)
	@test -x "$(SFO)" || (echo "error: sfo tool not found at $(SFO)" >&2; exit 1)
	$(MOLDIER) patch $(PS3_RUST_LINK_TARGET_DIR)/rust_world_probe.elf
	@mkdir -p $(PS3_RUST_LINK_TARGET_DIR)/rust-world/USRDIR
	$(MAKE_SELF) $(PS3_RUST_LINK_TARGET_DIR)/rust_world_probe.elf $(PS3_RUST_LINK_TARGET_DIR)/rust-world/USRDIR/EBOOT.BIN
	$(SFO) --fromxml smoke-sfo.xml $(PS3_RUST_LINK_TARGET_DIR)/rust-world/PARAM.SFO --title="Horizon World (unverified)" --appid=HZNPS3002
	@echo "Created experimental Rust world package at $(PS3_RUST_LINK_TARGET_DIR)/rust-world."

# Preserve the old goal name, now mapped to the official Rust PS3 staticlib.
rust-ppu-lib: rust-ps3-staticlib

# Historical ABI experiment only: Linux/PPC64 Rust objects linked by ppu-gcc
# are not proof of compatibility with the official PS3 Rust target.
legacy-ffi-check:
	@command -v $(CARGO) >/dev/null || (echo "error: cargo is not installed" >&2; exit 1)
	RUSTFLAGS="$(RUSTFLAGS_PPU)" $(CARGO) check -p horizon-ps3-platform --no-default-features --features psl1ght-ffi --target $(RUST_TARGET)

legacy-rust-ppu-lib:
	RUSTFLAGS="$(RUSTFLAGS_PPU)" $(CARGO) rustc -p horizon-ps3-platform --no-default-features --features psl1ght-ffi --target $(RUST_TARGET) -- --crate-type=staticlib

rust-ppu-link-probe: legacy-rust-ppu-lib bridge shader-objects
	$(PPU_RUN) $(PPU_CC) -std=c11 -Wall -Wextra -Werror -I. -c rust_abi_probe.c -o $(OUT)/rust_abi_probe.o
	$(PPU_RUN) $(PPU_CC) -o $(OUT)/rust_abi_probe.elf $(OUT)/rust_abi_probe.o $(OUT)/psl1ght_bridge.o $(OUT)/bringup_vertex.vpo.o $(OUT)/bringup_fragment.fpo.o ../../target/$(RUST_TARGET)/debug/libhorizon_ps3_platform.a -L$(PSL1GHT)/ppu/lib $(PS3_LIBS)
	@echo "Linked legacy Rust/PPU ABI probe (not the official PS3 Rust target): $(OUT)/rust_abi_probe.elf"

rust-ppu-link-probe-package: rust-ppu-link-probe
	@test -x "$(MAKE_SELF)" || (echo "error: make_self not found at $(MAKE_SELF)" >&2; exit 1)
	@test -x "$(SFO)" || (echo "error: sfo tool not found at $(SFO)" >&2; exit 1)
	@mkdir -p $(OUT)/rust-abi/USRDIR
	$(MAKE_SELF) $(OUT)/rust_abi_probe.elf $(OUT)/rust-abi/USRDIR/EBOOT.BIN
	$(SFO) --fromxml smoke-sfo.xml $(OUT)/rust-abi/PARAM.SFO --title="Horizon Rust ABI Probe" --appid=HZNPS3001
	@echo "Created legacy ABI probe HEN folder at $(OUT)/rust-abi; not validated for PS3 Rust target."

# Legacy cross-link experiment; not a PlayStation 3 game executable.
rust-world-link: legacy-rust-ppu-lib bridge shader-objects
	$(PPU_RUN) $(PPU_CC) -std=c11 -Wall -Wextra -Werror -I. -c rust_world_probe.c -o $(OUT)/rust_world_probe.o
	$(PPU_RUN) $(PPU_CC) -o $(OUT)/rust_world.elf $(OUT)/rust_world_probe.o $(OUT)/psl1ght_bridge.o $(OUT)/bringup_vertex.vpo.o $(OUT)/bringup_fragment.fpo.o ../../target/$(RUST_TARGET)/debug/libhorizon_ps3_platform.a -L$(PSL1GHT)/ppu/lib $(PS3_LIBS)
	@echo "Linked legacy Rust world experiment: $(OUT)/rust_world.elf"

rust-world-package: rust-world-link
	@test -x "$(MAKE_SELF)" || (echo "error: make_self not found at $(MAKE_SELF)" >&2; exit 1)
	@test -x "$(SFO)" || (echo "error: sfo tool not found at $(SFO)" >&2; exit 1)
	@mkdir -p $(OUT)/rust-world/USRDIR
	$(MAKE_SELF) $(OUT)/rust_world.elf $(OUT)/rust-world/USRDIR/EBOOT.BIN
	$(SFO) --fromxml smoke-sfo.xml $(OUT)/rust-world/PARAM.SFO --title="Horizon World (unverified)" --appid=HZNPS3002
	@echo "Created legacy world experiment HEN folder at $(OUT)/rust-world."

bridge:
	@test -n "$(PSL1GHT)" || (echo "error: set PSL1GHT or PS3DEV" >&2; exit 1)
	@command -v $(PPU_CC) >/dev/null || (echo "error: $(PPU_CC) is not installed" >&2; exit 1)
	@mkdir -p $(OUT)
	$(PPU_RUN) $(PPU_CC) -std=c11 -Wall -Wextra -Werror -I$(PSL1GHT)/ppu/include -c psl1ght_bridge.c -o $(OUT)/psl1ght_bridge.o

# Compile and embed the clean-room bring-up shaders as PPU objects.
shader-objects:
	@test -x "$(PS3DEV)/ppu/bin/powerpc64-ps3-elf-objcopy" || (echo "error: PS3 objcopy not found" >&2; exit 1)
	@mkdir -p $(OUT)/shaders
	@if [ "$(SHADER_BLOB_MODE)" = "prebuilt" ] || { [ "$(SHADER_BLOB_MODE)" = "auto" ] && [ ! -x "$(RSX_CG_COMPILER)" ]; }; then \
		test -s shaders/bringup_vertex.vpo && test -s shaders/bringup_fragment.fpo || (echo "error: prebuilt shader blobs are missing; set RSX_CG_COMPILER or SHADER_BLOB_MODE=cgcomp" >&2; exit 1); \
		cp shaders/bringup_vertex.vpo $(OUT)/shaders/bringup_vertex.vpo; \
		cp shaders/bringup_fragment.fpo $(OUT)/shaders/bringup_fragment.fpo; \
		echo "Using checked-in prebuilt RSX shader blobs (no source compiler available)."; \
	elif [ "$(SHADER_BLOB_MODE)" = "cgcomp" ]; then \
		test -x "$(CGCOMP)" || (echo "error: cgcomp not found at $(CGCOMP)" >&2; exit 1); \
		$(CGCOMP) -v shaders/bringup_vertex.vcg $(OUT)/shaders/bringup_vertex.vpo; \
		$(CGCOMP) -f shaders/bringup_fragment.fcg $(OUT)/shaders/bringup_fragment.fpo; \
	else \
		test -x "$(RSX_CG_COMPILER)" || (echo "error: rsx-cg-compiler not found at $(RSX_CG_COMPILER)" >&2; exit 1); \
		$(RSX_CG_COMPILER) -p sce_vp_rsx --emit-container $(OUT)/shaders/bringup_vertex.vpo shaders/bringup_vertex.vcg; \
		$(RSX_CG_COMPILER) -p sce_fp_rsx --emit-container $(OUT)/shaders/bringup_fragment.fpo shaders/bringup_fragment.fcg; \
	fi
	cd "$(OUT)" && $(PS3DEV)/ppu/bin/powerpc64-ps3-elf-objcopy -I binary -O elf64-powerpc -B powerpc:common --redefine-sym _binary_shaders_bringup_vertex_vpo_start=horizon_vertex_blob_start --redefine-sym _binary_shaders_bringup_vertex_vpo_end=horizon_vertex_blob_end --redefine-sym _binary_shaders_bringup_vertex_vpo_size=horizon_vertex_blob_size shaders/bringup_vertex.vpo bringup_vertex.vpo.o
	cd "$(OUT)" && $(PS3DEV)/ppu/bin/powerpc64-ps3-elf-objcopy -I binary -O elf64-powerpc -B powerpc:common --redefine-sym _binary_shaders_bringup_fragment_fpo_start=horizon_fragment_blob_start --redefine-sym _binary_shaders_bringup_fragment_fpo_end=horizon_fragment_blob_end --redefine-sym _binary_shaders_bringup_fragment_fpo_size=horizon_fragment_blob_size shaders/bringup_fragment.fpo bringup_fragment.fpo.o

# Compile both PPU translation units.
smoke-objects: bridge shader-objects
	$(PPU_RUN) $(PPU_CC) -std=c11 -Wall -Wextra -Werror -I. -c bringup.c -o $(OUT)/bringup.o

# Link the standalone RSX/controller smoke test; this is not the Horizon game.
smoke-elf: smoke-objects
	$(PPU_RUN) $(PPU_CC) -o $(OUT)/bringup.elf $(OUT)/bringup.o $(OUT)/psl1ght_bridge.o $(OUT)/bringup_vertex.vpo.o $(OUT)/bringup_fragment.fpo.o -L$(PSL1GHT)/ppu/lib $(PS3_LIBS)
	@echo "Linked PPU smoke-test ELF: $(OUT)/bringup.elf"

# Wrap the PPU ELF as the homebrew SELF expected at USRDIR/EBOOT.BIN.
smoke-package: smoke-elf
	@test -x "$(MAKE_SELF)" || (echo "error: make_self not found at $(MAKE_SELF)" >&2; exit 1)
	@test -x "$(SFO)" || (echo "error: sfo tool not found at $(SFO)" >&2; exit 1)
	@mkdir -p $(OUT)/USRDIR
	$(MAKE_SELF) $(OUT)/bringup.elf $(OUT)/USRDIR/EBOOT.BIN
	$(SFO) --fromxml smoke-sfo.xml $(OUT)/PARAM.SFO --title="$(TITLE)" --appid=$(TITLE_ID)
	@echo "Created smoke-test HEN folder at $(OUT) (not the Horizon game)."

software-raster-test:
	@mkdir -p $(OUT)
	$(HOST_CC) -std=gnu11 -Wall -Wextra -Werror -I. tests/software_raster_test.c -lm -o $(OUT)/software-raster-test
	$(OUT)/software-raster-test

package:
	@mkdir -p $(OUT)/USRDIR
	@test -f ../../target/$(TARGET)/$(PROFILE)/horizon-client || (echo "error: build horizon-client first" >&2; exit 1)
	cp ../../target/$(TARGET)/$(PROFILE)/horizon-client $(OUT)/USRDIR/EBOOT.BIN
	@echo "Created $(OUT)/USRDIR/EBOOT.BIN"
	@echo "Add the SDK-specific PARAM.SFO and homebrew metadata before copying to HEN."

clean:
	rm -rf $(OUT)
