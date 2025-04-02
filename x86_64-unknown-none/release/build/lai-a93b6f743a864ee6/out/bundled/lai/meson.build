project('lai', 'c')

# One easy way to build LAI with custom flags (such as -ffreestanding,
# -mcmodel=kernel, -mno-red-zone etc.)
# is including it as a Meson subproject and using the 'sources' and 'includes'
# variables to build your own LAI library.

sources = files(
    'core/error.c',
    'core/eval.c',
    'core/exec.c',
    'core/exec-operand.c',
    'core/libc.c',
    'core/ns.c',
    'core/object.c',
    'core/opregion.c',
    'core/os_methods.c',
    'core/variable.c',
    'core/vsnprintf.c',
    'helpers/pc-bios.c',
    'helpers/pci.c',
    'helpers/resource.c',
    'helpers/sci.c',
    'helpers/pm.c',
    'drivers/ec.c',
    'drivers/timer.c',
)

includes = include_directories('include')

library = static_library('lai', sources,
    include_directories: includes,
    pic: false)

dependency = declare_dependency(link_with: library,
    include_directories: includes)
