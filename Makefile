all: run

pull:
	@ echo !======== RUNNING GIT PULL ========!
	@
	@ git pull --recurse-submodules
	@ git submodule sync --recursive
	
	@ echo !======== UPDATING SUBMODULES ========!
	@ git submodule update --recursive --init

test:
	@ echo !======== RUNNING CARGO TEST ========!
	@
	@ cargo boot test

build:
	@ echo !======== RUNNING CARGO BUILD ========!
	@
	@ cargo boot build

run:
	@ @ echo !======== RUNNING CARGO RUN ========!
	@
	@ cargo boot run

clean:
	@ echo !======== RUNNING CARGO CLEAN ========!
	@
	@ cargo boot clean
