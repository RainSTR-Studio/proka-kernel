.PHONY: all clean

# Verbosity control
ifeq ($(V),1)
    Q :=
else
    Q := @
endif

all:
	$(Q)echo "[INFO] Building proka-fs..."
	$(Q)cargo anaxa build

clean:
	$(Q)echo "[INFO] Cleaning proka-fs..."
	$(Q)cargo clean
