#[macro_use] extern crate lazy_static;

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam::atomic::ArcCell;
use failure::Fallible;
//use ffmpeg::software;
use image::{FilterType, GenericImageView, ImageBuffer, Pixel};
use rand::Rng;
use rand::seq::SliceRandom;

mod socket;

use crate::socket::{cleanup_sockets, init_sockets, make_socket};

const WIDTH: usize = 1024;
const HEIGHT: usize = 768;

const SPLIT_FACTOR: usize = 900;

lazy_static! {
    static ref CANVAS: ArcCell<Vec<Vec<u8>>> = {
        ArcCell::new(Arc::new(vec![vec!['\n' as u8]; SPLIT_FACTOR]))
    };
}

struct Pixelflut {
    stream: TcpStream,
    //reader: BufReader<TcpStream>,
}

impl Pixelflut {
    fn new(local_addr: Ipv4Addr) -> Self {
        let remote_addr = Ipv4Addr::new(10, 13, 38, 233);
        let stream = make_socket(local_addr, remote_addr);
        //let reader = BufReader::new(stream.try_clone().unwrap());
        Self {
            stream,
            //reader,
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

        //let len = self.reader.read_line(buf)?;
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

    let img = image::open("kaiden.png").unwrap();
    let img = img.resize_to_fill(WIDTH as u32, HEIGHT as u32, FilterType::CatmullRom);

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

#[inline]
fn canvas_thread() -> Fallible<()> {
    loop {
        println!("update");

        //let cmd_buf = static_bg();
        let cmd_buf = image_bg();
        let chunk_size = cmd_buf.len() / SPLIT_FACTOR;

        println!("commands: {}, chunk_size: {}", cmd_buf.len(), chunk_size);

        let mut thread_data: Vec<Vec<u8>> = Vec::with_capacity(SPLIT_FACTOR);

        for (i, part) in cmd_buf.into_iter().enumerate() {
            if let Some(thread_data) = thread_data.get_mut(i / chunk_size) {
                thread_data.extend(part);
            } else {
                thread_data.push(part);
            }
        }

        /*
        for (i, thread) in thread_data.iter().enumerate() {
            let num_commands = thread.iter()
                .filter(|&ch| *ch == b'\n')
                .count();

            println!("thread_data[{}].len() = {}", i, num_commands);
        }
        */

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

fn pull_image() -> Fallible<()> {
    let mut pf = Pixelflut::new(Ipv4Addr::UNSPECIFIED);
    let mut buf = [0; 1024];

    let len = pf.cmd_response(&mut buf, &b"SIZE\n"[..])?;
    let value = std::str::from_utf8(&buf[..len - 1])?;
    let values: Vec<_> = value.split(' ').collect();
    //let values: Vec<_> = buf[..len - 1].split(' ').collect();

    let width = values[1].parse::<usize>()?;
    let height = values[2].parse::<usize>()?;
    println!("width: {}, height: {}", width, height);

    let mut canvas = ImageBuffer::new(width as u32, height as u32);
    let mut cmd_buf = Vec::new();

    for x in 0..width {
        println!("establishing connection");
        pf = Pixelflut::new(Ipv4Addr::UNSPECIFIED);
        pf.stream.set_nodelay(true)?;
        pf.stream.set_read_timeout(None)?;
        pf.stream.set_write_timeout(None)?;

        cmd_buf.clear();

        for y in 0..height {
            cmd_buf.write_fmt(format_args!("PX {} {}\n", x, y))?;
        }

        println!("writing to buf");
        pf.cmd(&cmd_buf)?;
        println!("flushing");
        pf.stream.flush()?;

        let mut reader = BufReader::new(&mut pf.stream);
        let mut buf = String::new();

        for _ in 0..height {
            let len = reader.read_line(&mut buf)?;
            println!("result: {} (len: {})", buf.trim(), len);

            if len == 0 {
                break;
            }

            // PX {x} {y} {color}
            let mut values = buf.split(' ').skip(1);
            let x = values.next().unwrap().parse::<u32>()?;
            let y = values.next().unwrap().parse::<u32>()?;
            let col = values.next().unwrap();
            println!("col: {}", col.trim());

            let r = u8::from_str_radix(&col[0..2], 16)?;
            let g = u8::from_str_radix(&col[2..4], 16)?;
            let b = u8::from_str_radix(&col[4..6], 16)?;
            println!("(x, y) (r,g,b): ({},{}) ({},{},{})", x, y, r, g, b);
            canvas[(x as u32, y as u32)] = image::Rgb([r, g, b]);

            buf.clear();
        }
    }

    canvas.save("pixelflut.png").unwrap();

    Err(std::io::Error::last_os_error().into())
}

fn main() {
    let addrs = &[
        Ipv4Addr::new(10, 13, 38, 159),
        Ipv4Addr::new(10, 13, 39, 162),
    ];
    let mut handles = Vec::with_capacity(8);

    init_sockets();

    pull_image().unwrap();

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
