#![cfg(unix)]

use goreecloud_containers_core::ContainerId;
use goreecloud_containers_engine::oci_config_from_image_configuration;
use goreecloud_containers_image::registry::{ImageConfiguration, ImageProcessConfig};
use goreecloud_containers_oci::{DEFAULT_ROOTFS_DIRECTORY, initialize_linux_bundle};
use goreecloud_containers_runtime::{OciRuntimeKind, ProcessOciRuntime, RuntimeExecutor};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn test_directory() -> PathBuf {
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "goreecloud-containers-engine-runtime-lifecycle-handoff-{}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn mapped_image_bundle_crosses_bounded_runtime_lifecycle_boundaries() {
    let directory = test_directory();
    let bundle = directory.join("bundle");
    let rootfs = bundle.join(DEFAULT_ROOTFS_DIRECTORY);
    fs::create_dir_all(&rootfs).expect("test bundle rootfs should be created");

    let image = ImageConfiguration {
        architecture: "amd64".to_owned(),
        os: "linux".to_owned(),
        diff_ids: Vec::new(),
        process: ImageProcessConfig {
            user: Some("1000:1000".to_owned()),
            env: vec!["MODE=development".to_owned()],
            entrypoint: vec!["/bin/echo".to_owned()],
            cmd: vec!["engine-runtime-lifecycle-handoff".to_owned()],
            working_dir: Some("/work".to_owned()),
        },
    };
    let config = oci_config_from_image_configuration(&image)
        .expect("supported image configuration should map to OCI");
    let initialized = initialize_linux_bundle(&bundle, &config)
        .expect("mapped OCI configuration should initialize the controlled bundle");

    let runtime_script = directory.join("fake-runtime");
    fs::write(&runtime_script, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n")
        .expect("fake runtime should be written");
    fs::set_permissions(&runtime_script, fs::Permissions::from_mode(0o755))
        .expect("fake runtime should be executable");

    let runtime = ProcessOciRuntime::new_for_execution(OciRuntimeKind::Crun, &runtime_script)
        .expect("bounded fake runtime should pass executable validation");
    let executor = RuntimeExecutor::default();
    let id = ContainerId::parse("lifecycle-handoff-test")
        .expect("test container identifier should be valid");

    let create = runtime
        .create(&executor, &id, &initialized.bundle_path)
        .expect("engine-mapped bundle should pass the runtime create boundary");
    assert!(create.stdout.text_lossy().starts_with("create\n"));
    assert!(create.stdout.text_lossy().contains("--bundle\n"));
    assert!(
        create
            .stdout
            .text_lossy()
            .contains(initialized.bundle_path.to_string_lossy().as_ref())
    );

    let start = runtime
        .start(&executor, &id)
        .expect("container identifier should pass the runtime start boundary");
    assert_eq!(start.stdout.text_lossy(), "start\nlifecycle-handoff-test\n");

    let state = runtime
        .state(&executor, &id)
        .expect("container identifier should pass the runtime state boundary");
    assert_eq!(state.stdout.text_lossy(), "state\nlifecycle-handoff-test\n");

    let delete = runtime
        .delete(&executor, &id)
        .expect("container identifier should pass the runtime delete boundary");
    assert_eq!(
        delete.stdout.text_lossy(),
        "delete\nlifecycle-handoff-test\n"
    );

    assert!(initialized.rootfs_path.is_dir());
    assert!(initialized.config_path.is_file());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}
