use std::ffi::CString;
use std::io::Error;
use std::mem;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::os::unix::io::FromRawFd;

use libc::{
  AF_INET,
  SOCK_STREAM,
  SOL_SOCKET,
  SO_BINDTODEVICE,
  IFNAMSIZ,
  IPPROTO_TCP,
  in_addr,
  in_port_t,
  sa_family_t,
  sockaddr_in,
  strnlen,
  bind,
  connect,
  setsockopt,
  socket,
};

const INTERFACES: &[&'static str] = &[
  "enp0s31f6",
  "enp0s20f0u2",
  "enp0s20f0u1",
];

#[inline]
pub fn init_sockets() {
}

#[inline]
pub fn cleanup_sockets() {
}

pub fn make_socket(if_index: usize, remote_socket: SocketAddrV4) -> TcpStream {
  let local_addr = u32::from(Ipv4Addr::UNSPECIFIED).to_be();
  let remote_addr = u32::from(*remote_socket.ip()).to_be();

  let if_name = CString::new(INTERFACES[if_index % INTERFACES.len()]).unwrap();

  unsafe {
    let fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if fd < 0 {
      panic!("socket invalid (fd: {}), failed with: {:?}", fd, Error::last_os_error());
    }

    let len = strnlen(if_name.as_ptr(), IFNAMSIZ);
    if len == IFNAMSIZ {
      panic!("interface name is too long");
    }

    let result = setsockopt(fd, SOL_SOCKET, SO_BINDTODEVICE, if_name.as_ptr() as _, len as u32);
    if result != 0 {
      panic!("error after setsockopt(SOL_SOCKET, SO_BINDTODEVICE), failed with: {:?}", Error::last_os_error());
    }

    let local_service = sockaddr_in {
      sin_family: AF_INET as sa_family_t,
      sin_port: (0 as in_port_t).to_be(),
      sin_addr: in_addr {
        s_addr: local_addr,
      },
      sin_zero: [0; 8],
    };
    let result = bind(fd, mem::transmute(&local_service), mem::size_of::<sockaddr_in>() as u32);
    if result != 0 {
      panic!("error after bind, failed with: {:?}", Error::last_os_error());
    }

    let remote_service = sockaddr_in {
      sin_family: AF_INET as sa_family_t,
      sin_port: (remote_socket.port() as in_port_t).to_be(),
      sin_addr: in_addr {
        s_addr: remote_addr,
      },
      sin_zero: [0; 8],
    };
    let result = connect(fd, mem::transmute(&remote_service), mem::size_of::<sockaddr_in>() as u32);
    if result != 0 {
      panic!("error after connect, failed with: {:?}", Error::last_os_error());
    }

    TcpStream::from_raw_fd(fd)
  }
}
