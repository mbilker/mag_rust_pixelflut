use std::mem;
use std::net::{Ipv4Addr, TcpStream};
use std::os::windows::io::FromRawSocket;

use winapi::um::winsock2::{
  INVALID_SOCKET,
  SOCK_STREAM,
  SOCKET_ERROR,
  WSADATA,
  WSACleanup,
  WSAGetLastError,
  WSAStartup,
  connect,
  htons,
  bind,
  socket,
};
use winapi::shared::inaddr::IN_ADDR;
use winapi::shared::minwindef::MAKEWORD;
use winapi::shared::ws2def::{ADDRESS_FAMILY, AF_INET, IPPROTO_TCP, SOCKADDR_IN};

#[inline]
pub fn init_sockets() {
  unsafe {
    let mut wsa_data: WSADATA = mem::zeroed();

    WSAStartup(MAKEWORD(2, 2), &mut wsa_data);
  }
}

#[inline]
pub fn cleanup_sockets() {
  unsafe {
    WSACleanup();
  }
}

pub fn make_socket(local_addr: Ipv4Addr, remote_addr: Ipv4Addr) -> TcpStream {
  let local_addr = u32::from(local_addr).to_be();
  let remote_addr = u32::from(remote_addr).to_be();

  unsafe {
    let socket = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP as _);
    if socket == INVALID_SOCKET {
      panic!("socket invalid, failed with: {}", WSAGetLastError());
    }

    let mut addr: IN_ADDR = mem::zeroed();
    *addr.S_un.S_addr_mut() = local_addr;

    let local_service = SOCKADDR_IN {
      sin_family: AF_INET as ADDRESS_FAMILY,
      sin_port: htons(0),
      sin_addr: addr,
      sin_zero: [0; 8],
    };
    let result = bind(socket, mem::transmute(&local_service), mem::size_of::<SOCKADDR_IN>() as i32);
    //println!("bind result: {}", result);

    if result == SOCKET_ERROR {
      panic!("error after bind, failed with: {}", WSAGetLastError());
    }

    let mut addr: IN_ADDR = mem::zeroed();
    *addr.S_un.S_addr_mut() = remote_addr;

    let remote_service = SOCKADDR_IN {
      sin_family: AF_INET as ADDRESS_FAMILY,
      sin_port: htons(1234),
      sin_addr: addr,
      sin_zero: [0; 8],
    };
    let result = connect(socket, mem::transmute(&remote_service), mem::size_of::<SOCKADDR_IN>() as i32);
    //println!("connect result: {}", result);

    if result == SOCKET_ERROR {
      panic!("error after connect, failed with: {}", WSAGetLastError());
    }

    TcpStream::from_raw_socket(socket as _)
  }
}