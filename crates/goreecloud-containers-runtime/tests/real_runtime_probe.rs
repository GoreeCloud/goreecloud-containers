use goreecloud_containers_runtime::{OciRuntimeKind, ProcessOciRuntime};
use std::env;
use std::path::PathBuf;
use std::str::FromStr;

const KIND_ENV: &str = "GOREECLOUD_CONTAINERS_REAL_RUNTIME_KIND";
const PATH_ENV: &str = "GOREECLOUD_CONTAINERS_REAL_RUNTIME_PATH";

#[test]
fn opt_in_real_runtime_probe_requires_complete_configuration() {
    let kind = env::var_os(KIND_ENV);
    let path = env::var_os(PATH_ENV);

    match (kind, path) {
        (None, None) => return,
        (Some(_), None) => panic!("{PATH_ENV} is required when {KIND_ENV} is set"),
        (None, Some(_)) => panic!("{KIND_ENV} is required when {PATH_ENV} is set"),
        (Some(kind), Some(path)) => {
            let kind = kind
                .into_string()
                .expect("real runtime kind must be valid UTF-8");
            let kind = OciRuntimeKind::from_str(&kind)
                .expect("real runtime kind must be exactly crun or runc");
            let path = PathBuf::from(path);
            assert!(path.is_absolute(), "real runtime path must be absolute");

            let runtime = ProcessOciRuntime::new_for_execution(kind, &path)
                .expect("configured real runtime executable must pass execution validation");
            let probe = runtime
                .probe()
                .expect("configured real runtime must answer a bounded --version probe");

            assert_eq!(probe.kind, kind);
            assert!(probe.executable.is_absolute());
            assert!(!probe.version_output.trim().is_empty());
        }
    }
}
