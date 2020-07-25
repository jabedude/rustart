use log::{LevelFilter, error, warn, info, debug, trace};
use libsystemd::{activation, daemon};

use mio::net::{UnixDatagram, UnixStream};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};

use failure::Error;

//use std::os::unix::net::UnixDatagram;
use std::time::Duration;
use std::io::Read;
use std::fs::File;
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

/// /run/systemd/journal/socket
const SDJLOG: Token = Token(0);
/// /dev/log (I think)
const DEVLOG: Token = Token(1);
/// /run/systemd/journal/syslog
const SYSLOG: Token = Token(2);
/// Audit, looks like netlink
const AUDLOG: Token = Token(3);
/// Kernel logs (/dev/kmsg)
const KERNLOG: Token = Token(4);

fn run() -> Result<(), Error> {
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(128);
    debug!("Starting up");

    let mut fds = activation::receive_descriptors(false)?;
    debug!("Received fds: {:?}", fds);

    //let raw_fd = fds.remove(0).into_raw_fd();
    //info!("Creating from {}", raw_fd);
    //let mut unk_sock = unsafe { UnixDatagram::from_raw_fd(raw_fd) };
    //poll.registry().register(&mut unk_sock, AUDLOG, Interest::READABLE)?;

    let raw_fd = fds.remove(1).into_raw_fd();
    info!("Creating from {}", raw_fd);
    let mut devlog_sock = unsafe { UnixStream::from_raw_fd(raw_fd) };
    poll.registry().register(&mut devlog_sock, DEVLOG, Interest::READABLE)?;

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
                //SDJLOG => {
                //    let mut buf = [0u8; 1024];
                //    info!("Got read event on /run/systemd/journal/socket");
                //    ok_or_error!(sdlog_sock.recv(&mut buf));
                //    info!("Read event done on /run/systemd/journal/socket");
                //    let s = ok_or_continue!(std::str::from_utf8(&buf[..]));
                //    info!("Recieved {}", s);
                //}
                DEVLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on /dev/log");
                    ok_or_error!(devlog_sock.read(&mut buf));
                    info!("Read event done on /dev/log");
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
