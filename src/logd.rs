use log::{LevelFilter, error, warn, info, debug, trace};
use libsystemd::{activation, daemon};
use libsystemd::activation::IsType;

use mio::net::{UnixDatagram, UnixStream, UnixListener};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};

use failure::Error;

use libc::{sockaddr, sockaddr_un, getsockname};

//use std::os::unix::net::UnixDatagram;
use std::time::Duration;
use std::io::Read;
use std::fs::File;
use std::ffi::CStr;
use std::path::PathBuf;
use std::iter::FromIterator;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::os::unix::io::RawFd;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::FromRawFd;
use std::os::unix::io::IntoRawFd;

mod errors;

use crate::errors::*;

macro_rules! ok_or_continue {
    ($e:expr) => {{
        if let Ok(x) = $e {
            x
        } else {
            continue;
        }
    }};
}

macro_rules! ok_or_error {
    ($e:expr) => {{
        match $e {
            Ok(x) => (),
            Err(e) => {
                error!("Error: {:?}", e);
            }
        }
    }};
}

fn sock_unix_path(fd: RawFd) -> Result<PathBuf, Error> {
    let mut addr: sockaddr_un = sockaddr_un {
        sun_family: 0,
        sun_path: [0i8; 108],
    };
    let mut len = std::mem::size_of::<sockaddr_un>() as u32;
    unsafe {
    if getsockname(fd, &mut addr as *mut _ as *mut sockaddr, &mut len as *mut u32) != 0 {
        return Err(LogError::FdError.into());
    }
    let cstr = CStr::from_ptr(addr.sun_path.as_ptr()).to_str().unwrap();
    let path = PathBuf::from(cstr);
    debug!("sock path: {:?}", path);

    Ok(path)
    }
}

/// /run/systemd/journal/socket
const NATLOG: Token = Token(0);
/// /dev/log (I think)
const DEVLOG: Token = Token(1);
/// /run/systemd/journal/syslog
const SYSLOG: Token = Token(2);
/// Audit, looks like netlink
const AUDLOG: Token = Token(3);
/// Kernel logs (/dev/kmsg)
const KERNLOG: Token = Token(4);
/// /run/systemd/journal/stdout
const STDLOG: Token = Token(5);

fn run() -> Result<(), Error> {
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(128);
    debug!("Starting up");
    let mut streams: HashMap<Token, RefCell<UnixStream>> = HashMap::new();
    let mut datagrams: HashMap<Token, RefCell<UnixDatagram>> = HashMap::new();

    let mut fds = activation::receive_descriptors(false)?;
    debug!("Received fds: {:?}", fds);
    if fds.len() != 4 {
        return Err(LogError::LoggingError.into());
    }

    let mut stdout_sock: UnixStream;

    while let Some(raw_fd) = fds.pop() {
        if raw_fd.is_unix() && raw_fd.is_stream() {
            let raw_fd = raw_fd.into_raw_fd();
            let path = sock_unix_path(raw_fd)?;
            info!("path: {:?}", path);
            if path.ends_with("stdout") {
                info!("Creating from {}", raw_fd);
                stdout_sock = unsafe { UnixStream::from_raw_fd(raw_fd) };
                poll.registry().register(&mut stdout_sock, STDLOG, Interest::READABLE)?;
                streams.insert(STDLOG, RefCell::new(stdout_sock));
            }
        } else if raw_fd.is_unix() && raw_fd.is_dgram() {
            let raw_fd = raw_fd.into_raw_fd();
            let path = sock_unix_path(raw_fd)?;
            info!("path: {:?}", path);
            if path.ends_with("dev-log") {
                info!("Creating from {}", raw_fd);
                let mut devlog_sock = unsafe { UnixDatagram::from_raw_fd(raw_fd) };
                poll.registry().register(&mut devlog_sock, DEVLOG, Interest::READABLE)?;
                datagrams.insert(DEVLOG, RefCell::new(devlog_sock));
            } else if path.ends_with("socket") {
                info!("Creating from {}", raw_fd);
                let mut native_sock = unsafe { UnixDatagram::from_raw_fd(raw_fd) };
                poll.registry().register(&mut native_sock, NATLOG, Interest::READABLE)?;
                datagrams.insert(NATLOG, RefCell::new(native_sock));
            }
        }
    }

    info!("Streams: {:?}", streams);
    info!("Datagrams: {:?}", datagrams);

    //let raw_fd = fds.remove(0).into_raw_fd();
    //unsafe { sock_unix_path(raw_fd); }
    //info!("Creating from {}", raw_fd);
    //let mut unk_sock = unsafe { UnixDatagram::from_raw_fd(raw_fd) };
    //poll.registry().register(&mut unk_sock, AUDLOG, Interest::READABLE)?;

    //let raw_fd = fds.remove(1).into_raw_fd();
    //info!("Creating from {}", raw_fd);
    //let mut devlog_sock = unsafe { UnixStream::from_raw_fd(raw_fd) };
    //poll.registry().register(&mut devlog_sock, DEVLOG, Interest::READABLE)?;

    //let raw_fd = fds.remove(2).into_raw_fd();
    //info!("Creating from {}", raw_fd);
    //let mut sdlog_sock = unsafe { UnixDatagram::from_raw_fd(raw_fd) };
    //poll.registry().register(&mut sdlog_sock, SDJLOG, Interest::READABLE)?;

    //let mut kmsg = File::open("/dev/kmsg")?;
    //info!("Registering /dev/kmsg");
    //poll.registry().register(&mut SourceFd(&kmsg.as_raw_fd()), KERNLOG, Interest::READABLE)?;

    info!("Sending initial notify");
    daemon::notify(false, &[daemon::NotifyState::Ready])?;

    loop {
        poll.poll(&mut events, Some(Duration::from_secs(30)))?;
        trace!("Notifying watchdog");
        if let Err(e) = daemon::notify(false, &[daemon::NotifyState::Watchdog]) {
            error!("Error notifying: {:?}", e);
        }

        for event in events.iter() {
            match event.token() {
                STDLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on stdout socket");
                    let sock = &mut streams[&STDLOG].borrow_mut();
                    // TODO: some issues with the stdout socket. Investigate
                    ok_or_error!(sock.read(&mut buf));
                    info!("Read event done on stdout socket");
                    let s = ok_or_continue!(std::str::from_utf8(&buf[..]));
                    info!("Recieved {}", s);
                }
                DEVLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on /dev/log");
                    let sock = &mut datagrams[&DEVLOG].borrow_mut();
                    ok_or_error!(sock.recv(&mut buf));
                    info!("Read event done on /dev/log");
                    let s = ok_or_continue!(std::str::from_utf8(&buf[..]));
                    info!("Recieved {}", s);
                }
                NATLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on systemd native socket");
                    let sock = &mut datagrams[&NATLOG].borrow_mut();
                    ok_or_error!(sock.recv(&mut buf));
                    info!("Read event done on systemd native socket");
                    let s = ok_or_continue!(std::str::from_utf8(&buf[..]));
                    info!("Recieved {}", s);
                }
                //AUDLOG => {
                //    let mut buf = [0u8; 1024];
                //    info!("Got read event on unk fd");
                //    ok_or_error!(unk_sock.recv(&mut buf));
                //    info!("Read event done on unk fd");
                //    let s = ok_or_continue!(std::str::from_utf8(&buf[..]));
                //    info!("Recieved {}", s);
                //}
                //KERNLOG => {
                //    let mut buf = [0u8; 1024];
                //    info!("Got read event on kernel log device");
                //    ok_or_error!(kmsg.read(&mut buf));
                //    info!("Read event done on kernel log device");
                //    let s = ok_or_continue!(std::str::from_utf8(&buf[..]));
                //    info!("Recieved {}", s);
                //}
                _ => {
                    error!("Unhandled event");
                    panic!();
                }
            }
        }
    }

    Ok(())
}

fn main() {
    simple_logging::log_to_file("/vagrant/logd.log", LevelFilter::Trace).unwrap();

    debug!("starting main loop");
    let res = run();

    warn!("Res: {:?}", res);
}
