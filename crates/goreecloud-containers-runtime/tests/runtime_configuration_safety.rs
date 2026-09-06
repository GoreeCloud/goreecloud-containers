use goreecloud_containers_runtime::{OciRuntimeKind, ProcessOciRuntime, RuntimeError};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "goreecloud-containers-runtime-config-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temporary test directory");
    path
}

#[test]
fn execution_runtime_rejects_relative_path_before_lookup() {
    let error = ProcessOciRuntime::new_for_execution(OciRuntimeKind::Crun, "crun")
        .expect_err("relative executable path must fail closed");
    assert!(matches!(
        error,
        RuntimeError::ExecutablePathNotAbsolute(_)
    ));
}

#[test]
fn execution_runtime_rejects_directory_path() {
    let directory = unique_temp_dir();
    let error = ProcessOciRuntime::new_for_execution(OciRuntimeKind::Runc, &directory)
        .expect_err("directory cannot be accepted as runtime executable");
    assert!(matches!(
        error,
        RuntimeError::ExecutableNotRegularFile(_)
    ));
    fs::remove_dir_all(directory).expect("remove temporary test directory");
}

#[cfg(unix)]
#[test]
fn execution_runtime_rejects_non_executable_regular_file() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = unique_temp_dir();
    let executable = directory.join("crun");
    fs::write(&executable, "#!/bin/sh\nprintf 'crun test\\n'\n").expect("write fake runtime");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o600))
        .expect("set non-executable permissions");

    let error = ProcessOciRuntime::new_for_execution(OciRuntimeKind::Crun, &executable)
        .expect_err("non-executable runtime file must fail closed");
    assert!(matches!(
        error,
        RuntimeError::ExecutableNotExecutable(_)
    ));
    fs::remove_dir_all(directory).expect("remove temporary test directory");
}

#[cfg(unix)]
#[test]
fn execution_runtime_canonicalizes_and_probes_valid_executable() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = unique_temp_dir();
    let executable = directory.join("crun");
    fs::write(
        &executable,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'crun test-version\\n'; exit 0; fi\nexit 64\n",
    )
    .expect("write fake runtime");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
        .expect("set executable permissions");

    let runtime = ProcessOciRuntime::new_for_execution(OciRuntimeKind::Crun, &executable)
        .expect("valid absolute runtime executable should be accepted");
    let probe = runtime
        .probe()
        .expect("fake runtime version probe should succeed");
    assert_eq!(probe.kind, OciRuntimeKind::Crun);
    assert_eq!(
        probe.executable,
        fs::canonicalize(&executable).expect("canonical executable")
    );
    assert_eq!(probe.version_output.trim(), "crun test-version");

    fs::remove_dir_all(directory).expect("remove temporary test directory");
}
