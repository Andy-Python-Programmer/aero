/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

#[repr(u8)]
#[derive(Default, Copy, Clone)]
pub enum IdentifyCns {
    Namespace = 0x00,
    Controller = 0x01,
    ActivateList = 0x2,

    #[default]
    Unknown = u8::MAX,
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct CommandFlags: u16 {
        const QUEUE_PHYS_CONTIG = 1 << 0;
        const CQ_IRQ_ENABLED = 1 << 1;
    }
}

#[repr(u8)]
#[derive(Default, Copy, Clone)]
pub enum CommandOpcode {
    Read = 0x2,

    #[default]
    Unknown = u8::MAX,
}

#[repr(u8)]
#[derive(Default, Copy, Clone)]
pub enum AdminOpcode {
    CreateSq = 0x1,
    CreateCq = 0x5,
    Identify = 0x6,

    #[default]
    Unknown = u8::MAX,
}

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct DataPointer {
    pub prp1: u64,
    pub prp2: u64,
}

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct CommonCommand {
    pub opcode: u8,
    pub flags: u8,
    pub command_id: u16,
    pub namespace_id: u32,
    pub cdw2: [u32; 2],
    pub metadata: u64,
    pub data_ptr: DataPointer,
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

const_assert_eq!(core::mem::size_of::<CommonCommand>(), 64);

#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct IdentifyCommand {
    pub opcode: u8,
    pub flags: u8,
    pub command_id: u16,
    pub nsid: u32,
    pub reserved2: [u64; 2],
    pub data_ptr: DataPointer,
    pub cns: u8,
    pub reserved3: u8,
    pub controller_id: u16,
    pub reserved11: [u32; 5],
}

impl Into<Command> for IdentifyCommand {
    fn into(self) -> Command {
        Command { identify: self }
    }
}

const_assert_eq!(core::mem::size_of::<IdentifyCommand>(), 64);

#[derive(Debug)]
#[repr(C)]
pub struct PowerState {
    pub max_power: u16,
    pub rsvd1: u8,
    pub flags: u8,
    pub entry_lat: u32,
    pub exit_lat: u32,
    pub read_tput: u8,
    pub read_lat: u8,
    pub write_tput: u8,
    pub write_lat: u8,
    pub idle_power: u16,
    pub idle_scale: u8,
    pub rsvd2: u8,
    pub active_power: u16,
    pub active_work_scale: u8,
    pub rsvd3: [u8; 9],
}

const_assert_eq!(core::mem::size_of::<PowerState>(), 32);

#[derive(Debug)]
#[repr(C)]
pub struct IdentifyController {
    pub vid: u16,
    pub ssvid: u16,
    pub sn: [u8; 20],
    pub mn: [u8; 40],
    pub fr: [u8; 8],
    pub rab: u8,
    pub ieee: [u8; 3],
    pub cmic: u8,
    pub mdts: u8,
    pub cntlid: u16,
    pub ver: u32,
    pub rtd3r: u32,
    pub rtd3e: u32,
    pub oaes: u32,
    pub ctratt: u32,
    pub reserved100: [u8; 28],
    pub crdt1: u16,
    pub crdt2: u16,
    pub crdt3: u16,
    pub reserved134: [u8; 122],
    pub oacs: u16,
    pub acl: u8,
    pub aerl: u8,
    pub frmw: u8,
    pub lpa: u8,
    pub elpe: u8,
    pub npss: u8,
    pub avscc: u8,
    pub apsta: u8,
    pub wctemp: u16,
    pub cctemp: u16,
    pub mtfa: u16,
    pub hmpre: u32,
    pub hmmin: u32,
    pub tnvmcap: [u8; 16],
    pub unvmcap: [u8; 16],
    pub rpmbs: u32,
    pub edstt: u16,
    pub dsto: u8,
    pub fwug: u8,
    pub kas: u16,
    pub hctma: u16,
    pub mntmt: u16,
    pub mxtmt: u16,
    pub sanicap: u32,
    pub hmminds: u32,
    pub hmmaxd: u16,
    pub reserved338: [u8; 4],
    pub anatt: u8,
    pub anacap: u8,
    pub anagrpmax: u32,
    pub nanagrpid: u32,
    pub reserved352: [u8; 160],
    pub sqes: u8,
    pub cqes: u8,
    pub maxcmd: u16,
    /// This field indicates the maximum value of a valid NSID for the NVM subsystem.
    pub nn: u32,
    pub oncs: u16,
    pub fuses: u16,
    pub fna: u8,
    pub vwc: u8,
    pub awun: u16,
    pub awupf: u16,
    pub nvscc: u8,
    pub nwpc: u8,
    pub acwu: u16,
    pub reserved534: [u8; 2],
    pub sgls: u32,
    pub mnan: u32,
    pub reserved544: [u8; 224],
    pub subnqn: [u8; 256],
    pub reserved1024: [u8; 768],
    pub ioccsz: u32,
    pub iorcsz: u32,
    pub icdoff: u16,
    pub ctrattr: u8,
    pub msdbd: u8,
    pub reserved1804: [u8; 244],
    pub psd: [PowerState; 32],
    pub vs: [u8; 1024],
}

const_assert_eq!(core::mem::size_of::<IdentifyController>(), 0x1000);

#[repr(C)]
#[derive(Debug)]
pub struct LbaFormat {
    pub ms: u16,
    pub ds: u8,
    pub rp: u8,
}

const_assert_eq!(core::mem::size_of::<LbaFormat>(), 4);

#[derive(Debug)]
#[repr(C)]
pub struct IdentifyNamespace {
    pub nsze: u64,
    pub ncap: u64,
    pub nuse: u64,
    pub nsfeat: u8,
    pub nlbaf: u8,
    pub flbas: u8,
    pub mc: u8,
    pub dpc: u8,
    pub dps: u8,
    pub nmic: u8,
    pub rescap: u8,
    pub fpi: u8,
    pub dlfeat: u8,
    pub nawun: u16,
    pub nawupf: u16,
    pub nacwu: u16,
    pub nabsn: u16,
    pub nabo: u16,
    pub nabspf: u16,
    pub noiob: u16,
    pub nvmcap: [u8; 16],
    pub npwg: u16,
    pub npwa: u16,
    pub npdg: u16,
    pub npda: u16,
    pub nows: u16,
    pub reserved74: [u8; 18],
    pub anagrpid: u32,
    pub reserved96: [u8; 3],
    pub nsattr: u8,
    pub nvmsetid: u16,
    pub endgid: u16,
    pub nguid: [u8; 16],
    pub eui64: [u8; 8],
    pub lbaf: [LbaFormat; 16],
    pub reserved192: [u8; 192],
    pub vs: [u8; 3712],
}

const_assert_eq!(core::mem::size_of::<IdentifyNamespace>(), 0x1000);

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct ReadWriteCommand {
    pub opcode: u8,
    pub flags: u8,
    pub command_id: u16,
    pub nsid: u32,
    pub reserved2: u64,
    pub metadata: u64,
    pub data_ptr: DataPointer,
    pub start_lba: u64,
    pub length: u16,
    pub control: u16,
    pub ds_mgmt: u32,
    pub ref_tag: u32,
    pub app_tag: u16,
    pub app_mask: u16,
}

impl Into<Command> for ReadWriteCommand {
    fn into(self) -> Command {
        Command { rw: self }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct CreateSQCommand {
    pub opcode: u8,
    pub flags: u8,
    pub command_id: u16,
    pub reserved1: [u32; 5],
    pub prp1: u64,
    pub prp2: u64,
    pub sqid: u16,
    pub q_size: u16,
    pub sq_flags: u16,
    pub cqid: u16,
    pub reserved2: [u32; 4],
}

impl Into<Command> for CreateSQCommand {
    fn into(self) -> Command {
        Command { create_sq: self }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct CreateCQCommand {
    pub opcode: u8,
    pub flags: u8,
    pub command_id: u16,
    pub reserved1: [u32; 5],
    pub prp1: u64,
    pub prp2: u64,
    pub cqid: u16,
    pub q_size: u16,
    pub cq_flags: u16,
    pub irq_vector: u16,
    pub reserved2: [u32; 4],
}

impl Into<Command> for CreateCQCommand {
    fn into(self) -> Command {
        Command { create_cq: self }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct CompletionEntry {
    pub result: u32, // Used by admin commands to return data.
    pub reserved: u32,
    pub sq_head: u16,    // Portion of the queue that may be reclaimed
    pub sq_id: u16,      // Submission Queue that generated this entry.
    pub command_id: u16, // Command ID of the command that was completed.
    pub status: u16,     // Reason why the command failed, if it did.
}

#[repr(C)]
pub union Command {
    common: CommonCommand,
    identify: IdentifyCommand,
    rw: ReadWriteCommand,
    create_sq: CreateSQCommand,
    create_cq: CreateCQCommand,
}

const_assert_eq!(core::mem::size_of::<Command>(), 64);
