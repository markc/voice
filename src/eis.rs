use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::Duration;

use reis::ei::{self, keyboard::KeyState};
use reis::PendingRequestResult;

use crate::keymap;

/// Poll the context fd for readability with a 500ms timeout.
fn poll_readable(context: &ei::Context) -> std::io::Result<bool> {
    let fd = context.as_raw_fd();
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let ret = unsafe { libc::poll(&mut pfd, 1, 500) };
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(ret > 0)
    }
}

pub struct EisConnection {
    context: ei::Context,
    keyboard: ei::Keyboard,
    device: ei::Device,
    last_serial: u32,
    verbose: bool,
}

impl EisConnection {
    /// Connect to EIS via an already-obtained Unix socket fd.
    /// Performs the libei handshake, binds all capabilities (KWin requires this),
    /// and waits for a keyboard device to become available.
    pub fn connect(
        stream: UnixStream,
        name: &str,
        verbose: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let context = ei::Context::new(stream)?;

        if verbose {
            eprintln!("ei-type: context created, starting handshake");
        }

        // Use the blocking handshake helper — handles version/name/context_type/interfaces
        let resp = reis::handshake::ei_handshake_blocking(
            &context,
            name,
            ei::handshake::ContextType::Sender,
        )?;
        let mut last_serial = resp.serial;

        if verbose {
            eprintln!("ei-type: handshake complete, serial={}", last_serial);
            eprintln!("ei-type: negotiated interfaces: {:?}", resp.negotiated_interfaces);
        }

        // Send a sync request and handle ping (like libei does after handshake)
        let _callback = resp.connection.sync(1);

        // Drain any events buffered during handshake read (including ping)
        while let Some(result) = context.pending_event() {
            if let PendingRequestResult::Request(ei::Event::Connection(
                _,
                ei::connection::Event::Ping { ping },
            )) = result
            {
                if verbose {
                    eprintln!("ei-type: responding to post-handshake ping");
                }
                ping.done(0);
            }
        }

        // Flush sync + ping response together (matching C version behavior)
        context.flush()?;

        if verbose {
            eprintln!("ei-type: sent sync + ping response");
        }

        // Now process seat/device events using low-level API
        let mut seat_caps: HashMap<String, u64> = HashMap::new();
        let mut keyboard: Option<ei::Keyboard> = None;
        let mut kbd_device: Option<ei::Device> = None;
        let mut device_interfaces: HashMap<String, reis::Object> = HashMap::new();
        let mut ready = false;
        let mut timeout_count = 0;
        let max_timeouts = 10;

        while !ready && timeout_count < max_timeouts {
            // First drain any already-buffered events (handshake may have read extra data)
            let mut had_events = false;
            while let Some(result) = context.pending_event() {
                had_events = true;
                let request = match result {
                    PendingRequestResult::Request(r) => r,
                    PendingRequestResult::ParseError(e) => {
                        return Err(format!("parse error: {:?}", e).into());
                    }
                    PendingRequestResult::InvalidObject(id) => {
                        if verbose {
                            eprintln!("ei-type: invalid object {}", id);
                        }
                        continue;
                    }
                };

                match request {
                    ei::Event::Connection(_connection, req) => match req {
                        ei::connection::Event::Seat { seat: _ } => {
                            if verbose {
                                eprintln!("ei-type: seat announced");
                            }
                        }
                        ei::connection::Event::Ping { ping } => {
                            if verbose {
                                eprintln!("ei-type: responding to ping");
                            }
                            ping.done(0);
                            context.flush()?;
                        }
                        ei::connection::Event::Disconnected { last_serial, reason, explanation } => {
                            eprintln!("ei-type: DISCONNECTED! serial={}, reason={:?}, explanation={:?}",
                                last_serial, reason, explanation);
                            return Err("server disconnected".into());
                        }
                        _ => {}
                    },
                    ei::Event::Seat(seat, req) => match req {
                        ei::seat::Event::Capability { mask, interface } => {
                            if verbose {
                                eprintln!("ei-type: seat capability: {} mask={}", interface, mask);
                            }
                            seat_caps.insert(interface, mask);
                        }
                        ei::seat::Event::Done => {
                            // Bind ALL capabilities — KWin requires this
                            let combined: u64 = seat_caps.values().copied().sum();
                            if verbose {
                                eprintln!("ei-type: binding all capabilities, mask={}", combined);
                            }
                            seat.bind(combined);
                            context.flush()?;
                        }
                        ei::seat::Event::Device { device: _ } => {
                            device_interfaces.clear();
                            if verbose {
                                eprintln!("ei-type: device announced");
                            }
                        }
                        _ => {}
                    },
                    ei::Event::Device(device, req) => match req {
                        ei::device::Event::Interface { object } => {
                            if verbose {
                                eprintln!("ei-type: device interface: {}", object.interface());
                            }
                            device_interfaces
                                .insert(object.interface().to_owned(), object);
                        }
                        ei::device::Event::Done => {
                            if let Some(obj) = device_interfaces.get("ei_keyboard") {
                                if let Some(kb) = obj.clone().downcast::<ei::Keyboard>() {
                                    if verbose {
                                        eprintln!("ei-type: keyboard device found");
                                    }
                                    keyboard = Some(kb);
                                    kbd_device = Some(device.clone());
                                }
                            }
                        }
                        ei::device::Event::Resumed { serial } => {
                            last_serial = serial;
                            if keyboard.is_some() {
                                if verbose {
                                    eprintln!("ei-type: device resumed, serial={}", serial);
                                }
                                ready = true;
                            }
                        }
                        _ => {}
                    },
                    ei::Event::Keyboard(_kb, ref _evt) => {
                        if verbose {
                            eprintln!("ei-type: keyboard event (keymap etc.)");
                        }
                    }
                    _ => {}
                }
            }

            let _ = context.flush();

            // If we already processed events, loop back to check for more
            // Even if ready, keep draining to process keymaps etc.
            if had_events && !ready {
                continue;
            }
            if had_events && ready {
                // One more drain to catch any trailing events
                continue;
            }

            // No pending events — poll for new data
            match poll_readable(&context) {
                Ok(true) => {
                    context.read()?;
                }
                Ok(false) => {
                    timeout_count += 1;
                    if verbose {
                        eprintln!("ei-type: poll timeout {}/{}", timeout_count, max_timeouts);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }

        if timeout_count >= max_timeouts {
            return Err("timeout waiting for EIS events (no response in 5s)".into());
        }

        let keyboard = keyboard.ok_or("no keyboard device found")?;
        let device = kbd_device.ok_or("no device found")?;

        device.start_emulating(last_serial, 0);
        context.flush()?;

        // Drain any remaining events (keymap fds, other device resumed, etc.)
        // Non-blocking: just process what's already buffered
        while let Some(result) = context.pending_event() {
            if let PendingRequestResult::Request(ei::Event::Connection(
                _,
                ei::connection::Event::Ping { ping },
            )) = result
            {
                ping.done(0);
            }
        }
        // Try one more read in case there's data on the socket
        let _ = context.read();
        while let Some(result) = context.pending_event() {
            if let PendingRequestResult::Request(ei::Event::Connection(
                _,
                ei::connection::Event::Ping { ping },
            )) = result
            {
                ping.done(0);
            }
        }
        let _ = context.flush();

        if verbose {
            eprintln!("ei-type: ready to type");
        }

        Ok(Self {
            context,
            keyboard,
            device,
            last_serial,
            verbose,
        })
    }

    /// Process any pending incoming events (e.g. pings from the server).
    fn dispatch(&self) {
        let _ = self.context.read();
        while let Some(result) = self.context.pending_event() {
            if let PendingRequestResult::Request(ei::Event::Connection(
                _,
                ei::connection::Event::Ping { ping },
            )) = result
            {
                ping.done(0);
            }
        }
    }

    /// Type a string character by character with inter-key delay.
    pub fn type_text(
        &mut self,
        text: &str,
        delay_us: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for c in text.chars() {
            if let Some(ki) = keymap::char_to_key(c) {
                self.type_key(ki.code, ki.shift, delay_us)?;
            } else if self.verbose {
                eprintln!("ei-type: skipping unmapped char '{}'", c.escape_debug());
            }
        }
        Ok(())
    }

    /// Send a key combo like "ctrl+v" or "enter".
    pub fn send_key_combo(
        &mut self,
        combo: &str,
        delay_us: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (modifiers, keycode) = keymap::parse_combo(combo)?;

        // Press modifiers
        for &m in &modifiers {
            self.keyboard.key(m, KeyState::Press);
            self.device.frame(self.last_serial, 0);
        }

        // Press and release key
        self.keyboard.key(keycode, KeyState::Press);
        self.device.frame(self.last_serial, 0);
        self.context.flush()?;
        thread::sleep(Duration::from_micros(delay_us));

        self.keyboard.key(keycode, KeyState::Released);
        self.device.frame(self.last_serial, 0);

        // Release modifiers in reverse
        for &m in modifiers.iter().rev() {
            self.keyboard.key(m, KeyState::Released);
            self.device.frame(self.last_serial, 0);
        }

        self.context.flush()?;
        self.dispatch();
        Ok(())
    }

    fn type_key(
        &mut self,
        code: u32,
        shift: bool,
        delay_us: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if shift {
            self.keyboard.key(keymap::KEY_LEFTSHIFT, KeyState::Press);
            self.device.frame(self.last_serial, 0);
        }

        self.keyboard.key(code, KeyState::Press);
        self.device.frame(self.last_serial, 0);
        self.context.flush()?;
        thread::sleep(Duration::from_micros(delay_us));

        self.keyboard.key(code, KeyState::Released);
        self.device.frame(self.last_serial, 0);

        if shift {
            self.keyboard.key(keymap::KEY_LEFTSHIFT, KeyState::Released);
            self.device.frame(self.last_serial, 0);
        }

        self.context.flush()?;
        self.dispatch();
        thread::sleep(Duration::from_micros(delay_us));
        Ok(())
    }
}
