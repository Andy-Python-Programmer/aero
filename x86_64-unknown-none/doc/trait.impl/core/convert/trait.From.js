(function() {
    var implementors = Object.fromEntries([["aero_kernel",[["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;&amp;str&gt; for <a class=\"struct\" href=\"aero_kernel/fs/path/struct.PathBuf.html\" title=\"struct aero_kernel::fs::path::PathBuf\">PathBuf</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/fs/enum.FileSystemError.html\" title=\"enum aero_kernel::fs::FileSystemError\">FileSystemError</a>&gt; for <a class=\"enum\" href=\"aero_syscall/enum.SyscallError.html\" title=\"enum aero_syscall::SyscallError\">SyscallError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/fs/ext2/disk/enum.FileType.html\" title=\"enum aero_kernel::fs::ext2::disk::FileType\">FileType</a>&gt; for <a class=\"enum\" href=\"aero_kernel/fs/inode/enum.FileType.html\" title=\"enum aero_kernel::fs::inode::FileType\">FileType</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/fs/inode/enum.FileType.html\" title=\"enum aero_kernel::fs::inode::FileType\">FileType</a>&gt; for <a class=\"enum\" href=\"aero_syscall/enum.SysFileType.html\" title=\"enum aero_syscall::SysFileType\">SysFileType</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/addr/enum.ReadErr.html\" title=\"enum aero_kernel::mem::paging::addr::ReadErr\">ReadErr</a>&gt; for <a class=\"enum\" href=\"aero_kernel/drivers/e1000/enum.Error.html\" title=\"enum aero_kernel::drivers::e1000::Error\">Error</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/addr/enum.ReadErr.html\" title=\"enum aero_kernel::mem::paging::addr::ReadErr\">ReadErr</a>&gt; for <a class=\"enum\" href=\"aero_kernel/fs/enum.FileSystemError.html\" title=\"enum aero_kernel::fs::FileSystemError\">FileSystemError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/addr/enum.ReadErr.html\" title=\"enum aero_kernel::mem::paging::addr::ReadErr\">ReadErr</a>&gt; for <a class=\"enum\" href=\"aero_syscall/enum.SyscallError.html\" title=\"enum aero_syscall::SyscallError\">SyscallError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.PageTableCreateError.html\" title=\"enum aero_kernel::mem::paging::mapper::PageTableCreateError\">PageTableCreateError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.MapToError.html\" title=\"enum aero_kernel::mem::paging::mapper::MapToError\">MapToError</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/page/enum.Size1GiB.html\" title=\"enum aero_kernel::mem::paging::page::Size1GiB\">Size1GiB</a>&gt;"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.PageTableCreateError.html\" title=\"enum aero_kernel::mem::paging::mapper::PageTableCreateError\">PageTableCreateError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.MapToError.html\" title=\"enum aero_kernel::mem::paging::mapper::MapToError\">MapToError</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/page/enum.Size2MiB.html\" title=\"enum aero_kernel::mem::paging::page::Size2MiB\">Size2MiB</a>&gt;"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.PageTableCreateError.html\" title=\"enum aero_kernel::mem::paging::mapper::PageTableCreateError\">PageTableCreateError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.MapToError.html\" title=\"enum aero_kernel::mem::paging::mapper::MapToError\">MapToError</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/page/enum.Size4KiB.html\" title=\"enum aero_kernel::mem::paging::page::Size4KiB\">Size4KiB</a>&gt;"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.PageTableWalkError.html\" title=\"enum aero_kernel::mem::paging::mapper::PageTableWalkError\">PageTableWalkError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.FlagUpdateError.html\" title=\"enum aero_kernel::mem::paging::mapper::FlagUpdateError\">FlagUpdateError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.PageTableWalkError.html\" title=\"enum aero_kernel::mem::paging::mapper::PageTableWalkError\">PageTableWalkError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.TranslateError.html\" title=\"enum aero_kernel::mem::paging::mapper::TranslateError\">TranslateError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.PageTableWalkError.html\" title=\"enum aero_kernel::mem::paging::mapper::PageTableWalkError\">PageTableWalkError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.UnmapError.html\" title=\"enum aero_kernel::mem::paging::mapper::UnmapError\">UnmapError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/mem/paging/page_table/enum.FrameError.html\" title=\"enum aero_kernel::mem::paging::page_table::FrameError\">FrameError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/mem/paging/mapper/enum.PageTableWalkError.html\" title=\"enum aero_kernel::mem::paging::mapper::PageTableWalkError\">PageTableWalkError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/userland/signals/enum.SignalError.html\" title=\"enum aero_kernel::userland::signals::SignalError\">SignalError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/fs/enum.FileSystemError.html\" title=\"enum aero_kernel::fs::FileSystemError\">FileSystemError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/userland/signals/enum.SignalError.html\" title=\"enum aero_kernel::userland::signals::SignalError\">SignalError</a>&gt; for <a class=\"enum\" href=\"aero_syscall/enum.SyscallError.html\" title=\"enum aero_syscall::SyscallError\">SyscallError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/utils/sync/enum.WaitQueueError.html\" title=\"enum aero_kernel::utils::sync::WaitQueueError\">WaitQueueError</a>&gt; for <a class=\"enum\" href=\"aero_kernel/fs/enum.FileSystemError.html\" title=\"enum aero_kernel::fs::FileSystemError\">FileSystemError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"enum\" href=\"aero_kernel/utils/sync/enum.WaitQueueError.html\" title=\"enum aero_kernel::utils::sync::WaitQueueError\">WaitQueueError</a>&gt; for <a class=\"enum\" href=\"aero_syscall/enum.SyscallError.html\" title=\"enum aero_syscall::SyscallError\">SyscallError</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/nvme/command/struct.CreateCQCommand.html\" title=\"struct aero_kernel::drivers::block::nvme::command::CreateCQCommand\">CreateCQCommand</a>&gt; for <a class=\"union\" href=\"aero_kernel/drivers/block/nvme/command/union.Command.html\" title=\"union aero_kernel::drivers::block::nvme::command::Command\">Command</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/nvme/command/struct.CreateSQCommand.html\" title=\"struct aero_kernel::drivers::block::nvme::command::CreateSQCommand\">CreateSQCommand</a>&gt; for <a class=\"union\" href=\"aero_kernel/drivers/block/nvme/command/union.Command.html\" title=\"union aero_kernel::drivers::block::nvme::command::Command\">Command</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/nvme/command/struct.IdentifyCommand.html\" title=\"struct aero_kernel::drivers::block::nvme::command::IdentifyCommand\">IdentifyCommand</a>&gt; for <a class=\"union\" href=\"aero_kernel/drivers/block/nvme/command/union.Command.html\" title=\"union aero_kernel::drivers::block::nvme::command::Command\">Command</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/nvme/command/struct.ReadWriteCommand.html\" title=\"struct aero_kernel::drivers::block::nvme::command::ReadWriteCommand\">ReadWriteCommand</a>&gt; for <a class=\"union\" href=\"aero_kernel/drivers/block/nvme/command/union.Command.html\" title=\"union aero_kernel::drivers::block::nvme::command::Command\">Command</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/fs/inode/struct.PollFlags.html\" title=\"struct aero_kernel::fs::inode::PollFlags\">PollFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/consts/struct.EPollEventFlags.html\" title=\"struct aero_syscall::consts::EPollEventFlags\">EPollEventFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/fs/inode/struct.PollFlags.html\" title=\"struct aero_kernel::fs::inode::PollFlags\">PollFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/consts/struct.PollEventFlags.html\" title=\"struct aero_syscall::consts::PollEventFlags\">PollEventFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/fs/path/struct.PathBuf.html\" title=\"struct aero_kernel::fs::path::PathBuf\">PathBuf</a>&gt; for <a class=\"struct\" href=\"aero_kernel/prelude/rust_2021/struct.String.html\" title=\"struct aero_kernel::prelude::rust_2021::String\">String</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageOffset.html\" title=\"struct aero_kernel::mem::paging::page_table::PageOffset\">PageOffset</a>&gt; for u16"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageOffset.html\" title=\"struct aero_kernel::mem::paging::page_table::PageOffset\">PageOffset</a>&gt; for u32"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageOffset.html\" title=\"struct aero_kernel::mem::paging::page_table::PageOffset\">PageOffset</a>&gt; for u64"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageOffset.html\" title=\"struct aero_kernel::mem::paging::page_table::PageOffset\">PageOffset</a>&gt; for usize"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageTableIndex.html\" title=\"struct aero_kernel::mem::paging::page_table::PageTableIndex\">PageTableIndex</a>&gt; for u16"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageTableIndex.html\" title=\"struct aero_kernel::mem::paging::page_table::PageTableIndex\">PageTableIndex</a>&gt; for u32"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageTableIndex.html\" title=\"struct aero_kernel::mem::paging::page_table::PageTableIndex\">PageTableIndex</a>&gt; for u64"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageTableIndex.html\" title=\"struct aero_kernel::mem::paging::page_table::PageTableIndex\">PageTableIndex</a>&gt; for usize"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/prelude/rust_2021/struct.String.html\" title=\"struct aero_kernel::prelude::rust_2021::String\">String</a>&gt; for <a class=\"struct\" href=\"aero_kernel/fs/path/struct.PathBuf.html\" title=\"struct aero_kernel::fs::path::PathBuf\">PathBuf</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/syscall/fs/struct.FileDescriptor.html\" title=\"struct aero_kernel::syscall::fs::FileDescriptor\">FileDescriptor</a>&gt; for usize"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_kernel/userland/vm/struct.VmFlag.html\" title=\"struct aero_kernel::userland::vm::VmFlag\">VmFlag</a>&gt; for <a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageTableFlags.html\" title=\"struct aero_kernel::mem::paging::page_table::PageTableFlags\">PageTableFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_syscall/struct.MMapProt.html\" title=\"struct aero_syscall::MMapProt\">MMapProt</a>&gt; for <a class=\"struct\" href=\"aero_kernel/userland/vm/struct.VmFlag.html\" title=\"struct aero_kernel::userland::vm::VmFlag\">VmFlag</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"aero_syscall/struct.OpenFlags.html\" title=\"struct aero_syscall::OpenFlags\">OpenFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/utils/sync/struct.WaitQueueFlags.html\" title=\"struct aero_kernel::utils::sync::WaitQueueFlags\">WaitQueueFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;<a class=\"struct\" href=\"raw_cpuid/struct.FeatureInfo.html\" title=\"struct raw_cpuid::FeatureInfo\">FeatureInfo</a>&gt; for <a class=\"enum\" href=\"aero_kernel/arch/x86_64/apic/enum.ApicType.html\" title=\"enum aero_kernel::arch::x86_64::apic::ApicType\">ApicType</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;u8&gt; for <a class=\"enum\" href=\"aero_kernel/userland/task/enum.TaskState.html\" title=\"enum aero_kernel::userland::task::TaskState\">TaskState</a>"],["impl&lt;T: <a class=\"trait\" href=\"aero_kernel/fs/cache/trait.CacheDropper.html\" title=\"trait aero_kernel::fs::cache::CacheDropper\">CacheDropper</a>&gt; <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.From.html\" title=\"trait aero_kernel::prelude::rust_2021::From\">From</a>&lt;Arc&lt;T&gt;&gt; for <a class=\"struct\" href=\"aero_kernel/fs/cache/struct.CacheArc.html\" title=\"struct aero_kernel::fs::cache::CacheArc\">CacheArc</a>&lt;T&gt;"]]],["aero_syscall",[["impl From&lt;<a class=\"enum\" href=\"aero_syscall/signal/enum.SigProcMask.html\" title=\"enum aero_syscall::signal::SigProcMask\">SigProcMask</a>&gt; for usize"],["impl From&lt;<a class=\"enum\" href=\"aero_syscall/signal/enum.SignalHandler.html\" title=\"enum aero_syscall::signal::SignalHandler\">SignalHandler</a>&gt; for u64"],["impl From&lt;<a class=\"enum\" href=\"aero_syscall/signal/enum.SignalHandler.html\" title=\"enum aero_syscall::signal::SignalHandler\">SignalHandler</a>&gt; for usize"],["impl From&lt;<a class=\"struct\" href=\"aero_syscall/struct.SocketFlags.html\" title=\"struct aero_syscall::SocketFlags\">SocketFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.OpenFlags.html\" title=\"struct aero_syscall::OpenFlags\">OpenFlags</a>"],["impl From&lt;Duration&gt; for <a class=\"struct\" href=\"aero_syscall/struct.TimeSpec.html\" title=\"struct aero_syscall::TimeSpec\">TimeSpec</a>"],["impl From&lt;u64&gt; for <a class=\"enum\" href=\"aero_syscall/signal/enum.SigProcMask.html\" title=\"enum aero_syscall::signal::SigProcMask\">SigProcMask</a>"],["impl From&lt;u64&gt; for <a class=\"enum\" href=\"aero_syscall/signal/enum.SignalHandler.html\" title=\"enum aero_syscall::signal::SignalHandler\">SignalHandler</a>"],["impl From&lt;usize&gt; for <a class=\"enum\" href=\"aero_syscall/enum.SeekWhence.html\" title=\"enum aero_syscall::SeekWhence\">SeekWhence</a>"]]],["allocator_api2",[["impl From&lt;&amp;str&gt; for <a class=\"struct\" href=\"allocator_api2/vec/struct.Vec.html\" title=\"struct allocator_api2::vec::Vec\">Vec</a>&lt;u8&gt;"],["impl From&lt;<a class=\"enum\" href=\"allocator_api2/collections/enum.TryReserveErrorKind.html\" title=\"enum allocator_api2::collections::TryReserveErrorKind\">TryReserveErrorKind</a>&gt; for <a class=\"struct\" href=\"allocator_api2/collections/struct.TryReserveError.html\" title=\"struct allocator_api2::collections::TryReserveError\">TryReserveError</a>"],["impl From&lt;<a class=\"struct\" href=\"allocator_api2/alloc/struct.LayoutError.html\" title=\"struct allocator_api2::alloc::LayoutError\">LayoutError</a>&gt; for <a class=\"enum\" href=\"allocator_api2/collections/enum.TryReserveErrorKind.html\" title=\"enum allocator_api2::collections::TryReserveErrorKind\">TryReserveErrorKind</a>"],["impl&lt;A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a> + Default&gt; From&lt;&amp;str&gt; for <a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;str, A&gt;"],["impl&lt;A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a>&gt; From&lt;<a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;str, A&gt;&gt; for <a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;[u8], A&gt;"],["impl&lt;T&gt; From&lt;T&gt; for <a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;T&gt;"],["impl&lt;T, A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a>&gt; From&lt;<a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;[T], A&gt;&gt; for <a class=\"struct\" href=\"allocator_api2/vec/struct.Vec.html\" title=\"struct allocator_api2::vec::Vec\">Vec</a>&lt;T, A&gt;"],["impl&lt;T, A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a>&gt; From&lt;<a class=\"struct\" href=\"allocator_api2/vec/struct.Vec.html\" title=\"struct allocator_api2::vec::Vec\">Vec</a>&lt;T, A&gt;&gt; for <a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;[T], A&gt;"],["impl&lt;T, A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a>, const N: usize&gt; From&lt;<a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;[T; N], A&gt;&gt; for <a class=\"struct\" href=\"allocator_api2/vec/struct.Vec.html\" title=\"struct allocator_api2::vec::Vec\">Vec</a>&lt;T, A&gt;"],["impl&lt;T, const N: usize&gt; From&lt;[T; N]&gt; for <a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;[T]&gt;"],["impl&lt;T, const N: usize&gt; From&lt;[T; N]&gt; for <a class=\"struct\" href=\"allocator_api2/vec/struct.Vec.html\" title=\"struct allocator_api2::vec::Vec\">Vec</a>&lt;T&gt;"],["impl&lt;T: ?Sized, A&gt; From&lt;<a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;T, A&gt;&gt; for Pin&lt;<a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;T, A&gt;&gt;<div class=\"where\">where\n    A: 'static + <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a>,</div>"],["impl&lt;T: Clone&gt; From&lt;&amp;[T]&gt; for <a class=\"struct\" href=\"allocator_api2/vec/struct.Vec.html\" title=\"struct allocator_api2::vec::Vec\">Vec</a>&lt;T&gt;"],["impl&lt;T: Clone&gt; From&lt;&amp;mut [T]&gt; for <a class=\"struct\" href=\"allocator_api2/vec/struct.Vec.html\" title=\"struct allocator_api2::vec::Vec\">Vec</a>&lt;T&gt;"],["impl&lt;T: Copy, A: <a class=\"trait\" href=\"allocator_api2/alloc/trait.Allocator.html\" title=\"trait allocator_api2::alloc::Allocator\">Allocator</a> + Default&gt; From&lt;&amp;[T]&gt; for <a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;[T], A&gt;"]]],["arrayvec",[["impl&lt;T, const CAP: usize&gt; From&lt;[T; CAP]&gt; for <a class=\"struct\" href=\"arrayvec/struct.ArrayVec.html\" title=\"struct arrayvec::ArrayVec\">ArrayVec</a>&lt;T, CAP&gt;"]]],["byte_endian",[["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.BigEndian.html\" title=\"struct byte_endian::BigEndian\">BigEndian</a>&lt;u128&gt;&gt; for u128"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.BigEndian.html\" title=\"struct byte_endian::BigEndian\">BigEndian</a>&lt;u16&gt;&gt; for u16"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.BigEndian.html\" title=\"struct byte_endian::BigEndian\">BigEndian</a>&lt;u32&gt;&gt; for u32"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.BigEndian.html\" title=\"struct byte_endian::BigEndian\">BigEndian</a>&lt;u64&gt;&gt; for u64"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.BigEndian.html\" title=\"struct byte_endian::BigEndian\">BigEndian</a>&lt;u8&gt;&gt; for u8"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.BigEndian.html\" title=\"struct byte_endian::BigEndian\">BigEndian</a>&lt;usize&gt;&gt; for usize"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.LittleEndian.html\" title=\"struct byte_endian::LittleEndian\">LittleEndian</a>&lt;u128&gt;&gt; for u128"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.LittleEndian.html\" title=\"struct byte_endian::LittleEndian\">LittleEndian</a>&lt;u16&gt;&gt; for u16"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.LittleEndian.html\" title=\"struct byte_endian::LittleEndian\">LittleEndian</a>&lt;u32&gt;&gt; for u32"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.LittleEndian.html\" title=\"struct byte_endian::LittleEndian\">LittleEndian</a>&lt;u64&gt;&gt; for u64"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.LittleEndian.html\" title=\"struct byte_endian::LittleEndian\">LittleEndian</a>&lt;u8&gt;&gt; for u8"],["impl From&lt;<a class=\"struct\" href=\"byte_endian/struct.LittleEndian.html\" title=\"struct byte_endian::LittleEndian\">LittleEndian</a>&lt;usize&gt;&gt; for usize"],["impl&lt;T: <a class=\"trait\" href=\"byte_endian/trait.Endian.html\" title=\"trait byte_endian::Endian\">Endian</a>&lt;T&gt;&gt; From&lt;T&gt; for <a class=\"struct\" href=\"byte_endian/struct.BigEndian.html\" title=\"struct byte_endian::BigEndian\">BigEndian</a>&lt;T&gt;"],["impl&lt;T: <a class=\"trait\" href=\"byte_endian/trait.Endian.html\" title=\"trait byte_endian::Endian\">Endian</a>&lt;T&gt;&gt; From&lt;T&gt; for <a class=\"struct\" href=\"byte_endian/struct.LittleEndian.html\" title=\"struct byte_endian::LittleEndian\">LittleEndian</a>&lt;T&gt;"]]],["bytemuck",[["impl From&lt;<a class=\"enum\" href=\"bytemuck/enum.PodCastError.html\" title=\"enum bytemuck::PodCastError\">PodCastError</a>&gt; for <a class=\"enum\" href=\"bytemuck/checked/enum.CheckedCastError.html\" title=\"enum bytemuck::checked::CheckedCastError\">CheckedCastError</a>"]]],["crabnet",[["impl From&lt;<a class=\"struct\" href=\"crabnet/transport/struct.SeqNumber.html\" title=\"struct crabnet::transport::SeqNumber\">SeqNumber</a>&gt; for u32"],["impl From&lt;[u8; 4]&gt; for <a class=\"struct\" href=\"crabnet/network/struct.Ipv4Addr.html\" title=\"struct crabnet::network::Ipv4Addr\">Ipv4Addr</a>"],["impl From&lt;u32&gt; for <a class=\"struct\" href=\"crabnet/transport/struct.SeqNumber.html\" title=\"struct crabnet::transport::SeqNumber\">SeqNumber</a>"]]],["hashbrown",[["impl&lt;K, V, A, const N: usize&gt; From&lt;[(K, V); N]&gt; for <a class=\"struct\" href=\"hashbrown/struct.HashMap.html\" title=\"struct hashbrown::HashMap\">HashMap</a>&lt;K, V, <a class=\"type\" href=\"hashbrown/type.DefaultHashBuilder.html\" title=\"type hashbrown::DefaultHashBuilder\">DefaultHashBuilder</a>, A&gt;<div class=\"where\">where\n    K: Eq + Hash,\n    A: Default + <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</div>"],["impl&lt;T, A, const N: usize&gt; From&lt;[T; N]&gt; for <a class=\"struct\" href=\"hashbrown/struct.HashSet.html\" title=\"struct hashbrown::HashSet\">HashSet</a>&lt;T, <a class=\"type\" href=\"hashbrown/type.DefaultHashBuilder.html\" title=\"type hashbrown::DefaultHashBuilder\">DefaultHashBuilder</a>, A&gt;<div class=\"where\">where\n    T: Eq + Hash,\n    A: Default + <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</div>"],["impl&lt;T, S, A&gt; From&lt;<a class=\"struct\" href=\"hashbrown/struct.HashMap.html\" title=\"struct hashbrown::HashMap\">HashMap</a>&lt;T, (), S, A&gt;&gt; for <a class=\"struct\" href=\"hashbrown/struct.HashSet.html\" title=\"struct hashbrown::HashSet\">HashSet</a>&lt;T, S, A&gt;<div class=\"where\">where\n    A: <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</div>"]]],["limine",[["impl From&lt;u64&gt; for <a class=\"struct\" href=\"limine/memory_map/struct.EntryType.html\" title=\"struct limine::memory_map::EntryType\">EntryType</a>"],["impl From&lt;u64&gt; for <a class=\"struct\" href=\"limine/paging/struct.Mode.html\" title=\"struct limine::paging::Mode\">Mode</a>"]]],["raw_cpuid",[["impl From&lt;u32&gt; for <a class=\"enum\" href=\"raw_cpuid/enum.ExtendedRegisterType.html\" title=\"enum raw_cpuid::ExtendedRegisterType\">ExtendedRegisterType</a>"]]],["serde_json",[["impl From&lt;&amp;str&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;()&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;<a class=\"struct\" href=\"serde_json/struct.Map.html\" title=\"struct serde_json::Map\">Map</a>&lt;String, <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>&gt;&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;<a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;String&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;bool&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;f32&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;f64&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;i16&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;i16&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;i32&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;i32&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;i64&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;i64&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;i8&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;i8&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;isize&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;isize&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;u16&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;u16&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;u32&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;u32&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;u64&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;u64&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;u8&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;u8&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl From&lt;usize&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl From&lt;usize&gt; for <a class=\"struct\" href=\"serde_json/struct.Number.html\" title=\"struct serde_json::Number\">Number</a>"],["impl&lt;'a&gt; From&lt;Cow&lt;'a, str&gt;&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl&lt;T&gt; From&lt;Option&lt;T&gt;&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a><div class=\"where\">where\n    T: Into&lt;<a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>&gt;,</div>"],["impl&lt;T: Clone + Into&lt;<a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>&gt;&gt; From&lt;&amp;[T]&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl&lt;T: Into&lt;<a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>&gt;&gt; From&lt;Vec&lt;T&gt;&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl&lt;T: Into&lt;<a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>&gt;, const N: usize&gt; From&lt;[T; N]&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"]]],["spin",[["impl&lt;T, R&gt; From&lt;T&gt; for <a class=\"struct\" href=\"spin/mutex/spin/struct.SpinMutex.html\" title=\"struct spin::mutex::spin::SpinMutex\">SpinMutex</a>&lt;T, R&gt;"],["impl&lt;T, R&gt; From&lt;T&gt; for <a class=\"struct\" href=\"spin/mutex/struct.Mutex.html\" title=\"struct spin::mutex::Mutex\">Mutex</a>&lt;T, R&gt;"],["impl&lt;T, R&gt; From&lt;T&gt; for <a class=\"struct\" href=\"spin/once/struct.Once.html\" title=\"struct spin::once::Once\">Once</a>&lt;T, R&gt;"],["impl&lt;T, R&gt; From&lt;T&gt; for <a class=\"struct\" href=\"spin/rwlock/struct.RwLock.html\" title=\"struct spin::rwlock::RwLock\">RwLock</a>&lt;T, R&gt;"]]],["vte",[["impl From&lt;<a class=\"enum\" href=\"vte/ansi/enum.NamedMode.html\" title=\"enum vte::ansi::NamedMode\">NamedMode</a>&gt; for <a class=\"enum\" href=\"vte/ansi/enum.Mode.html\" title=\"enum vte::ansi::Mode\">Mode</a>"],["impl From&lt;<a class=\"enum\" href=\"vte/ansi/enum.NamedPrivateMode.html\" title=\"enum vte::ansi::NamedPrivateMode\">NamedPrivateMode</a>&gt; for <a class=\"enum\" href=\"vte/ansi/enum.PrivateMode.html\" title=\"enum vte::ansi::PrivateMode\">PrivateMode</a>"]]]]);
    if (window.register_implementors) {
        window.register_implementors(implementors);
    } else {
        window.pending_implementors = implementors;
    }
})()
//{"start":57,"fragment_lengths":[17805,1422,4646,198,2527,297,488,1568,308,185,5178,654,496]}