#[macro_use] extern crate lazy_static;

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam::atomic::ArcCell;
use failure::Fallible;
//use ffmpeg::software;
use image::{GenericImageView, Pixel};
use rand::Rng;
use rand::seq::SliceRandom;

mod socket;

use crate::socket::{cleanup_sockets, init_sockets, make_socket};

const WIDTH: usize = 1024;
const HEIGHT: usize = 768;

const SPLIT_FACTOR: usize = 50;

lazy_static! {
    static ref CANVAS: ArcCell<Vec<Vec<u8>>> = {
        ArcCell::new(Arc::new(vec![vec!['\n' as u8]; SPLIT_FACTOR]))
    };
}

struct Pixelflut {
    stream: TcpStream,
}

impl Pixelflut {
    fn new(local_addr: Ipv4Addr) -> Self {
        let remote_addr = Ipv4Addr::new(10, 13, 38, 233);
        let stream = make_socket(local_addr, remote_addr);
        Self {
            stream,
        }
    }

    #[inline]
    fn cmd(&mut self, cmd: &[u8]) -> std::io::Result<()> {
        //print!("cmd: ");
        //io::stdout().write_all(cmd)?;

        self.stream.write_all(cmd)?;

        Ok(())
    }

    #[inline]
    #[allow(dead_code)]
    fn cmd_response(&mut self, buf: &mut [u8], cmd: &[u8]) -> std::io::Result<usize> {
        self.cmd(cmd)?;

        let len = self.stream.read(buf)?;
        println!("len: {}", len);

        Ok(len)
    }

    #[inline]
    #[allow(dead_code)]
    fn cmd_print(&mut self, cmd: &[u8]) -> std::io::Result<()> {
        let mut buf = [0; 1024];

        let len = self.cmd_response(&mut buf, cmd)?;
        println!("result:");
        io::stdout().write_all(&buf[..len])?;
        println!();

        Ok(())
    }
}

#[allow(dead_code)]
fn get_capabilities() -> std::io::Result<()> {
    let mut pf = Pixelflut::new(Ipv4Addr::UNSPECIFIED);

    pf.cmd_print(&b"SIZE\n"[..])?;
    pf.cmd_print(&b"CONNECTIONS\n"[..])?;
    pf.cmd_print(&b"HELP\n"[..])?;

    Ok(())
}

#[inline]
fn static_bg() -> Vec<Vec<u8>> {
    let start_x = 0;
    let end_x = WIDTH;

    let start_y = 0;
    let end_y = HEIGHT;

    let mut rng = rand::thread_rng();

    let mut cmd_buf = Vec::new();
    let mut current_color: [u8; 3] = [0; 3];

    let mut x_pos: Vec<_> = (start_x..end_x).collect();
    x_pos.shuffle(&mut rng);

    let mut y_pos: Vec<_> = (start_y..end_y).collect();
    for x in x_pos.into_iter() {
        y_pos.shuffle(&mut rng);

        for y in y_pos.iter() {
            rng.fill(&mut current_color);

            //cmd_buf.push(format!("PX {} {} 000000\n", x, y));
            cmd_buf.push(format!("PX {} {} {:02x}{:02x}{:02x}\n",
                                 x, y,
                                 current_color[0],
                                 current_color[1],
                                 current_color[2]));
        }
    }

    let cmd_buf: Vec<_> = cmd_buf.into_iter()
        .map(String::into_bytes)
        .collect();

    cmd_buf
}

#[inline]
fn image_bg() -> Vec<Vec<u8>> {
    let mut rng = rand::thread_rng();

    let img = image::open("bg05.png").unwrap();

    let (width, height) = img.dimensions();
    let buf = img.to_rgb();
    let mut cmd_buf = Vec::with_capacity(width as usize * height as usize);

    for (x, y, px) in buf.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);

        if x >= WIDTH || y >= HEIGHT {
            continue;
        }

        let col = px.channels();
        cmd_buf.push(format!("PX {} {} {:02x}{:02x}{:02x}\n",
            x, y,
            col[0],
            col[1],
            col[2]));
    }

    cmd_buf.shuffle(&mut rng);

    let cmd_buf: Vec<_> = cmd_buf.into_iter()
        .map(String::into_bytes)
        .collect();

    cmd_buf
}

/*
#[inline]
fn video_bg() -> Vec<Vec<u8>> {
    format::input("video.mp4").unwrap();
    unimplemented!();
}
*/

#[inline]
fn canvas_thread() -> Fallible<()> {
    loop {
        println!("update");

        //let cmd_buf = static_bg();
        let cmd_buf = image_bg();
        let chunk_size = cmd_buf.len() / SPLIT_FACTOR;

        let mut thread_data: Vec<Vec<u8>> = Vec::with_capacity(SPLIT_FACTOR);

        for (i, part) in cmd_buf.into_iter().enumerate() {
            if let Some(thread_data) = thread_data.get_mut(i / chunk_size) {
                thread_data.extend(part);
            } else {
                thread_data.push(part);
            }
        }

        /*
        for (i, part) in cmd_buf.into_iter().enumerate() {
            if i < SPLIT_FACTOR {
                thread_data.push(part);
            } else {
                thread_data[i % SPLIT_FACTOR].extend(part);
            }
        }
        */

        CANVAS.set(Arc::new(thread_data));

        thread::sleep(Duration::new(1, 0));
    }

    //Ok(())
}

#[inline]
fn network_thread(i: usize, local_addr: Ipv4Addr) -> Fallible<()> {
    let canvas = &CANVAS;

    let mut pf = Pixelflut::new(local_addr);
    println!("{}: connected", i);

    loop {
        let data = canvas.get();
        let data = &data[i];

        pf.cmd(&data)?;
    }

    //Ok(())
}

fn main() {
    let addrs = &[
        Ipv4Addr::new(10, 13, 38, 159),
        Ipv4Addr::new(10, 13, 39, 162),
    ];
    let mut handles = Vec::with_capacity(8);

    init_sockets();

    handles.push(thread::spawn(move || {
        canvas_thread().unwrap();
    }));

    for i in 0..SPLIT_FACTOR {
        let addr = addrs[i % addrs.len()];

        handles.push(thread::spawn(move || {
            loop {
                if let Err(e) = network_thread(i, addr){
                    eprintln!("Error in network thread {}: {}", i, e);
                }
            }
        }));
    }

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }

    cleanup_sockets();
}
