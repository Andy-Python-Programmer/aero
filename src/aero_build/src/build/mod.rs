use std::boxed::Box;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

use serde_json::{from_str, Value};

use crate::constants;
use crate::frontend::{BIOS, BuildArgs, BuildMode, Cli};
use crate::utils;

mod download;

pub fn build(cli: &Cli, args: &BuildArgs) {
    if args.debug {
        std::env::set_var("AERO_BUILD_DEBUG", "true");
    }
    
    // todo: match `args` from cli.command, or unreachable!();
    // ensure xbstrap is installed
    let output = utils::run_command("python3", vec!["-m", "pip", "show", "xbstrap"], None).unwrap();
    
    if let Some(stderr) = output.stderr {
        if stderr.starts_with("WARNING: Package(s) not found: xbstrap\n") {
            // todo: return error instead of exiting the process
            utils::log_error("xbstrap not found, install with:");
            utils::log_error("python3 -m pip install xbstrap");
            return;
        }
    }
    utils::log_debug("xbstrap is installed");
    
    if args.target == "aarch64" && args.bios != BIOS::UEFI {
        utils::log_error("aarch64 requires UEFI (help: run again with `--bios=uefi`)");
        return;
    }
    
    if cli.clean {
        let src_target = Path::new("src").join("target").join(args.target.to_owned());
        let userland_target = Path::new("userland").join("target");
        if src_target.exists() {
            fs::remove_dir_all(src_target).unwrap();
            utils::log_debug(format!("cleaned src/target/{}", &args.target));
        }
        if userland_target.exists() {
            fs::remove_dir_all(userland_target).unwrap();
            utils::log_debug("cleaned userland/target/");
        }
    }

    download_bundled();

    if args.sysroot {
        // build full sysroot
        build_userland_sysroot(false);
    }

    match args.mode {
        BuildMode::OnlyBuild => {
            build_kernel(&args);
        },
        BuildMode::OnlyRun => {},
        BuildMode::BuildAndRun => {},
    }
}

fn download_bundled() {
    utils::log_debug("downloading bundled");

    let bundled = Path::new(constants::BUNDLED_DIR);
    if !bundled.exists() {
        utils::log_debug("creating bundled dir");
        fs::create_dir(bundled).unwrap();
    }

    let ovmf = bundled.join("ovmf");
    if !ovmf.exists() {
        utils::log_debug("cloning ovmf into bundled/ovmf/");
        let cmd = utils::run_command(
            "git",
            vec![
                "clone",
                "-q",
                "--depth",
                "1",
                constants::OVMF_URL,
                ovmf.to_str().unwrap(),
            ],
            None,
        )
        .unwrap()
        .log_if_exists();
    } else {
        utils::log_debug("bundled ovmf already downloaded");
    }

    let limine = bundled.join("limine");
    if !limine.exists() {
        utils::log_debug("cloning limine into bundled/limine/");
        let cmd = utils::run_command(
            "git",
            vec![
                "clone",
                "-q",
                "--branch",
                "v4.x-branch-binary",
                "--depth",
                "1",
                constants::LIMINE_URL,
                limine.to_str().unwrap(),
            ],
            None,
        )
        .unwrap()
        .log_if_exists();
    } else {
        utils::log_debug("bundled limine already downloaded");
    }

    let sysroot = Path::new(constants::SYSROOT_DIR);
    if !sysroot.exists() {
        utils::log_debug("building minimal sysroot");
        build_userland_sysroot(true);
    } else {
        utils::log_debug("minimal sysroot is already built");
    }
}

fn build_userland_sysroot(minimal: bool) {
    let sysroot = Path::new(constants::SYSROOT_DIR);
    if !sysroot.exists() {
        fs::create_dir(sysroot).unwrap();
    }

    // FIXME(xbstrap): xbstrap does not copy over the extra-files/rust/config.toml
    // file into the cargo home directory.
    let sysroot_cargo_home = Path::new(constants::SYSROOT_CARGO_HOME);
    if !sysroot_cargo_home.exists() {
        fs::create_dir(sysroot_cargo_home).unwrap();
    }

    let cwd = std::env::current_dir().expect("could not read cwd");
    let cargo_sys_cfg = sysroot_cargo_home.join("config.toml");

    if !cargo_sys_cfg.exists() {
        let cargo_cfg_path = Path::new(constants::EXTRA_FILES)
            .join("rust")
            .join("config.toml");
        let cargo_cfg = fs::read_to_string(cargo_cfg_path)
            .expect("could not read file")
            .replace("@SOURCE_ROOT@", cwd.to_str().unwrap())
            .replace(
                "@BUILD_ROOT@",
                cwd.join(constants::SYSROOT_DIR).to_str().unwrap(),
            );

        let mut cfg_file = File::create(cargo_sys_cfg).unwrap();
        writeln!(cfg_file, "{}", cargo_cfg).unwrap();
    }

    let blink = sysroot.join("bootstrap.link");
    if !blink.is_symlink() {
        symlink(cwd.join("bootstrap.yml"), blink).unwrap();
    }

    // todo: pipe to stdout and stderr
    if minimal {
        run_xbstrap(vec!["install", "-u", "bash", "coreutils"]);
    } else {
        run_xbstrap(vec!["install", "-u", "--all"]);
    }
}

// def build_userland(args):
//     # We need to check if we have host-cargo in-order for us to build
//     # our rust userland applications in `userland/`.
//     host_cargo = os.path.join(SYSROOT_DIR, "tools/host-cargo")

//     if not os.path.exists(host_cargo):
//         log_error("host-cargo not built as a part of the sysroot, skipping compilation of `userland/`")
//         return []

//     HOST_CARGO = "host-cargo/bin/cargo"
//     HOST_RUST = "host-rust/bin/rustc"
//     HOST_GCC = "host-gcc/bin/x86_64-aero-gcc"
//     HOST_BINUTILS = "host-binutils/x86_64-aero/bin"
//     PACKAGE_MLIBC = "mlibc"

//     tool_dir = get_userland_tool()
//     pkg_dir = get_userland_package()

//     def get_cargo(): return os.path.join('..', tool_dir, HOST_CARGO)
//     def get_rustc(): return os.path.join('..', tool_dir, HOST_RUST)
//     def get_gcc(): return os.path.join('..', tool_dir, HOST_GCC)
//     def get_binutils(): return os.path.join("..", tool_dir, HOST_BINUTILS)
//     def get_mlibc(): return os.path.join("..", pkg_dir, PACKAGE_MLIBC)

//     command = 'build'
//     # TODO: handle the unbased architectures.
//     cmd_args = ["--target", "x86_64-unknown-aero-system",

//                 # cargo config
//                 "--config", f"build.rustc = '{get_rustc()}'",
//                 "--config", "build.target = 'x86_64-unknown-aero-system'",
//                 "--config", f"build.rustflags = ['-C', 'link-args=-no-pie -B {get_binutils()} --sysroot {get_mlibc()}', '-lc']",
//                 "--config", f"target.x86_64-unknown-aero-system.linker = '{get_gcc()}'",

//                 "-Z", "unstable-options"]

//     if not args.debug:
//         cmd_args += ['--release']

//     if args.check:
//         command = 'check'

//     if args.test:
//         return build_cargo_workspace('userland', 'build', ['--package', 'utest', *cmd_args], get_cargo())
//     else:
//         return build_cargo_workspace('userland', command, cmd_args, get_cargo())

//     # TODO: Userland check
//     # elif args.check:
//     #     command = 'check'

fn build_userland(args: BuildArgs) {
    
}

fn build_kernel(args: &BuildArgs) {
    // command = 'build'
    // cmd_args = ['--package', 'aero_kernel',
    //             '--target', f'.cargo/{args.target}.json']
    let mut subcommand = "build";
    let target = format!(".cargo/{}.json", &args.target);
    let mut cmd_args = vec!["--package", "aero_kernel", "--target", target.as_str()];

    if !std::env::var("AERO_BUILD_DEBUG").is_ok() {
        cmd_args.push("--release");
    }

    // implement in test.rs:
    // if args.test:
    //     command = 'test'
    //     cmd_args += ['--no-run']

    if args.check {
        subcommand = "check";
    }

    // implement in docs.rs
    // elif args.document:
    //     command = 'doc'

    let features = args.features.join(",");
    if !args.features.is_empty() {
        cmd_args.push("--features");
        cmd_args.push(&features.as_str());
    }

    let usable_args: Vec<String> = cmd_args.iter().map(|x| x.to_string()).collect();
    // println!("{:#?}", usable_args);

    return build_cargo_workspace(Path::new("src"), subcommand, usable_args, None);
}

fn build_cargo_workspace(cwd: &Path, subcommand: &str, args: Vec<String>, cargo: Option<&str>) {
    let cargo_bin = cargo.unwrap_or("cargo");
    let mut new_args = vec![subcommand];
    let mut temp_args: Vec<&str> = args.iter().map(String::as_str).collect();
    new_args.append(&mut temp_args);
    new_args.push("--message-format=json");

    println!("{} {:?}", cargo_bin, new_args);
    // todo: discard stderr (pipe to /dev/null)
    // also extract_artifacts from stdout
    let result = utils::run_command(cargo_bin, &new_args, Some(cwd)).unwrap();
    // result.log_if_exists();

    return extract_artifacts(result.stdout.unwrap());
}

fn extract_artifacts(stdout: String) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();

    for line in stdout.lines() {
        // utils::log_info(line);

        let value: Value = from_str(line).unwrap();
        match &value["executable"] {
            Value::String(string) => {
                result.push(string.to_string());
            },
            Value::Null => {},
            _ => {},
        }
    }
    // println!("{result:#?}");
    result
}

fn run_xbstrap(args: Vec<&str>) {
    let sysroot = Path::new(constants::SYSROOT_DIR);
    let new_args: Vec<String> = args.iter().map(|x| String::from(*x)).collect::<Vec<String>>();
    // run normally
    let output = match utils::run_command("xbstrap", &new_args, Some(sysroot)) {
        Ok(output) => output,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                utils::log_debug("Test");
                // command not found, use bin path
                utils::run_command(
                    format!(
                        "{}/.local/bin/xbstrap",
                        String::from(std::env::var("HOME").unwrap())
                    )
                    .as_str(),
                    &new_args,
                    Some(sysroot),
                )
                .unwrap()
                .log_if_exists();
            }
            return;
        }
    };
}
