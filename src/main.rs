#![feature(const_ip)]

#[macro_use] extern crate failure;
//#[macro_use] extern crate lazy_static;

use std::env;
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::mem;
//use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

//use crossbeam::atomic::ArcCell;
use failure::Fallible;
use image::{FilterType, GenericImageView, ImageBuffer, Pixel};
use rand::Rng;
use rand::seq::SliceRandom;

mod pixelflut;
mod socket;

use crate::pixelflut::Pixelflut;
use crate::socket::{cleanup_sockets, init_sockets};

const WIDTH: usize = 1024;
const HEIGHT: usize = 768;

const SPLIT_FACTOR: usize = 1;

/// Array of byte strings
type CommandBuffer = Vec<Vec<u8>>;

type ThreadSender = mpsc::Sender<DrawCall>;
type ThreadReceiver = mpsc::Receiver<DrawCall>;

/*
/// Array of bytes for each thread
type ThreadData = Vec<Vec<u8>>;

lazy_static! {
    static ref CANVAS: ArcCell<ThreadData> = {
        let mut renderer = Renderer::new().unwrap();
        let thread_data = renderer.update_canvas().unwrap();
        ArcCell::new(Arc::new(thread_data))
    };
}
*/

#[inline]
#[allow(dead_code)]
fn static_bg() -> CommandBuffer {
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
fn image_bg(cmd_buf: &mut Vec<DrawCall>) {
    let (offset_x, offset_y) = (0, 0);

    let width = (WIDTH - offset_x) as u32;
    let height = (HEIGHT - offset_y) as u32;

    let img = image::open("kaiden.png").unwrap();
    let img = img.adjust_contrast(2.0);
    //let img = img.crop(0, 100, 500, 350);
    let img = img.resize(width, height, FilterType::CatmullRom);

    let buf = img.to_rgba();

    for (x, y, px) in buf.enumerate_pixels() {
        let x = x as usize + offset_x;
        let y = y as usize + offset_y;

        if x >= WIDTH || y >= HEIGHT {
            continue;
        }

        let col = px.channels();
        if col[3] > 0 {
            /*
            cmd_buf.push(format!("PX {} {} {:02x}{:02x}{:02x}\n",
                x, y,
                col[0],
                col[1],
                col[2]));
            */
            cmd_buf.push(DrawCall {
                x,
                y,
                color: Color::Rgb([
                    col[0],
                    col[1],
                    col[2],
                ]),
            });
        }
    }
}

enum Color {
  Grayscale(u8),
  Rgb([u8; 3]),
  Rgba([u8; 4]),
}

struct DrawCall {
  x: usize,
  y: usize,
  color: Color,
}

impl fmt::Display for Color {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Color::Grayscale(c) => write!(f, "{:02x}", c),
      Color::Rgb([r, g, b]) => write!(f, "{:02x}{:02x}{:02x}", r, g, b),
      Color::Rgba([r, g, b, a]) => write!(f, "{:02x}{:02x}{:02x}{:02x}", r, g, b, a),
    }
  }
}

use ffmpeg::codec::decoder::video::Video as VideoDecoder;
use ffmpeg::format::context::input::Input;
use ffmpeg::util::frame::video::Video as VideoFrame;

struct VideoRenderer {
  ictx: Input,
  index: usize,
  decoder: VideoDecoder,
  frame: VideoFrame,
  scaled_frame: VideoFrame,
  rgb_frame: VideoFrame,
}

impl VideoRenderer {
  #[inline]
  fn new() -> Fallible<Self> {
    use ffmpeg::format;
    use ffmpeg::media::Type;

    let ictx = format::input(&"1479219047354.webm").unwrap();

    let stream = ictx.streams().best(Type::Video).ok_or_else(|| format_err!("No video stream found"))?;
    let index = stream.index();
    println!("stream index: {}", index);

    let frame = VideoFrame::empty();
    let scaled_frame = VideoFrame::empty();
    let rgb_frame = VideoFrame::empty();

    let decoder = stream.codec().decoder().video()?;

    Ok(Self {
      ictx,
      index,
      decoder,
      frame,
      scaled_frame,
      rgb_frame,
    })
  }

  fn next_frame(&mut self, cmd_buf: &mut Vec<DrawCall>) -> Fallible<()> {
    use ffmpeg::util::format::pixel::Pixel;

    for (stream, packet) in self.ictx.packets() {
      if stream.index() != self.index {
        continue;
      }

      let result = self.decoder.decode(&packet, &mut self.frame)?;
      if !result {
        continue;
      }

      //println!("planes: {}, pix_fmt: {:?}", frame.planes(), frame.format());

      let mut ctx = self.frame.converter(Pixel::RGB24)?;
      ctx.run(&self.frame, &mut self.rgb_frame)?;

      let width = self.rgb_frame.width() as usize;
      let height = self.rgb_frame.height() as usize;
      //println!("planes: {}, pix_fmt: {:?}", rgb_frame.planes(), rgb_frame.format());

      let data = self.rgb_frame.data(0);
      let linesize = data.len() / height;
      //println!("data len: {} (h*w: {}, linesize: {})", data.len(), height * width, linesize);

      for y in 0..height {
        for x in 0..width {
          cmd_buf.push(DrawCall {
            x,
            y,
            color: Color::Rgb([
              data[x * 3 + y * linesize],
              data[x * 3 + y * linesize + 1],
              data[x * 3 + y * linesize + 2],
            ]),
          });
        }
      }

      break;
    }

    if cmd_buf.is_empty() {
      self.ictx.seek(0, 0..)?;
      self.frame = VideoFrame::empty();
      self.rgb_frame = VideoFrame::empty();
    }

    Ok(())

  }
}

struct Renderer {
    thread_senders: Vec<ThreadSender>,
    cmd_buf: Vec<DrawCall>,
    video: VideoRenderer,
}

impl Renderer {
    fn new(thread_senders: Vec<ThreadSender>) -> Fallible<Self> {
        Ok(Self {
            thread_senders,
            cmd_buf: Vec::new(),
            video: VideoRenderer::new()?,
        })
    }

    fn update_canvas(&mut self) -> Fallible<()> {
        println!("update");

        let mut rng = rand::thread_rng();

        //let cmd_buf = static_bg();
        image_bg(&mut self.cmd_buf);
        //let cmd_buf = video_bg()?;
        //self.video.next_frame(&mut self.cmd_buf)?;

        self.cmd_buf.shuffle(&mut rng);

        println!("commands: {}", self.cmd_buf.len());
        /*
        let chunk_size = self.cmd_buf.len() / SPLIT_FACTOR;
        println!("commands: {}, chunk_size: {}", self.cmd_buf.len(), chunk_size);

        let mut current_thread_data = Vec::with_capacity(chunk_size);
        let mut thread_index = 0;

        let iter = self.cmd_buf.drain(..);
        for part in iter {
            if current_thread_data.len() < chunk_size {
                //current_thread_data.write_fmt(format_args!("PX {} {} {}\n", part.x, part.y, part.color))?;
                current_thread_data.push(part);
            } else {
                let thread_data = mem::replace(&mut current_thread_data, Vec::with_capacity(chunk_size));
                self.thread_senders[thread_index].send(thread_data)?;

                thread_index += 1;
                thread_index = thread_index % self.thread_senders.len();
            }
        }

        if !current_thread_data.is_empty() {
            self.thread_senders[thread_index].send(current_thread_data)?;
        }
        */
        let len = self.thread_senders.len();
        for (i, part) in self.cmd_buf.drain(..).enumerate() {
          self.thread_senders[i % len].send(part)?;
        }

        Ok(())
    }
}

#[inline]
fn canvas_thread(senders: Vec<ThreadSender>) -> Fallible<()> {
    let mut renderer = Renderer::new(senders)?;

    loop {
        //let thread_data = renderer.update_canvas()?;
        //CANVAS.set(Arc::new(thread_data));
        renderer.update_canvas()?;

        //thread::sleep(Duration::new(0, 1000));
    }

    //Ok(())
}

#[inline]
fn network_thread(i: usize, if_index: usize, receiver: &mut ThreadReceiver) -> Fallible<()> {
    use std::io::Cursor;

    let mut pf = Pixelflut::new(if_index);
    println!("{}: connected", i);

    const BUF_SIZE: u64 = 10240;

    // PX + 3 spaces + 4 digit x + 4 digit y + 6 digit color
    const MAX_SIZE: u64 = 2 + 1 + 4 + 1 + 4 + 1 + 6;

    let mut buf = [0u8; BUF_SIZE as usize];
    let mut cursor = Cursor::new(&mut buf[..]);

    loop {
        let data = receiver.recv().expect("receiver should be open");
        if cursor.position() + MAX_SIZE <= BUF_SIZE {
          //cursor.write_all(b"PX ")?;
          writeln!(cursor, "PX {} {} {}\n", data.x, data.y, data.color)?;
        } else {
          pf.cmd(&cursor.get_ref()[..])?;
          cursor.set_position(0);
        }
    }

    //Ok(())
}

fn pull_image() -> Fallible<()> {
    println!("establishing connection");
    let mut pf = Pixelflut::new(0);

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

fn parse_image() -> Fallible<()> {
  use std::fs::File;

  let file = File::open("pixelflut_image_data.txt")?;
  let mut reader = BufReader::new(file);

  let width = WIDTH;
  let height = HEIGHT;
  println!("width: {}, height: {}", width, height);

  let mut canvas = ImageBuffer::new(width as u32, height as u32);
  let mut buf = String::new();

  let mut retrieved = vec![vec![false; height]; width];

  for i_x in 0..width {
    for i_y in 0..height {
      let len = reader.read_line(&mut buf)?;
      println!("({},{}) result: {} (len: {})", i_x, i_y, buf.trim(), len);

      if len == 0 {
          break;
      }

      // PX {x} {y} {color}
      let mut values = buf.split(' ').skip(1);
      let x = values.next().unwrap().parse::<u32>()?;
      let y = values.next().unwrap().parse::<u32>()?;
      let col = values.next().unwrap();
      println!("col: {}", col.trim());

      retrieved[x as usize][y as usize] = true;

      let r = u8::from_str_radix(&col[0..2], 16)?;
      let g = u8::from_str_radix(&col[2..4], 16)?;
      let b = u8::from_str_radix(&col[4..6], 16)?;
      println!("(x, y) (r,g,b): ({},{}) ({},{},{})", x, y, r, g, b);
      canvas[(x as u32, y as u32)] = image::Rgb([r, g, b]);

      buf.clear();
    }
  }

  // List out the missing pixels
  println!("Missing pixels:");
  for (x, vec) in retrieved.into_iter().enumerate() {
    for (y, v) in vec.into_iter().enumerate() {
      if !v {
        println!("({},{})", x, y);
      }
    }
  }

  canvas.save("pixelflut.png").unwrap();

  Err(std::io::Error::last_os_error().into())
}

fn run_threads() {
    const INTERFACES_LEN: usize = 3;

    // Make sure the canvas function works
    //update_canvas().unwrap();

    let mut handles = Vec::with_capacity(SPLIT_FACTOR + 1);
    let mut senders = Vec::with_capacity(SPLIT_FACTOR);

    for i in 0..SPLIT_FACTOR {
        let if_index = i % INTERFACES_LEN;
        let (tx, mut rx) = mpsc::channel();

        senders.push(tx);
        handles.push(thread::spawn(move || {
            loop {
                if let Err(e) = network_thread(i, if_index, &mut rx) {
                    eprintln!("Error in network thread {}: {}", i, e);
                }
            }
        }));
        thread::sleep(Duration::new(0, 1000));
    }

    handles.push(thread::spawn(move || {
        canvas_thread(senders).unwrap();
    }));

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }
}

fn main() {
    init_sockets();
    ffmpeg::init().unwrap();

    let args: Vec<_> = env::args().collect();

    if let Some(arg) = args.get(1) {
      match arg.as_str() {
        "dump" => parse_image().unwrap(),
        _ => run_threads(),
      };
    } else {
      run_threads();
    }

    //pull_image().unwrap();
    //parse_image().unwrap();

    cleanup_sockets();
}
