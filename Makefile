jinx:
	if [ ! -d "3rdparty/jinx" ]; then \
		git clone https://github.com/mintsuki/jinx 3rdparty/jinx; \
	fi

.PHONY: distro
distro: jinx
	./3rdparty/jinx/jinx --help
