mod eis;
mod keymap;

use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::process;

use clap::Parser;

/// Type text into the focused window via KWin EIS + libei
#[derive(Parser)]
#[command(name = "ei-type")]
struct Args {
    /// Inter-key delay in milliseconds
    #[arg(short = 'd', long = "delay", default_value = "5")]
    delay_ms: u64,

    /// Send a key combo (e.g. ctrl+v, enter)
    #[arg(long = "key")]
    key: Option<String>,

    /// Verbose debug output
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

/// Call KWin's connectToEIS D-Bus method, returning the EIS Unix socket.
/// Returns both the stream AND the D-Bus connection (must stay alive for EIS to work).
async fn connect_kwin_eis(verbose: bool) -> Result<(UnixStream, zbus::Connection), Box<dyn std::error::Error>> {
    let connection = zbus::Connection::session().await?;

    let proxy = zbus::Proxy::new(
        &connection,
        "org.kde.KWin",
        "/org/kde/KWin/EIS/RemoteDesktop",
        "org.kde.KWin.EIS.RemoteDesktop",
    )
    .await?;

    // CAP_ALL = 63 — KWin requires all capabilities to be requested
    let reply: zbus::Message = proxy.call_method("connectToEIS", &(63i32,)).await?;
    let body = reply.body();
    let (fd, cookie): (zbus::zvariant::OwnedFd, i32) = body.deserialize()?;

    if verbose {
        eprintln!("ei-type: got EIS fd, cookie={}", cookie);
    }

    let owned_fd: std::os::fd::OwnedFd = fd.into();
    let stream = UnixStream::from(owned_fd);
    stream.set_nonblocking(true)?;
    Ok((stream, connection))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let delay_us = args.delay_ms * 1000;

    // Get EIS socket from KWin via D-Bus
    // Keep the D-Bus connection alive — KWin invalidates EIS when D-Bus disconnects
    let (stream, _dbus_conn) = match connect_kwin_eis(args.verbose).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ei-type: D-Bus connectToEIS failed: {}", e);
            process::exit(1);
        }
    };

    // Connect to EIS and negotiate keyboard device
    let mut eis = match eis::EisConnection::connect(stream, "ei-type", args.verbose) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ei-type: failed to get keyboard device: {}", e);
            process::exit(1);
        }
    };

    // Key combo mode
    if let Some(combo) = &args.key {
        if let Err(e) = eis.send_key_combo(combo, delay_us) {
            eprintln!("ei-type: key combo failed: {}", e);
            process::exit(1);
        }
        return;
    }

    // Read stdin and type each character
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        eprintln!("ei-type: failed to read stdin: {}", e);
        process::exit(1);
    }

    if let Err(e) = eis.type_text(&input, delay_us) {
        eprintln!("ei-type: typing failed: {}", e);
        process::exit(1);
    }
}
