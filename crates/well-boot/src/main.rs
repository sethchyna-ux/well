//! Builds and boots the real AgentOS payload. No simulation backend.
#[allow(dead_code)]
pub mod netboot;
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256, Sha384};
use std::{
    error::Error,
    fs,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
#[derive(Parser)]
#[command(
    version,
    about = "Well AgentOS build, firmware boot, export and netboot"
)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    Build {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        ipxe_bootstrap: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    Run {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        firmware: Option<PathBuf>,
        #[arg(long)]
        network: bool,
        #[arg(long)]
        network_url: Option<String>,
    },
    Serve {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        bind_ip: Ipv4Addr,
        #[arg(long, default_value = "proxy")]
        mode: String,
        #[arg(long)]
        range: Option<String>,
        #[arg(long, default_value = "255.255.255.0")]
        mask: Ipv4Addr,
        #[arg(long)]
        router: Option<Ipv4Addr>,
        #[arg(long)]
        dns: Option<Ipv4Addr>,
        #[arg(long, default_value_t = 67)]
        dhcp_port: u16,
        #[arg(long, default_value_t = 4011)]
        proxy_port: u16,
        #[arg(long, default_value_t = 69)]
        tftp_port: u16,
        #[arg(long, default_value_t = 8080)]
        http_port: u16,
    },
}
fn checked(cmd: &mut Command) -> Result<()> {
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("Command failed ({status}): {cmd:?}").into());
    }
    Ok(())
}
fn digest(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
fn verify(output: &Path) -> Result<()> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
    for name in ["BOOTX64.EFI", "agentos-esp.img", "agent_os_unikernel.elf"] {
        if manifest["sha256"][name].as_str() != Some(digest(&output.join(name))?.as_str()) {
            return Err(format!("Artifact checksum mismatch: {name}").into());
        }
    }
    if let Some(hash) = manifest["sha256"]["netboot-bootstrap.img"].as_str() {
        if digest(&output.join("netboot-bootstrap.img"))? != hash {
            return Err("Network bootstrap image checksum mismatch".into());
        }
    }
    for alias in ["tftpboot/ipxe.efi", "tftpboot/BOOTX64.EFI"] {
        if digest(&output.join(alias))? != digest(&output.join("BOOTX64.EFI"))? {
            return Err(format!("TFTP artifact checksum mismatch: {alias}").into());
        }
    }
    Ok(())
}
fn revision(source: &Path) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !out.status.success() {
        return Err("AgentOS source must be a Git checkout".into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().into())
}
fn build(source: PathBuf, output: PathBuf, ipxe_bootstrap: Option<PathBuf>) -> Result<()> {
    let source = source.canonicalize()?;
    fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let bundled = std::env::current_exe()?
        .parent()
        .ok_or("Missing binary directory")?
        .join("../Resources/agentos-loader");
    let linux_bundle = std::env::current_exe()?
        .parent()
        .ok_or("Missing binary directory")?
        .join("../share/well/agentos-loader");
    let loader = if linux_bundle.is_dir() {
        linux_bundle.canonicalize()?
    } else if bundled.is_dir() {
        bundled.canonicalize()?
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../boot/agentos")
            .canonicalize()?
    };

    let zig = Command::new("zig").arg("version").output()?;
    let zig_version = String::from_utf8(zig.stdout)?.trim().to_string();
    if zig_version != "0.17.1" {
        return Err(format!("Boot build requires Zig 0.17.1; found {zig_version}").into());
    }
    let kernel_prefix = output.join("kernel-build");
    checked(
        Command::new("zig")
            .current_dir(source.join("microvm"))
            .args(["build", "kernel", "-Doptimize=ReleaseSafe", "--prefix"])
            .arg(&kernel_prefix),
    )?;
    let kernel = kernel_prefix.join("bin/trismegistus-unikernel.elf");
    let loader_cache = output.join("loader-cache");
    let loader_build = output.join("loader-build");
    let _ = fs::remove_dir_all(&loader_cache);
    let _ = fs::remove_dir_all(&loader_build);
    checked(
        Command::new("zig")
            .current_dir(&loader)
            .arg("build")
            .arg(format!("-Dkernel={}", kernel.display()))
            .arg(format!(
                "-Dprotocol={}",
                source.join("microvm/boot/protocol.zig").display()
            ))
            .arg("--cache-dir")
            .arg(&loader_cache)
            .arg("--prefix")
            .arg(&loader_build),
    )?;
    fs::copy(
        output.join("loader-build/bin/BOOTX64.efi"),
        output.join("BOOTX64.EFI"),
    )?;
    fs::copy(kernel, output.join("agent_os_unikernel.elf"))?;
    let efi = fs::read(output.join("BOOTX64.EFI"))?;
    fs::write(output.join("agentos-esp.img"), fat_image(&efi)?)?;
    fs::create_dir_all(output.join("tftpboot"))?;
    // The pinned PXE engine uses this conventional x86_64 boot filename.
    fs::copy(output.join("BOOTX64.EFI"), output.join("tftpboot/ipxe.efi"))?;
    fs::copy(
        output.join("BOOTX64.EFI"),
        output.join("tftpboot/BOOTX64.EFI"),
    )?;
    if let Some(path) =
        ipxe_bootstrap.or_else(|| std::env::var_os("WELL_IPXE_BOOTSTRAP").map(PathBuf::from))
    {
        let bootstrap = fs::read(path)?;
        fs::write(output.join("netboot-bootstrap.img"), fat_image(&bootstrap)?)?;
    }
    let mut hashes = serde_json::Map::new();
    for name in ["BOOTX64.EFI", "agentos-esp.img", "agent_os_unikernel.elf"] {
        hashes.insert(name.into(), digest(&output.join(name))?.into());
    }
    let dirty = Command::new("git")
        .arg("-C")
        .arg(&source)
        .args(["status", "--porcelain"])
        .output()?;
    if output.join("netboot-bootstrap.img").is_file() {
        hashes.insert(
            "netboot-bootstrap.img".into(),
            digest(&output.join("netboot-bootstrap.img"))?.into(),
        );
    }
    let manifest = serde_json::json!({"version":1,"architecture":"x86_64","agentos_revision":revision(&source)?,"agentos_dirty":!dirty.stdout.is_empty(),"zig":zig_version,"loader_sha256":digest(&loader.join("src/main.zig"))?,"protocol_sha256":digest(&source.join("microvm/boot/protocol.zig"))?,"kernel_sha384":format!("{:x}",Sha384::digest(fs::read(output.join("agent_os_unikernel.elf"))?)),"sha256":hashes});
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let checksums = hashes
        .iter()
        .map(|(k, v)| format!("{}  {k}\n", v.as_str().unwrap()))
        .collect::<String>();
    fs::write(output.join("SHA256SUMS"), checksums)?;
    verify(&output)?;
    println!(
        "Verified AgentOS artifacts exported to {}",
        output.display()
    );
    Ok(())
}
/// Deterministic FAT16 removable ESP volume. 64 MiB, 2 KiB clusters, two FATs.
fn fat_image(efi: &[u8]) -> Result<Vec<u8>> {
    if efi.len() < 64 || !efi.starts_with(b"MZ") {
        return Err("Not a PE/COFF EFI application".into());
    }
    const DATA: usize = (1 + 2 * 128 + 32) * 512;
    const CLUSTER: usize = 2048;
    let count = efi.len().div_ceil(CLUSTER);
    if count > 32000 {
        return Err("EFI payload exceeds ESP capacity".into());
    }
    let mut image = vec![0u8; 64 * 1024 * 1024];
    image[..3].copy_from_slice(&[0xeb, 0x3c, 0x90]);
    image[3..11].copy_from_slice(b"WELLBOOT");
    image[11..13].copy_from_slice(&512u16.to_le_bytes());
    image[13] = 4;
    image[14..16].copy_from_slice(&1u16.to_le_bytes());
    image[16] = 2;
    image[17..19].copy_from_slice(&512u16.to_le_bytes());
    image[21] = 0xf8;
    image[22..24].copy_from_slice(&128u16.to_le_bytes());
    image[24..26].copy_from_slice(&32u16.to_le_bytes());
    image[26..28].copy_from_slice(&64u16.to_le_bytes());
    image[32..36].copy_from_slice(&131072u32.to_le_bytes());
    image[36] = 0x80;
    image[38] = 0x29;
    image[43..54].copy_from_slice(b"WELLAGENTOS");
    image[54..62].copy_from_slice(b"FAT16   ");
    image[510..512].copy_from_slice(&[0x55, 0xaa]);
    for fat in [512, (1 + 128) * 512] {
        for (index, value) in [(0, 0xfff8u16), (1, 0xffff), (2, 0xffff), (3, 0xffff)] {
            image[fat + index * 2..fat + index * 2 + 2].copy_from_slice(&value.to_le_bytes());
        }
        for n in 0..count {
            let value = if n + 1 == count {
                0xffff
            } else {
                (n + 5) as u16
            };
            let offset = fat + (n + 4) * 2;
            image[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    fn entry(image: &mut [u8], off: usize, name: &[u8; 11], attr: u8, cluster: u16, size: u32) {
        image[off..off + 11].copy_from_slice(name);
        image[off + 11] = attr;
        image[off + 26..off + 28].copy_from_slice(&cluster.to_le_bytes());
        image[off + 28..off + 32].copy_from_slice(&size.to_le_bytes());
    }
    entry(&mut image, (1 + 256) * 512, b"EFI        ", 0x10, 2, 0);
    entry(&mut image, DATA, b".          ", 0x10, 2, 0);
    entry(&mut image, DATA + 32, b"..         ", 0x10, 0, 0);
    entry(&mut image, DATA + 64, b"BOOT       ", 0x10, 3, 0);
    entry(&mut image, DATA + CLUSTER, b".          ", 0x10, 3, 0);
    entry(&mut image, DATA + CLUSTER + 32, b"..         ", 0x10, 2, 0);
    entry(
        &mut image,
        DATA + CLUSTER + 64,
        b"BOOTX64 EFI",
        0x20,
        4,
        efi.len() as u32,
    );
    image[DATA + 2 * CLUSTER..DATA + 2 * CLUSTER + efi.len()].copy_from_slice(efi);
    Ok(image)
}
fn firmware_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
    explicit
        .or_else(|| std::env::var_os("WELL_UEFI_FIRMWARE").map(PathBuf::from))
        .or_else(|| {
            [
                "/opt/homebrew/share/qemu/edk2-x86_64-code.fd",
                "/usr/local/share/qemu/edk2-x86_64-code.fd",
                "/usr/share/OVMF/OVMF_CODE.fd",
            ]
            .into_iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
        })
        .ok_or_else(|| "Set --firmware or WELL_UEFI_FIRMWARE to x86_64 EDK2 firmware".into())
}
fn run(
    output: PathBuf,
    firmware: Option<PathBuf>,
    network: bool,
    network_url: Option<String>,
) -> Result<()> {
    verify(&output)?;
    let output = output.canonicalize()?;
    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args(["-machine", "q35", "-m", "512M", "-drive"])
        .arg(format!(
            "if=pflash,format=raw,readonly=on,file={}",
            firmware_path(firmware)?.display()
        ));
    if network && output.join("netboot-bootstrap.img").is_file() {
        cmd.arg("-drive").arg(format!(
            "file={},format=raw,snapshot=on",
            output.join("netboot-bootstrap.img").display()
        ));
    }
    if network {
        let file = network_url.as_deref().unwrap_or("BOOTX64.EFI");
        if file.bytes().any(|b| b == 44 || b == 10 || b == 13) {
            return Err("Invalid QEMU network boot URL".into());
        }
        let restrict = if network_url.is_some() { "off" } else { "on" };
        cmd.arg("-netdev")
            .arg(format!(
                "user,id=boot,restrict={restrict},tftp={},bootfile={file}",
                output.join("tftpboot").display()
            ))
            .args([
                "-device",
                if output.join("netboot-bootstrap.img").is_file() {
                    "virtio-net-pci,netdev=boot,romfile=,bootindex=2"
                } else {
                    "e1000,netdev=boot,romfile=efi-e1000.rom,bootindex=1"
                },
                "-boot",
                if output.join("netboot-bootstrap.img").is_file() {
                    "cn"
                } else {
                    "n"
                },
            ]);
    } else {
        cmd.arg("-drive").arg(format!(
            "file={},format=raw,snapshot=on",
            output.join("agentos-esp.img").display()
        ));
    }
    checked(
        cmd.args([
            "-serial",
            "stdio",
            "-monitor",
            "none",
            "-display",
            "none",
            "-no-reboot",
        ])
        .stdin(Stdio::inherit()),
    )
}
#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Action::Build {
            source,
            output,
            ipxe_bootstrap,
        } => build(source, output, ipxe_bootstrap),
        Action::Run {
            output,
            firmware,
            network,
            network_url,
        } => run(output, firmware, network, network_url),
        Action::Serve {
            output,
            bind_ip,
            mode,
            range,
            mask,
            router,
            dns,
            dhcp_port,
            proxy_port,
            tftp_port,
            http_port,
        } => {
            verify(&output)?;
            if !bind_ip.is_loopback()
                && !pxe::config::list_network_interfaces()
                    .iter()
                    .any(|(_, ip)| *ip == bind_ip)
            {
                return Err("--bind-ip must identify an active local interface".into());
            }
            let mode = match mode.as_str() {
                "proxy" => pxe::config::ServerMode::Proxy,
                "standalone" => pxe::config::ServerMode::Standalone,
                _ => return Err("Mode must be proxy or standalone".into()),
            };
            let pool = if let Some(range) = range {
                let (a, b) = range.split_once(',').ok_or("Range must be START,END")?;
                let a: Ipv4Addr = a.parse()?;
                let b: Ipv4Addr = b.parse()?;
                if u32::from(a) >= u32::from(b)
                    || (u32::from(a) & u32::from(mask)) != (u32::from(bind_ip) & u32::from(mask))
                    || (u32::from(b) & u32::from(mask)) != (u32::from(bind_ip) & u32::from(mask))
                {
                    return Err("Invalid lease range for selected subnet".into());
                }
                Some((a, b))
            } else {
                None
            };
            if mode == pxe::config::ServerMode::Standalone && pool.is_none() {
                return Err(
                    "Standalone DHCP requires --range START,END on an isolated interface".into(),
                );
            }
            let state = Arc::new(pxe::state::SharedState::new(1000));
            let dhcp = Arc::new(pxe::dhcp::DhcpServer::new(
                state.clone(),
                pxe::dhcp::DhcpConfig {
                    server_ip: bind_ip,
                    mode,
                    dhcp_port,
                    proxy_port,
                    http_port,
                    subnet_mask: mask,
                    router,
                    dns,
                    pool_range: pool,
                },
            ));
            let tftp = Arc::new(pxe::tftp::TftpServer::new(
                state.clone(),
                output.join("tftpboot"),
                tftp_port,
            ));
            let listener = tokio::net::TcpListener::bind((bind_ip, http_port)).await?;
            let status_state = state.clone();
            let metrics_state = state.clone();
            let app = axum::Router::new()
                .route(
                    "/status",
                    axum::routing::get(move || {
                        let state = status_state.clone();
                        async move {
                            axum::Json(serde_json::json!({
                                "metrics": state.metrics_snapshot(),
                                "clients": state.client_snapshots(),
                                "events": state.get_events()
                            }))
                        }
                    }),
                )
                .route_service(
                    "/BOOTX64.EFI",
                    tower_http::services::ServeFile::new(output.join("BOOTX64.EFI")),
                )
                .route_service(
                    "/agentos-esp.img",
                    tower_http::services::ServeFile::new(output.join("agentos-esp.img")),
                )
                .route_service(
                    "/manifest.json",
                    tower_http::services::ServeFile::new(output.join("manifest.json")),
                )
                .route_service(
                    "/SHA256SUMS",
                    tower_http::services::ServeFile::new(output.join("SHA256SUMS")),
                )
                .layer(axum::middleware::from_fn(
                    move |request: axum::extract::Request, next: axum::middleware::Next| {
                        let state = metrics_state.clone();
                        async move {
                            state.record_http_request();
                            next.run(request).await
                        }
                    },
                ));
            let http = async {
                axum::serve(listener, app)
                    .await
                    .map_err(|e| -> Box<dyn Error + Send + Sync> { Box::new(e) })
            };
            println!("HTTP bound to {bind_ip}:{http_port}; starting DHCP and TFTP. Ctrl+C stops the companion.");
            tokio::select! {
                result=async {tokio::try_join!(async{dhcp.run().await.map_err(|e| -> Box<dyn Error+Send+Sync>{Box::new(e)})},async{tftp.run().await.map_err(|e| -> Box<dyn Error+Send+Sync>{Box::new(e)})},http)} => {result?;}
                result=tokio::signal::ctrl_c()=>{result?;}
            }
            Ok(())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fat_capacity_and_signature() {
        assert!(fat_image(b"fake").is_err());
        let mut efi = vec![0; 4097];
        efi[..2].copy_from_slice(b"MZ");
        let image = fat_image(&efi).unwrap();
        assert_eq!(&image[510..512], &[0x55, 0xaa]);
        let data = (1 + 256 + 32) * 512 + 4096;
        assert_eq!(&image[data..data + efi.len()], efi);
        assert_eq!(
            &image[512 + 6 * 2..512 + 6 * 2 + 2],
            &0xffffu16.to_le_bytes()
        );
    }
}
