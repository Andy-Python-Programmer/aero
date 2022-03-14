use aero_syscall::*;

/// Assert that the `mmap` syscall bails out when you provide `0` as the
/// size of the mapping.
#[utest_proc::test]
pub fn zero_sized_mapping() -> Result<usize, AeroSyscallError> {
    let result = sys_mmap(
        0,
        0,
        MMapProt::PROT_READ,
        MMapFlags::MAP_ANONYOMUS | MMapFlags::MAP_PRIVATE,
        -1isize as usize,
        0,
    );

    core::assert_eq!(result, Err(AeroSyscallError::EFAULT));
    Ok(())
}
