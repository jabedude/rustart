use log::{LevelFilter, info};
use libsystemd::{activation, daemon};

use mio::net::UnixDatagram;
use mio::{Events, Interest, Poll, Token};

//use std::os::unix::net::UnixDatagram;
use std::error::Error;
use std::os::unix::io::FromRawFd;
use std::os::unix::io::IntoRawFd;

/// /run/systemd/journal/socket
const SDJLOG: Token = Token(0);
/// /dev/log (I think)
const DEVLOG: Token = Token(1);
/// /run/systemd/journal/syslog
const SYSLOG: Token = Token(2);
/// Audit, looks like netlink
const AUDLOG: Token = Token(3);

fn main() {
    simple_logging::log_to_file("/vagrant/pid_eins.log", LevelFilter::Info).unwrap();

    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(128);

    let mut fds = activation::receive_descriptors(false).expect("Error receiving descriptors");
    info!("Received fds: {:?}", fds);

    let mut unk_sock = unsafe { UnixDatagram::from_raw_fd(fds.remove(1).into_raw_fd()) };
    poll.registry().register(&mut unk_sock, DEVLOG, Interest::READABLE).unwrap();

    let mut sdlog_sock = unsafe { UnixDatagram::from_raw_fd(fds.remove(2).into_raw_fd()) };
    poll.registry().register(&mut sdlog_sock, SDJLOG, Interest::READABLE).unwrap();

    daemon::notify(false, &[daemon::NotifyState::Ready]).unwrap();

    loop {
        poll.poll(&mut events, None).unwrap();

        info!("Got event...");
        for event in events.iter() {
            match event.token() {
                SDJLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on /run/systemd/journal/socket");
                    sdlog_sock.recv(&mut buf).expect("Error recieving");
                    match std::str::from_utf8(&buf[..]) {
                        Ok(s) => info!("Recieved {}", s),
                        Err(_) => continue,
                    };
                }
                DEVLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on /dev/log");
                    unk_sock.recv(&mut buf).expect("Error recieving");
                    match std::str::from_utf8(&buf[..]) {
                        Ok(s) => info!("Recieved {}", s),
                        Err(_) => continue,
                    };
                }
                _ => {
                    panic!();
                }
            }
        }
    }
}
