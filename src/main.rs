use windows::{
    core::*,
    Foundation::{AsyncActionCompletedHandler, AsyncStatus, TypedEventHandler, Uri},
    Graphics::DirectX::DirectXPixelFormat,
    Graphics::Imaging::{
        BitmapAlphaMode, BitmapBufferAccessMode, BitmapPixelFormat, SoftwareBitmap,
    },
    Media::Core::MediaSource,
    Media::Playback::MediaPlayer,
    Media::VideoFrame,
    Win32::System::Console::SetConsoleCtrlHandler,
    Win32::System::WinRT::IMemoryBufferByteAccess,
};

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::ptr;
use windows::Win32::Foundation::BOOL;

extern "system" {
    fn _getch_nolock() -> i32;
}

fn main() -> Result<()> {
    let path = std::env::args().nth(1).unwrap_or_default();
    if path.len() == 0 {
        panic!("input path required");
    }

    let uri = Uri::CreateUri(&HSTRING::from(&path))?;
    let source = MediaSource::CreateFromUri(&uri)?;

    // A quick hack (for demo purposes) to get the window size via VT.
    let (width, height): (i32, i32) = {
        print!("\x1b[9999;9999H\x1b[6n");
        std::io::stdout().flush().unwrap();

        let mut buf = [0u8; 128];
        let mut len = 0usize;

        loop {
            let ch = unsafe { _getch_nolock() } as u8;
            buf[len] = ch;
            len += 1;
            if ch == b'R' {
                break;
            }
        }

        let (h, w) = unsafe { std::str::from_utf8_unchecked(&buf[..len]) }
            .strip_prefix("\x1b[")
            .unwrap()
            .strip_suffix("R")
            .unwrap()
            .split_once(';')
            .unwrap();

        (w.parse().unwrap(), h.parse().unwrap())
    };

    // Since we use use "square" pixels via U+2580 we have twice the vertical resolution.
    let height = height * 2;

    print!("\x1b[?1049h\x1b[?25l");
    std::io::stdout().flush().unwrap();

    unsafe {
        extern "system" fn foo(_: u32) -> BOOL {
            print!("\x1b[?25h\x1b[?1049l");
            std::io::stdout().flush().unwrap();
            BOOL(0)
        }

        SetConsoleCtrlHandler(Some(foo), BOOL(1));
    };

    let video_frame = VideoFrame::CreateAsDirect3D11SurfaceBacked(
        DirectXPixelFormat::B8G8R8X8UIntNormalized,
        width,
        height,
    )?;

    let software_bitmap = SoftwareBitmap::CreateWithAlpha(
        BitmapPixelFormat::Bgra8,
        width,
        height,
        BitmapAlphaMode::Ignore,
    )?;
    let software_video_frame = VideoFrame::CreateWithSoftwareBitmap(&software_bitmap)?;

    let player = MediaPlayer::new()?;
    player.SetSource(&source)?;
    player.SetRealTimePlayback(true)?;
    player.SetIsVideoFrameServerEnabled(true)?;
    player.VideoFrameAvailable(&TypedEventHandler::new(
        move |player: &Option<MediaPlayer>, _| -> Result<()> {
            let player = match player {
                Some(player) => player,
                None => return Ok(()),
            };

            let video_frame_surface = video_frame.Direct3DSurface()?;
            let software_bitmap = software_bitmap.clone();
            player.CopyFrameToVideoSurface(&video_frame_surface)?;

            video_frame
                .CopyToAsync(&software_video_frame)?
                .SetCompleted(&AsyncActionCompletedHandler::new(
                    move |_, status| -> Result<()> {
                        if status != AsyncStatus::Completed {
                            return Ok(());
                        }

                        let bitmap_buffer =
                            software_bitmap.LockBuffer(BitmapBufferAccessMode::Read)?;
                        let description = bitmap_buffer.GetPlaneDescription(0)?;
                        let reference: IMemoryBufferByteAccess =
                            bitmap_buffer.CreateReference()?.cast()?;

                        const PREFIX: &'static str = "\x1b[H";
                        const SUFFIX: &'static str = "\x1b[0m\n";
                        let pixel_count = (description.Width * description.Height) as usize;
                        let mut output =
                            String::with_capacity(PREFIX.len() + SUFFIX.len() + 41 * pixel_count);
                        output.push_str(PREFIX);

                        unsafe {
                            let mut value = ptr::null_mut();
                            let mut capacity = 0u32;
                            reference.GetBuffer(&mut value, &mut capacity)?;

                            let buffer = std::slice::from_raw_parts(value, capacity as usize);

                            for y in (0..(description.Height - 1)).step_by(2) {
                                let mut i0 = (y * description.Stride) as usize;
                                let mut i1 = ((y + 1) * description.Stride) as usize;

                                for _ in 0..description.Width {
                                    let r0 = buffer[i0 + 2];
                                    let g0 = buffer[i0 + 1];
                                    let b0 = buffer[i0 + 0];
                                    let r1 = buffer[i1 + 2];
                                    let g1 = buffer[i1 + 1];
                                    let b1 = buffer[i1 + 0];
                                    i0 += 4;
                                    i1 += 4;

                                    let _ = write!(
                                        output,
                                        "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                                        r0, g0, b0, r1, g1, b1
                                    );
                                }

                                output.push_str(SUFFIX);
                            }
                        }

                        // remove the trailing newline
                        output.pop();

                        let stdout = std::io::stdout();
                        let mut stdout = stdout.lock();
                        let _ = stdout.write_all(output.as_bytes());
                        let _ = stdout.flush();
                        Ok(())
                    },
                ))?;

            Ok(())
        },
    ))?;

    println!("\x1b[2J");
    player.Play()?;

    std::io::stdin().bytes().next();
    Ok(())
}
