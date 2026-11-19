//! Shared, private per-user local RPC location. Resolving never creates user state.
use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| {
            #[cfg(unix)]
            {
                std::env::temp_dir().join(format!("well-{}", unsafe { libc::geteuid() }))
            }
            #[cfg(not(unix))]
            {
                std::env::temp_dir().join("well")
            }
        });
    root.join("well/run/caduceus.sock")
}
