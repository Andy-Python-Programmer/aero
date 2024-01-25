.PHONY: all clean install

override TARGET_DIR := target

override SYSTRACE_DIR := apps/systrace
override SYSTRACE_TARGET := $(TARGET_DIR)/systrace

override INIT_DIR := init
override INIT_TARGET := $(TARGET_DIR)/init

all: $(INIT_TARGET) $(SYSTRACE_TARGET)

$(INIT_TARGET): $(INIT_DIR)/init.c
	mkdir -p $(TARGET_DIR)
	$(CC) -o $@ $^

$(SYSTRACE_TARGET): $(SYSTRACE_DIR)
	mkdir -p $(TARGET_DIR)
	cd $(SYSTRACE_DIR) && cargo build --release
	cp $(SYSTRACE_DIR)/target/x86_64-unknown-aero/release/systrace $(SYSTRACE_TARGET)

clean:
	rm -rf $(INIT_TARGET)
	rm -rf $(SYSTRACE_TARGET)

install:
	install -d "$(DESTDIR)$(PREFIX)/bin"
	install $(INIT_TARGET) "$(DESTDIR)$(PREFIX)/bin/"
	install $(SYSTRACE_TARGET) "$(DESTDIR)$(PREFIX)/bin/"
