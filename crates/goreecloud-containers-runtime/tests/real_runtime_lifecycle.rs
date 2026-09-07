use goreecloud_containers_core::ContainerId;
use goreecloud_containers_runtime::{OciRuntimeKind, ProcessOciRuntime, RuntimeExecutor};
use std::env;
use std::path::PathBuf;
use std::str::FromStr;

const KIND_ENV: &str = "GOREECLOUD_CONTAINERS_REAL_RUNTIME_KIND";
const PATH_ENV: &str = "GOREECLOUD_CONTAINERS_REAL_RUNTIME_PATH";
const BUNDLE_ENV: &str = "GOREECLOUD_CONTAINERS_REAL_RUNTIME_BUNDLE";
const ID_ENV: &str = "GOREECLOUD_CONTAINERS_REAL_RUNTIME_CONTAINER_ID";

#[test]
fn opt_in_real_runtime_executes_disposable_lifecycle_when_fully_configured() {
    let kind = env::var_os(KIND_ENV);
    let runtime_path = env::var_os(PATH_ENV);
    let bundle = env::var_os(BUNDLE_ENV);
    let container_id = env::var_os(ID_ENV);

    if kind.is_none() && runtime_path.is_none() && bundle.is_none() && container_id.is_none() {
        // Ordinary CI deliberately does not manufacture real-runtime acceptance.
        return;
    }
    let kind = kind
        .unwrap_or_else(|| panic!("{KIND_ENV} is required for the opt-in real lifecycle"))
        .into_string()
        .expect("real runtime kind must be valid UTF-8");
    let runtime_path = PathBuf::from(
        runtime_path.unwrap_or_else(|| panic!("{PATH_ENV} is required for the opt-in real lifecycle")),
    );
    let bundle = PathBuf::from(
        bundle.unwrap_or_else(|| panic!("{BUNDLE_ENV} is required for the opt-in real lifecycle")),
    );
    let container_id = container_id
        .unwrap_or_else(|| panic!("{ID_ENV} is required for the opt-in real lifecycle"))
        .into_string()
        .expect("real runtime container id must be valid UTF-8");

    assert!(bundle.is_absolute(), "real runtime bundle path must be absolute");
    let kind = OciRuntimeKind::from_str(&kind).expect("real runtime kind must be exactly crun or runc");
    let id = ContainerId::parse(&container_id).expect("real runtime container id must be valid");
    let runtime = ProcessOciRuntime::new_for_execution(kind, &runtime_path)
        .expect("configured real runtime executable must pass execution validation");
    let probe = runtime
        .probe()
        .expect("configured real runtime must answer the bounded version probe");
    assert!(
        probe.version_output.to_ascii_lowercase().contains(kind.as_str()),
        "real runtime version output must identify the configured runtime kind"
    );

    let executor = RuntimeExecutor::default();
    runtime
        .create(&executor, &id, &bundle)
        .expect("real runtime create must succeed for the supplied disposable bundle");

    let lifecycle_result = (|| {
        runtime
            .start(&executor, &id)
            .expect("real runtime start must succeed for the supplied disposable bundle");
        let state = runtime
            .state(&executor, &id)
            .expect("real runtime state must succeed after start");
        assert!(
            !state.stdout.text_lossy().trim().is_empty(),
            "real runtime state must return bounded observable state"
        );
        runtime
            .delete(&executor, &id)
            .expect("real runtime delete must succeed for the supplied disposable container");
    })();

    lifecycle_result
}
