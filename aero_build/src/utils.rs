use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::CARGO;

/// Locates a dependency with the given name by getting the cargo package metadata
/// from the given path.
pub fn locate_dependency_manifest<P: AsRef<Path>>(
    path: P,
    dependency: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let package_metadata = get_package_metadata(path)?;

    let root = package_metadata["resolve"]["root"]
        .as_str()
        .expect("Invalid package metadata");

    let root_resolve = package_metadata["resolve"]["nodes"]
        .members()
        .find(|r| r["id"] == root)
        .expect("Invalid package metadata");

    let dependency = root_resolve["deps"]
        .members()
        .find(|d| d["name"] == dependency)
        .expect("Dependency not found");

    let dependency_id = dependency["pkg"]
        .as_str()
        .expect("Invalid package metadata");

    let dependency_package = package_metadata["packages"]
        .members()
        .find(|p| p["id"] == dependency_id)
        .expect("Invalid package metadata");

    let dependency_manifest = dependency_package["manifest_path"]
        .as_str()
        .expect("Invalid package metadata");

    Ok(dependency_manifest.into())
}

/// Return the resolved dependencies of a package from the specified path.
pub fn get_package_metadata<P: AsRef<Path>>(path: P) -> Result<json::JsonValue, Box<dyn Error>> {
    let mut metadata_cmd = Command::new(CARGO);

    metadata_cmd.current_dir(path);

    metadata_cmd.arg("metadata");
    metadata_cmd.arg("--format-version").arg("1");

    let output = metadata_cmd.output()?;

    if !output.status.success() {
        panic!("Failed to get package metadata")
    }

    let metadata = String::from_utf8(output.stdout)?;
    let parsed_metadata = json::parse(&metadata)?;

    Ok(parsed_metadata)
}
