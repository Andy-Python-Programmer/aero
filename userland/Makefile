.PHONY: all clean install

override TARGET_DIR := target

override SYSTRACE_DIR := apps/systrace
override SYSTRACE_TARGET := $(TARGET_DIR)/systrace

override TEST_DIR := tests
override TEST_TARGET = $(TARGET_DIR)/utest

override F_TARGET := $(TARGET_DIR)/f

override INIT_DIR := init
override INIT_TARGET := $(TARGET_DIR)/init

all: $(INIT_TARGET) $(SYSTRACE_TARGET) $(TEST_TARGET) $(F_TARGET)

$(INIT_TARGET): $(INIT_DIR)/init.c
	mkdir -p $(TARGET_DIR)
	$(CC) -o $@ $^

$(SYSTRACE_TARGET): $(SYSTRACE_DIR)
	mkdir -p $(TARGET_DIR)
	cd $(SYSTRACE_DIR) && cargo build --release
	cp $(SYSTRACE_DIR)/target/x86_64-unknown-aero/release/systrace $(SYSTRACE_TARGET)

$(TEST_TARGET): $(TEST_DIR)/utest.cc
	mkdir -p $(TARGET_DIR)
	$(CXX) -o $@ $^

$(F_TARGET): $(TEST_DIR)/f.c
	mkdir -p $(TARGET_DIR)
	$(CC) -o $@ $^

clean:
	rm -rf $(INIT_TARGET)
	rm -rf $(SYSTRACE_TARGET)

install:
	install -d "$(DESTDIR)$(PREFIX)/bin"
	install $(INIT_TARGET) "$(DESTDIR)$(PREFIX)/bin/"
	install $(SYSTRACE_TARGET) "$(DESTDIR)$(PREFIX)/bin/"
	install $(TEST_TARGET) "$(DESTDIR)$(PREFIX)/bin/"
	install $(F_TARGET) "$(DESTDIR)$(PREFIX)/bin/"
