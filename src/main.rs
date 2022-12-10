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
    Win32::System::WinRT::IMemoryBufferByteAccess,
};

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::ptr;

fn main() -> Result<()> {
    let path = std::env::args().nth(1).unwrap_or_default();
    if path.len() == 0 {
        panic!("input path required");
    }

    let uri = Uri::CreateUri(&HSTRING::from(&path))?;
    let source = MediaSource::CreateFromUri(&uri)?;

    const FACTOR: i32 = 8;
    const WIDTH: i32 = 16 * FACTOR;
    const HEIGHT: i32 = 9 * FACTOR;

    let video_frame = VideoFrame::CreateAsDirect3D11SurfaceBacked(
        DirectXPixelFormat::B8G8R8X8UIntNormalized,
        WIDTH,
        HEIGHT,
    )?;

    let software_bitmap = SoftwareBitmap::CreateWithAlpha(
        BitmapPixelFormat::Bgra8,
        WIDTH,
        HEIGHT,
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
