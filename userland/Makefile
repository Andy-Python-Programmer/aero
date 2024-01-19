.PHONY: all clean install

override TARGET_DIR := target

override INIT_DIR := init
override INIT_TARGET := $(TARGET_DIR)/init

all: $(INIT_TARGET)

$(INIT_TARGET): $(INIT_DIR)/init.c
	mkdir -p $(TARGET_DIR)
	$(CC) -o $@ $^

clean:
	rm -rf $(INIT_TARGET)

install:
	install -d "$(DESTDIR)$(PREFIX)/bin"
	install $(INIT_TARGET) "$(DESTDIR)$(PREFIX)/bin/"
