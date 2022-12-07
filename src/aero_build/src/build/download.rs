// def download_userland_host_rust():
//     out_file = os.path.join(BUNDLED_DIR, "host-rust-prebuilt.tar.gz")

//     # we have already cloned the toolchain
//     if os.path.exists(out_file):
//         return

//     log_info("downloading prebuilt userland host rust toolchain")

//     cmd = r"""
//     wget --load-cookies /tmp/cookies.txt "https://docs.google.com/uc?export=download&confirm=$(wget --quiet --save-cookies /tmp/cookies.txt --keep-session-cookies --no-check-certificate "https://docs.google.com/uc?export=download&id=FILE_HASH" -O- | sed -rn 's/.*confirm=([0-9A-Za-z_]+).*/\1\n/p')&id=FILE_HASH" -O OUTPUT_FILE && rm -rf /tmp/cookies.txt
//     """.replace("FILE_HASH", "1TTC9qa1z-KdLaQkhgMCYxLE5nuKg4gcx").replace("OUTPUT_FILE", out_file)

//     subprocess.run(cmd, shell=True)

//     log_info("extracting prebuilt userland host rust toolchain")

//     # the toolchain is compressed, so we need to extract it
//     file = tarfile.open(out_file)
//     file.extractall(os.path.join(BUNDLED_DIR, "host-rust-prebuilt"))
//     file.close()

use std::path::Path;

use crate::constants;
use crate::utils;

pub fn download_userland_host_rust() {
    let out_file = Path::new(constants::BUNDLED_DIR).join("host-rust-prebuilt.tar.gz");

    if out_file.exists() {
        return;
    }

    utils::log_info("downloading prebuilt userland host rust toolchain");

    let command = r#"wget --load-cookies /tmp/cookies.txt "https://docs.google.com/uc?export=download&confirm=$(wget --quiet --save-cookies /tmp/cookies.txt --keep-session-cookies --no-check-certificate "https://docs.google.com/uc?export=download&id=FILE_HASH" -O- | sed -rn 's/.*confirm=([0-9A-Za-z_]+).*/\1\n/p')&id=FILE_HASH" -O OUTPUT_FILE && rm -rf /tmp/cookies.txt"#
        .replace("FILE_HASH", "1TTC9qa1z-KdLaQkhgMCYxLE5nuKg4gcx")
        .replace("OUTPUT_FILE", &out_file.into_os_string().into_string().unwrap());

    // utils::exec_stream("ls", vec!("-l", "-a"));
}
