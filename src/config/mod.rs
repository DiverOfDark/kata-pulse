use std::path::{Path, PathBuf};

// HTTP endpoint paths
pub const METRICS_URL: &str = "/metrics";

// Get the storage path where sandboxes info are stored (Go runtime)
pub fn get_sandboxes_storage_path() -> PathBuf {
    PathBuf::from("/run/vc/sbs")
}

// Get the storage path where sandboxes info are stored (Rust runtime)
pub fn get_sandboxes_storage_path_rust() -> PathBuf {
    PathBuf::from("/run/kata")
}

// Get socket path for the given storage path
pub fn socket_path(id: &str, storage_path: &Path) -> PathBuf {
    storage_path.join(id).join("shim-monitor.sock")
}

// Get socket path for the Go runtime
pub fn socket_path_go(id: &str) -> PathBuf {
    socket_path(id, &get_sandboxes_storage_path())
}

// Get socket path for the Rust runtime
pub fn socket_path_rust(id: &str) -> PathBuf {
    socket_path(id, &get_sandboxes_storage_path_rust())
}

// Get the client socket address
// Tries both Go and Rust runtime socket paths
pub fn client_socket_address(id: &str) -> anyhow::Result<String> {
    let go_socket = socket_path_go(id);

    if go_socket.exists() {
        return Ok(format!("unix://{}", go_socket.display()));
    }

    let rust_socket = socket_path_rust(id);
    if rust_socket.exists() {
        return Ok(format!("unix://{}", rust_socket.display()));
    }

    Err(anyhow::anyhow!(
        "socket not found for sandbox {}: checked {} and {}",
        id,
        go_socket.display(),
        rust_socket.display()
    ))
}
