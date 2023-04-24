use std::io::Write;
use std::net::Shutdown::Both;
use std::net::TcpStream;

use ctru::applets::swkbd::{Swkbd, Button, ValidInput, Filters};
use ctru::prelude::*;
use ctru::services::cam::{OutputFormat, BothOutwardCam};
use ctru::services::cfgu::Cfgu;
use ctru::services::cam::{Cam, Camera, FrameRate};
use thiserror::Error;

fn main() {
    ctru::use_panic_handler();
    
    let apt = Apt::init().unwrap();
    let mut hid = Hid::init().unwrap();
    let gfx = Gfx::init().unwrap();
    let soc = Soc::init().unwrap();
    let cfgu = Cfgu::init().unwrap();
    let console = Console::init(gfx.top_screen.borrow_mut());

    let address = soc.host_address();

    let mut cam = Cam::init().expect("Failed to initialize CAM service.");

    let mut camera = &mut cam.both_outer_cams;

    init_cameras(camera).unwrap();

    let mut status = AppStatus::NotConnected;

    let old_keys = KeyPad::empty();

    let mut stream_or_none: Option<TcpStream> = None;

    setup(&cfgu, &soc);
    while apt.main_loop() {

        hid.scan_input();
        let keys = hid.keys_held();

        if keys != old_keys
        {

            if keys.intersects(KeyPad::START) {
                println!("Exiting...");
                if let Some(stream) = stream_or_none {
                    if !stream.peek(&mut [0u8;0][..]).is_err() { // connected
                        stream.shutdown(Both).unwrap();
                    }
                }
                break;   
            }

            match status {
                AppStatus::NotConnected => {
                    if keys.intersects(KeyPad::X) {
                        status = AppStatus::Settings;
                        settings(camera);
                    }

                    if keys.intersects(KeyPad::A) {
                        match try_connect() {
                            Ok(Some(connection)) => {
                                println!("Connected to {}.", connection.peer_addr().unwrap());
                                stream_or_none = Some(connection);
                                status = AppStatus::Connected;

                            }
                            Ok(None) => {
                                println!("Cancelled");
                            }
                            Err(e) => println!("{}", e),
                        }
                    }
                }
                AppStatus::Settings => {
                    if keys.intersects(KeyPad::B) {
                        status = AppStatus::NotConnected;
                        console.clear();
                        setup(&cfgu, &soc);
                    }
                }
                AppStatus::Connected => {
                    if let Some(ref mut stream) = stream_or_none {
                        if keys.intersects(KeyPad::B) {
                            console.clear();
                            setup(&cfgu, &soc);
                            println!("Disconnected from {}.", stream.peer_addr().unwrap());
                            stream.shutdown(Both).unwrap();
                            status = AppStatus::NotConnected;
                        }
                    }
                    else {
                        status = AppStatus::NotConnected;
                    }
                }
            }
        }

        if status == AppStatus::Connected { // send camera data
            //todo: implement when ctru-rs adds the functionality
        }
        // Flush and swap framebuffers
        gfx.flush_buffers();
        gfx.swap_buffers();
        gfx.wait_for_vblank();
    }
}

fn setup(cfgu: &Cfgu, soc: &Soc) {
    println!("ctr-camera-rs v0.1.0 by Lena");
    println!("https://github.com/adryzz/ctr-camera-rs");
    println!("IP: {}, running on {:?}", soc.host_address(), cfgu.model().unwrap());
    println!("Press START to exit or A to connect to a server.");
    println!("Press X to go into the settings menu");

    println!("\u{001b}[46;1m                \u{001b}[0m");
    println!("\u{001b}[45;1m                \u{001b}[0m");
    println!("\u{001b}[47m                \u{001b}[0m");
    println!("\u{001b}[45;1m                \u{001b}[0m");
    println!("\u{001b}[46;1m                \u{001b}[0m");
}

fn settings(cam: &BothOutwardCam)
{
    println!("Selected camera: yes");
    println!("Auto exposure: {}", cam.is_auto_exposure_enabled().unwrap());
    println!("Auto white balance: {}", cam.is_auto_white_balance_enabled().unwrap());
    println!("Trimming: {}", cam.is_trimming_enabled().unwrap());
}

fn init_cameras(cam: &mut BothOutwardCam) -> Result<(), AppError> {
    cam.set_frame_rate(FrameRate::Fps30)?;

    cam.set_output_format(OutputFormat::Yuv422)?;

    Ok(())
}

fn try_connect() -> Result<Option<TcpStream>, AppError> {
    let text_or_none = get_keyboard_text()?;
    match text_or_none {
        Some(text) => {
            println!("Connecting to {}...", &text);
            return Ok(Some(TcpStream::connect(text)?));
        }
        None => return Ok(None),
    }
}

fn get_keyboard_text() -> Result<Option<String>, AppError> {
    let mut keyboard = Swkbd::default();

    keyboard.set_hint_text("192.168.1.1:5000");
    keyboard.set_max_text_len(64);
    keyboard.set_validation(ValidInput::NotEmptyNotBlank, Filters::BACKSLASH);
    
    match keyboard.get_string(64) {
        Ok((text, Button::Right)) => Ok(Some(text)),
        Ok((_, Button::Left)) => Ok(None),
        Ok((_, Button::Middle)) => Ok(None), // ??? unpressable
        Err(e) => Err(AppError::Swkbd(e)),
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum AppStatus {
    NotConnected,
    Connected,
    Settings
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Unknown error")]
    Unknown,
    #[error("libctru error")]
    Ctru(#[from] ctru::Error),
    #[error("Software keyboard error")]
    Swkbd(ctru::applets::swkbd::Error),
    #[error("I/O error")]
    Io(#[from] std::io::Error),
}