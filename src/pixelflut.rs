#![allow(dead_code)]

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};

use crate::socket::make_socket;

pub struct Pixelflut {
    pub stream: TcpStream,
    //reader: BufReader<TcpStream>,
}

impl Pixelflut {
    pub fn new(if_index: usize) -> Self {
        // maglan-srv-blade05.lan.magfest.net
        //let remote_addr = SocketAddrV4::new(Ipv4Addr::new(10, 13, 38, 233), 1234);
        let remote_socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1337);
        let stream = make_socket(if_index, remote_socket);
        //let reader = BufReader::new(stream.try_clone().unwrap());

        stream.set_nodelay(true).unwrap();
        stream.set_read_timeout(None).unwrap();
        stream.set_write_timeout(None).unwrap();

        Self {
            stream,
            //reader,
        }
    }

    #[inline]
    pub fn cmd(&mut self, cmd: &[u8]) -> std::io::Result<()> {
        //print!("cmd: ");
        //io::stdout().write_all(cmd)?;

        self.stream.write_all(cmd)?;

        Ok(())
    }

    #[inline]
    pub fn cmd_response(&mut self, buf: &mut [u8], cmd: &[u8]) -> std::io::Result<usize> {
        self.cmd(cmd)?;

        //let len = self.reader.read_line(buf)?;
        let len = self.stream.read(buf)?;
        println!("len: {}", len);

        Ok(len)
    }

    #[inline]
    pub fn cmd_print(&mut self, cmd: &[u8]) -> std::io::Result<()> {
        let mut buf = [0; 1024];

        let len = self.cmd_response(&mut buf, cmd)?;
        println!("result:");
        io::stdout().write_all(&buf[..len])?;
        println!();

        Ok(())
    }
}

pub fn print_capabilities() -> std::io::Result<()> {
    let mut pf = Pixelflut::new(0);

    pf.cmd_print(&b"SIZE\n"[..])?;
    pf.cmd_print(&b"CONNECTIONS\n"[..])?;
    pf.cmd_print(&b"HELP\n"[..])?;

    Ok(())
}
