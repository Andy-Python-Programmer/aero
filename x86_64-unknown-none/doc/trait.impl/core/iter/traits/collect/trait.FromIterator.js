(function() {
    var implementors = Object.fromEntries([["aero_kernel",[["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.Cr0Flags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::Cr0Flags\">Cr0Flags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.Cr0Flags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::Cr0Flags\">Cr0Flags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.Cr3Flags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::Cr3Flags\">Cr3Flags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.Cr3Flags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::Cr3Flags\">Cr3Flags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.Cr4Flags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::Cr4Flags\">Cr4Flags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.Cr4Flags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::Cr4Flags\">Cr4Flags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.MxCsr.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::MxCsr\">MxCsr</a>&gt; for <a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.MxCsr.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::MxCsr\">MxCsr</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.RFlags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::RFlags\">RFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.RFlags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::RFlags\">RFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.XCr0Flags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::XCr0Flags\">XCr0Flags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/arch/x86_64/controlregs/struct.XCr0Flags.html\" title=\"struct aero_kernel::arch::x86_64::controlregs::XCr0Flags\">XCr0Flags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/arch/x86_64/gdt/struct.GdtEntryFlags.html\" title=\"struct aero_kernel::arch::x86_64::gdt::GdtEntryFlags\">GdtEntryFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/arch/x86_64/gdt/struct.GdtEntryFlags.html\" title=\"struct aero_kernel::arch::x86_64::gdt::GdtEntryFlags\">GdtEntryFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaBohc.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaBohc\">HbaBohc</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaBohc.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaBohc\">HbaBohc</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaCapabilities.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaCapabilities\">HbaCapabilities</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaCapabilities.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaCapabilities\">HbaCapabilities</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaCapabilities2.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaCapabilities2\">HbaCapabilities2</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaCapabilities2.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaCapabilities2\">HbaCapabilities2</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaCmdHeaderFlags.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaCmdHeaderFlags\">HbaCmdHeaderFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaCmdHeaderFlags.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaCmdHeaderFlags\">HbaCmdHeaderFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaEnclosureCtrl.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaEnclosureCtrl\">HbaEnclosureCtrl</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaEnclosureCtrl.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaEnclosureCtrl\">HbaEnclosureCtrl</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaHostCont.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaHostCont\">HbaHostCont</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaHostCont.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaHostCont\">HbaHostCont</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaPortCmd.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaPortCmd\">HbaPortCmd</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaPortCmd.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaPortCmd\">HbaPortCmd</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaPortIE.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaPortIE\">HbaPortIE</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaPortIE.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaPortIE\">HbaPortIE</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaPortIS.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaPortIS\">HbaPortIS</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ahci/struct.HbaPortIS.html\" title=\"struct aero_kernel::drivers::block::ahci::HbaPortIS\">HbaPortIS</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.BMIdeCmd.html\" title=\"struct aero_kernel::drivers::block::ide::registers::BMIdeCmd\">BMIdeCmd</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.BMIdeCmd.html\" title=\"struct aero_kernel::drivers::block::ide::registers::BMIdeCmd\">BMIdeCmd</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.BMIdeStatus.html\" title=\"struct aero_kernel::drivers::block::ide::registers::BMIdeStatus\">BMIdeStatus</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.BMIdeStatus.html\" title=\"struct aero_kernel::drivers::block::ide::registers::BMIdeStatus\">BMIdeStatus</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.BaseErrorReg.html\" title=\"struct aero_kernel::drivers::block::ide::registers::BaseErrorReg\">BaseErrorReg</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.BaseErrorReg.html\" title=\"struct aero_kernel::drivers::block::ide::registers::BaseErrorReg\">BaseErrorReg</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.BaseStatusReg.html\" title=\"struct aero_kernel::drivers::block::ide::registers::BaseStatusReg\">BaseStatusReg</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.BaseStatusReg.html\" title=\"struct aero_kernel::drivers::block::ide::registers::BaseStatusReg\">BaseStatusReg</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.CtrlDevCtrlReg.html\" title=\"struct aero_kernel::drivers::block::ide::registers::CtrlDevCtrlReg\">CtrlDevCtrlReg</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.CtrlDevCtrlReg.html\" title=\"struct aero_kernel::drivers::block::ide::registers::CtrlDevCtrlReg\">CtrlDevCtrlReg</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.CtrlDriveAddrReg.html\" title=\"struct aero_kernel::drivers::block::ide::registers::CtrlDriveAddrReg\">CtrlDriveAddrReg</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/ide/registers/struct.CtrlDriveAddrReg.html\" title=\"struct aero_kernel::drivers::block::ide::registers::CtrlDriveAddrReg\">CtrlDriveAddrReg</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/nvme/command/struct.CommandFlags.html\" title=\"struct aero_kernel::drivers::block::nvme::command::CommandFlags\">CommandFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/nvme/command/struct.CommandFlags.html\" title=\"struct aero_kernel::drivers::block::nvme::command::CommandFlags\">CommandFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/block/nvme/struct.CommandSetsSupported.html\" title=\"struct aero_kernel::drivers::block::nvme::CommandSetsSupported\">CommandSetsSupported</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/block/nvme/struct.CommandSetsSupported.html\" title=\"struct aero_kernel::drivers::block::nvme::CommandSetsSupported\">CommandSetsSupported</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.ControlFlags.html\" title=\"struct aero_kernel::drivers::e1000::ControlFlags\">ControlFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.ControlFlags.html\" title=\"struct aero_kernel::drivers::e1000::ControlFlags\">ControlFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.ECtl.html\" title=\"struct aero_kernel::drivers::e1000::ECtl\">ECtl</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.ECtl.html\" title=\"struct aero_kernel::drivers::e1000::ECtl\">ECtl</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.InterruptFlags.html\" title=\"struct aero_kernel::drivers::e1000::InterruptFlags\">InterruptFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.InterruptFlags.html\" title=\"struct aero_kernel::drivers::e1000::InterruptFlags\">InterruptFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.RCtl.html\" title=\"struct aero_kernel::drivers::e1000::RCtl\">RCtl</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.RCtl.html\" title=\"struct aero_kernel::drivers::e1000::RCtl\">RCtl</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.TCtl.html\" title=\"struct aero_kernel::drivers::e1000::TCtl\">TCtl</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.TCtl.html\" title=\"struct aero_kernel::drivers::e1000::TCtl\">TCtl</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.TStatus.html\" title=\"struct aero_kernel::drivers::e1000::TStatus\">TStatus</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/e1000/struct.TStatus.html\" title=\"struct aero_kernel::drivers::e1000::TStatus\">TStatus</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/keyboard/struct.ConfigFlags.html\" title=\"struct aero_kernel::drivers::keyboard::ConfigFlags\">ConfigFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/keyboard/struct.ConfigFlags.html\" title=\"struct aero_kernel::drivers::keyboard::ConfigFlags\">ConfigFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/mouse/struct.MouseFlags.html\" title=\"struct aero_kernel::drivers::mouse::MouseFlags\">MouseFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/mouse/struct.MouseFlags.html\" title=\"struct aero_kernel::drivers::mouse::MouseFlags\">MouseFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/pci/struct.ProgramInterface.html\" title=\"struct aero_kernel::drivers::pci::ProgramInterface\">ProgramInterface</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/pci/struct.ProgramInterface.html\" title=\"struct aero_kernel::drivers::pci::ProgramInterface\">ProgramInterface</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/uart_16550/struct.InterruptEnable.html\" title=\"struct aero_kernel::drivers::uart_16550::InterruptEnable\">InterruptEnable</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/uart_16550/struct.InterruptEnable.html\" title=\"struct aero_kernel::drivers::uart_16550::InterruptEnable\">InterruptEnable</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/drivers/uart_16550/struct.LineStatus.html\" title=\"struct aero_kernel::drivers::uart_16550::LineStatus\">LineStatus</a>&gt; for <a class=\"struct\" href=\"aero_kernel/drivers/uart_16550/struct.LineStatus.html\" title=\"struct aero_kernel::drivers::uart_16550::LineStatus\">LineStatus</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/fs/inode/struct.PollFlags.html\" title=\"struct aero_kernel::fs::inode::PollFlags\">PollFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/fs/inode/struct.PollFlags.html\" title=\"struct aero_kernel::fs::inode::PollFlags\">PollFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageTableFlags.html\" title=\"struct aero_kernel::mem::paging::page_table::PageTableFlags\">PageTableFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/mem/paging/page_table/struct.PageTableFlags.html\" title=\"struct aero_kernel::mem::paging::page_table::PageTableFlags\">PageTableFlags</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/mem/paging/struct.PageFaultErrorCode.html\" title=\"struct aero_kernel::mem::paging::PageFaultErrorCode\">PageFaultErrorCode</a>&gt; for <a class=\"struct\" href=\"aero_kernel/mem/paging/struct.PageFaultErrorCode.html\" title=\"struct aero_kernel::mem::paging::PageFaultErrorCode\">PageFaultErrorCode</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/userland/vm/struct.VmFlag.html\" title=\"struct aero_kernel::userland::vm::VmFlag\">VmFlag</a>&gt; for <a class=\"struct\" href=\"aero_kernel/userland/vm/struct.VmFlag.html\" title=\"struct aero_kernel::userland::vm::VmFlag\">VmFlag</a>"],["impl <a class=\"trait\" href=\"aero_kernel/prelude/rust_2021/trait.FromIterator.html\" title=\"trait aero_kernel::prelude::rust_2021::FromIterator\">FromIterator</a>&lt;<a class=\"struct\" href=\"aero_kernel/utils/sync/struct.WaitQueueFlags.html\" title=\"struct aero_kernel::utils::sync::WaitQueueFlags\">WaitQueueFlags</a>&gt; for <a class=\"struct\" href=\"aero_kernel/utils/sync/struct.WaitQueueFlags.html\" title=\"struct aero_kernel::utils::sync::WaitQueueFlags\">WaitQueueFlags</a>"]]],["aero_syscall",[["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/consts/struct.EPollEventFlags.html\" title=\"struct aero_syscall::consts::EPollEventFlags\">EPollEventFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/consts/struct.EPollEventFlags.html\" title=\"struct aero_syscall::consts::EPollEventFlags\">EPollEventFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/consts/struct.EPollFlags.html\" title=\"struct aero_syscall::consts::EPollFlags\">EPollFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/consts/struct.EPollFlags.html\" title=\"struct aero_syscall::consts::EPollFlags\">EPollFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/consts/struct.EventFdFlags.html\" title=\"struct aero_syscall::consts::EventFdFlags\">EventFdFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/consts/struct.EventFdFlags.html\" title=\"struct aero_syscall::consts::EventFdFlags\">EventFdFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/consts/struct.FdFlags.html\" title=\"struct aero_syscall::consts::FdFlags\">FdFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/consts/struct.FdFlags.html\" title=\"struct aero_syscall::consts::FdFlags\">FdFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/consts/struct.PollEventFlags.html\" title=\"struct aero_syscall::consts::PollEventFlags\">PollEventFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/consts/struct.PollEventFlags.html\" title=\"struct aero_syscall::consts::PollEventFlags\">PollEventFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/netlink/struct.MessageFlags.html\" title=\"struct aero_syscall::netlink::MessageFlags\">MessageFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/netlink/struct.MessageFlags.html\" title=\"struct aero_syscall::netlink::MessageFlags\">MessageFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/signal/struct.SignalFlags.html\" title=\"struct aero_syscall::signal::SignalFlags\">SignalFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/signal/struct.SignalFlags.html\" title=\"struct aero_syscall::signal::SignalFlags\">SignalFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/socket/struct.MessageFlags.html\" title=\"struct aero_syscall::socket::MessageFlags\">MessageFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/socket/struct.MessageFlags.html\" title=\"struct aero_syscall::socket::MessageFlags\">MessageFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.AtFlags.html\" title=\"struct aero_syscall::AtFlags\">AtFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.AtFlags.html\" title=\"struct aero_syscall::AtFlags\">AtFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.MMapFlags.html\" title=\"struct aero_syscall::MMapFlags\">MMapFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.MMapFlags.html\" title=\"struct aero_syscall::MMapFlags\">MMapFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.MMapProt.html\" title=\"struct aero_syscall::MMapProt\">MMapProt</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.MMapProt.html\" title=\"struct aero_syscall::MMapProt\">MMapProt</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.Mode.html\" title=\"struct aero_syscall::Mode\">Mode</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.Mode.html\" title=\"struct aero_syscall::Mode\">Mode</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.OpenFlags.html\" title=\"struct aero_syscall::OpenFlags\">OpenFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.OpenFlags.html\" title=\"struct aero_syscall::OpenFlags\">OpenFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.SocketFlags.html\" title=\"struct aero_syscall::SocketFlags\">SocketFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.SocketFlags.html\" title=\"struct aero_syscall::SocketFlags\">SocketFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.TermiosCFlag.html\" title=\"struct aero_syscall::TermiosCFlag\">TermiosCFlag</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.TermiosCFlag.html\" title=\"struct aero_syscall::TermiosCFlag\">TermiosCFlag</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.TermiosIFlag.html\" title=\"struct aero_syscall::TermiosIFlag\">TermiosIFlag</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.TermiosIFlag.html\" title=\"struct aero_syscall::TermiosIFlag\">TermiosIFlag</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.TermiosLFlag.html\" title=\"struct aero_syscall::TermiosLFlag\">TermiosLFlag</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.TermiosLFlag.html\" title=\"struct aero_syscall::TermiosLFlag\">TermiosLFlag</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.TermiosOFlag.html\" title=\"struct aero_syscall::TermiosOFlag\">TermiosOFlag</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.TermiosOFlag.html\" title=\"struct aero_syscall::TermiosOFlag\">TermiosOFlag</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"aero_syscall/struct.WaitPidFlags.html\" title=\"struct aero_syscall::WaitPidFlags\">WaitPidFlags</a>&gt; for <a class=\"struct\" href=\"aero_syscall/struct.WaitPidFlags.html\" title=\"struct aero_syscall::WaitPidFlags\">WaitPidFlags</a>"]]],["allocator_api2",[["impl&lt;I&gt; FromIterator&lt;I&gt; for <a class=\"struct\" href=\"allocator_api2/boxed/struct.Box.html\" title=\"struct allocator_api2::boxed::Box\">Box</a>&lt;[I]&gt;"],["impl&lt;T&gt; FromIterator&lt;T&gt; for <a class=\"struct\" href=\"allocator_api2/vec/struct.Vec.html\" title=\"struct allocator_api2::vec::Vec\">Vec</a>&lt;T&gt;"]]],["arrayvec",[["impl&lt;T, const CAP: usize&gt; FromIterator&lt;T&gt; for <a class=\"struct\" href=\"arrayvec/struct.ArrayVec.html\" title=\"struct arrayvec::ArrayVec\">ArrayVec</a>&lt;T, CAP&gt;"]]],["cpio_reader",[["impl FromIterator&lt;<a class=\"struct\" href=\"cpio_reader/struct.Mode.html\" title=\"struct cpio_reader::Mode\">Mode</a>&gt; for <a class=\"struct\" href=\"cpio_reader/struct.Mode.html\" title=\"struct cpio_reader::Mode\">Mode</a>"]]],["crabnet",[["impl FromIterator&lt;<a class=\"struct\" href=\"crabnet/transport/struct.TcpFlags.html\" title=\"struct crabnet::transport::TcpFlags\">TcpFlags</a>&gt; for <a class=\"struct\" href=\"crabnet/transport/struct.TcpFlags.html\" title=\"struct crabnet::transport::TcpFlags\">TcpFlags</a>"]]],["hashbrown",[["impl&lt;K, V, S, A&gt; FromIterator&lt;(K, V)&gt; for <a class=\"struct\" href=\"hashbrown/struct.HashMap.html\" title=\"struct hashbrown::HashMap\">HashMap</a>&lt;K, V, S, A&gt;<div class=\"where\">where\n    K: Eq + Hash,\n    S: BuildHasher + Default,\n    A: Default + <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</div>"],["impl&lt;T, S, A&gt; FromIterator&lt;T&gt; for <a class=\"struct\" href=\"hashbrown/struct.HashSet.html\" title=\"struct hashbrown::HashSet\">HashSet</a>&lt;T, S, A&gt;<div class=\"where\">where\n    T: Eq + Hash,\n    S: BuildHasher + Default,\n    A: Default + <a class=\"trait\" href=\"allocator_api2/stable/alloc/trait.Allocator.html\" title=\"trait allocator_api2::stable::alloc::Allocator\">Allocator</a>,</div>"]]],["limine",[["impl FromIterator&lt;<a class=\"struct\" href=\"limine/modules/struct.ModuleFlags.html\" title=\"struct limine::modules::ModuleFlags\">ModuleFlags</a>&gt; for <a class=\"struct\" href=\"limine/modules/struct.ModuleFlags.html\" title=\"struct limine::modules::ModuleFlags\">ModuleFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"limine/paging/struct.Flags.html\" title=\"struct limine::paging::Flags\">Flags</a>&gt; for <a class=\"struct\" href=\"limine/paging/struct.Flags.html\" title=\"struct limine::paging::Flags\">Flags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"limine/smp/struct.RequestFlags.html\" title=\"struct limine::smp::RequestFlags\">RequestFlags</a>&gt; for <a class=\"struct\" href=\"limine/smp/struct.RequestFlags.html\" title=\"struct limine::smp::RequestFlags\">RequestFlags</a>"],["impl FromIterator&lt;<a class=\"struct\" href=\"limine/smp/struct.ResponseFlags.html\" title=\"struct limine::smp::ResponseFlags\">ResponseFlags</a>&gt; for <a class=\"struct\" href=\"limine/smp/struct.ResponseFlags.html\" title=\"struct limine::smp::ResponseFlags\">ResponseFlags</a>"]]],["serde_json",[["impl FromIterator&lt;(String, <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>)&gt; for <a class=\"struct\" href=\"serde_json/struct.Map.html\" title=\"struct serde_json::Map\">Map</a>&lt;String, <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>&gt;"],["impl&lt;K: Into&lt;String&gt;, V: Into&lt;<a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>&gt;&gt; FromIterator&lt;(K, V)&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"],["impl&lt;T: Into&lt;<a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>&gt;&gt; FromIterator&lt;T&gt; for <a class=\"enum\" href=\"serde_json/enum.Value.html\" title=\"enum serde_json::Value\">Value</a>"]]],["vte",[["impl FromIterator&lt;<a class=\"struct\" href=\"vte/ansi/struct.KeyboardModes.html\" title=\"struct vte::ansi::KeyboardModes\">KeyboardModes</a>&gt; for <a class=\"struct\" href=\"vte/ansi/struct.KeyboardModes.html\" title=\"struct vte::ansi::KeyboardModes\">KeyboardModes</a>"]]]]);
    if (window.register_implementors) {
        window.register_implementors(implementors);
    } else {
        window.pending_implementors = implementors;
    }
})()
//{"start":57,"fragment_lengths":[20427,5592,361,199,255,301,869,1131,896,291]}