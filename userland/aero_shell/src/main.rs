extern crate std;

static TEST_BSS_NON_ZERO: usize = usize::MAX;
static TEST_BSS_ZEROED: usize = 0x00;

fn main() {
    {
        assert!(TEST_BSS_ZEROED == 0x00);
        assert!(TEST_BSS_NON_ZERO == usize::MAX);
    }

    aero_syscall::sys_open("/dev/stdout", 0x00);

    aero_syscall::sys_write(1, "Hello, World".as_bytes());
}
