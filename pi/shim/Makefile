#
# p88 カーネル: Circle + Rust (crates/p88-pi)
#
CIRCLEHOME = ../circle
ROOT       = ../..
RUSTLIB    = $(ROOT)/target/armv6-none-eabihf/release/libp88_pi.a

OBJS = main.o kernel.o

LIBS = $(RUSTLIB) \
       $(CIRCLEHOME)/lib/usb/libusb.a \
       $(CIRCLEHOME)/lib/input/libinput.a \
       $(CIRCLEHOME)/lib/fs/libfs.a \
       $(CIRCLEHOME)/lib/libcircle.a

include $(CIRCLEHOME)/Rules.mk

.PHONY: $(RUSTLIB)
$(RUSTLIB):
	cd $(ROOT) && cargo build -p p88-pi --release --target armv6-none-eabihf -Zbuild-std=core,alloc

-include $(DEPS)
