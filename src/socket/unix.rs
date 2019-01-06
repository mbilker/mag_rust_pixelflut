use std::io::Error;
use std::net::{Ipv4Addr, TcpStream};
use std::os::unix::io::FromRawFd;

use libc::{
  AF_INET,
  IPPROTO_TCP,
  in_addr,
  sockaddr_in,
  bind,
  connect,
  socket,
};

#[inline]
pub fn init_sockets() {
}

#[inline]
pub fn cleanup_sockets() {
}

pub fn make_socket(local_addr: Ipv4Addr, remote_addr: Ipv4Addr) -> TcpStream {
  let local_addr = u32::from(local_addr).to_be();

  unsafe {
    let fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if fd != 0 {
      panic!("socket invalid, failed with: {:?}", Error::last_os_error());
    }

    let local_service = sockaddr_in {
      sin_family: AF_INET,
      sin_port: 0.to_be(),
      sin_addr: in_addr {
        s_addr: local_addr,
      },
      sin_zero: [0; 8],
    };
    let result = bind(fd, &local_service, mem::size_of::<sockaddr_in>());
    if result != 0 {
      panic!("error after bind, failed with: {:?}", Error::last_os_error());
    }

    let remote_service = sockaddr_in {
      sin_family: AF_INET,
      sin_port: 1234.to_be(),
      sin_addr: remote_addr,
      sin_zero: [0; 8],
    };
    let result = connect(fd, &remote_service, mem::size_of::<sockaddr_in>());
    if result != 0 {
      panic!("error after connect, failed with: {:?}", Error::last_os_error());
    }

    TcpStream::from_raw_fd(fd)
  }
}