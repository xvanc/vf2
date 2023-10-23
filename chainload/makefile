define DEFAULT_VAR =
	ifeq ($(origin $1),default)
		override $(1) := $(2)
	endif
	ifeq ($(origin $1),undefined)
		override $(1) := $(2)
	endif
endef

override DEFAULT_CC := clang
$(eval $(call DEFAULT_VAR,CC,$(DEFAULT_CC)))
export CC
override DEFAULT_OBJCOPY := llvm-objcopy
$(eval $(call DEFAULT_VAR,OBJCOPY,$(DEFAULT_OBJCOPY)))
export OBJCOPY
override DEFAULT_PROFILE := dev
$(eval $(call DEFAULT_VAR,PROFILE,$(DEFAULT_PROFILE)))
export PROFILE

ifeq ($(PROFILE),dev)
override TARGET_DIR := $(shell pwd)/target/riscv64imac-unknown-none/debug
else
override TARGET_DIR := $(shell pwd)/target/riscv64imac-unknown-none/$(PROFILE)
endif

.PHONY: chainload
chainload: ./build/chainload.img

./build/chainload.img: $(TARGET_DIR)/chainload
	mkdir -p ./build
	cp $(TARGET_DIR)/chainload ./build/chainload.elf
	$(OBJCOPY) -O binary ./build/chainload.elf ./build/chainload.bin
	../tools/mkfit.sh ./build/chainload.bin ./build/chainload.img

$(TARGET_DIR)/chainload:
	cargo build --profile $(PROFILE)

-include $(TARGET_DIR)/chainload.d

.PHONY: refresh
refresh:
	rm -f $(TARGET_DIR)/chainload.d $(TARGET_DIR)/chainload

.PHONY: clean
clean:
	rm -rf build
	cargo clean
