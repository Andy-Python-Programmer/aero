use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::string::String;
use alloc::sync::{Arc, Weak};

use crate::fs::devfs;
use crate::fs::inode::INodeInterface;
use crate::fs::FileSystem;

trait DrmDevice: Send + Sync {}

static DRM_CARD_ID: AtomicUsize = AtomicUsize::new(0);

/// The direct rendering manager (DRM) exposes the GPUs through the device filesystem. Each
/// GPU detected by the DRM is referred to as a DRM device and a device file (`/dev/dri/cardX`)
/// is created to interface with it; where X is a sequential number.
struct Drm {
    sref: Weak<Self>,

    inode: usize,
    card_id: usize,
    device: Arc<dyn DrmDevice>,
}

impl Drm {
    pub fn new(device: Arc<dyn DrmDevice>) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            inode: devfs::alloc_device_marker(),
            card_id: DRM_CARD_ID.fetch_add(1, Ordering::SeqCst),
            device,
        })
    }
}

impl INodeInterface for Drm {}

impl devfs::Device for Drm {
    fn device_marker(&self) -> usize {
        self.inode
    }

    fn device_name(&self) -> String {
        alloc::format!("card{}", self.card_id) // `/dev/dri/cardX`
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        self.sref.upgrade().unwrap()
    }
}

struct RawFramebuffer {}

impl DrmDevice for RawFramebuffer {}

fn init() {
    let dri = devfs::DEV_FILESYSTEM
        .root_dir()
        .inode()
        .mkdir("dri")
        .expect("devfs: failed to create DRM directory");

    let rfb = Drm::new(Arc::new(RawFramebuffer {}));
    devfs::install_device_at(dri, rfb).expect("ramfs: failed to install DRM device");
}

crate::module_init!(init);
