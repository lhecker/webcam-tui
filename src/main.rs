mod bindings {
    ::windows::include_bindings!();
}

use bindings::{
    Windows::Foundation::{AsyncActionCompletedHandler, AsyncStatus, TypedEventHandler, Uri},
    Windows::Graphics::DirectX::DirectXPixelFormat,
    Windows::Graphics::Imaging::{
        BitmapAlphaMode,
        BitmapBufferAccessMode,
        BitmapPixelFormat,
        SoftwareBitmap,
    },
    Windows::Media::Core::MediaSource,
    Windows::Media::Playback::MediaPlayer,
    Windows::Media::VideoFrame,
};

use std::fmt::Write as _;
use std::io::{Read, Write};
use windows::Interface;

#[repr(transparent)]
#[derive(std::cmp::PartialEq, std::cmp::Eq, std::clone::Clone, std::fmt::Debug)]
pub struct IMemoryBufferByteAccess(windows::Object);

impl IMemoryBufferByteAccess {
    #[allow(non_snake_case)]
    pub fn GetBuffer(&self) -> windows::Result<&[u8]> {
        let this = self;
        let mut ptr = std::ptr::null_mut();
        let mut capacity: u32 = 0;
        unsafe {
            (windows::Interface::vtable(this).3)(windows::Abi::abi(this), &mut ptr, &mut capacity)
                .ok()
                .map(|_| {
                    if capacity != 0 {
                        std::slice::from_raw_parts(ptr, capacity as usize)
                    } else {
                        &[]
                    }
                })
        }
    }
}

unsafe impl windows::Interface for IMemoryBufferByteAccess {
    type Vtable = IMemoryBufferByteAccess_abi;
    const IID: windows::Guid = windows::Guid::from_values(
        0x5b0d3235,
        0x4dba,
        0x4d44,
        [0x86, 0x5e, 0x8f, 0x1d, 0x0e, 0x4f, 0xd0, 0x4d],
    );
}

#[repr(C)]
pub struct IMemoryBufferByteAccess_abi(
    pub  unsafe extern "system" fn(
        this: ::windows::RawPtr,
        iid: &::windows::Guid,
        interface: *mut ::windows::RawPtr,
    ) -> ::windows::ErrorCode,
    pub unsafe extern "system" fn(this: ::windows::RawPtr) -> u32,
    pub unsafe extern "system" fn(this: ::windows::RawPtr) -> u32,
    pub  unsafe extern "system" fn(
        this: windows::RawPtr,
        value: *mut *mut u8,
        capacity: *mut u32,
    ) -> windows::ErrorCode,
);

fn main() -> windows::Result<()> {
    let path = std::env::args().nth(1).unwrap_or_default();
    if path.len() == 0 {
        panic!("input path required");
    }

    windows::initialize_mta().unwrap();

    let uri = Uri::CreateUri(path)?;
    let source = MediaSource::CreateFromUri(uri)?;

    const FACTOR: i32 = 8;
    const WIDTH: i32 = 16 * FACTOR;
    const HEIGHT: i32 = 9 * FACTOR;

    let video_frame = VideoFrame::CreateAsDirect3D11SurfaceBacked(
        DirectXPixelFormat::B8G8R8X8UIntNormalized,
        WIDTH,
        HEIGHT,
    )?;
    let video_frame_surface = video_frame.Direct3DSurface()?;

    let software_bitmap = SoftwareBitmap::CreateWithAlpha(
        BitmapPixelFormat::Bgra8,
        WIDTH,
        HEIGHT,
        BitmapAlphaMode::Ignore,
    )?;
    let software_video_frame = VideoFrame::CreateWithSoftwareBitmap(&software_bitmap)?;

    let player = MediaPlayer::new()?;
    player.SetSource(source)?;
    player.SetRealTimePlayback(true)?;
    player.SetIsVideoFrameServerEnabled(true)?;
    player.VideoFrameAvailable(TypedEventHandler::new(
        move |player: &Option<MediaPlayer>, _| -> windows::Result<()> {
            let player = match player {
                Some(player) => player,
                None => return Ok(()),
            };

            let software_bitmap = software_bitmap.clone();
            player.CopyFrameToVideoSurface(&video_frame_surface)?;

            video_frame
                .CopyToAsync(&software_video_frame)?
                .SetCompleted(AsyncActionCompletedHandler::new(
                    move |_, status| -> windows::Result<()> {
                        if status != AsyncStatus::Completed {
                            return Ok(());
                        }

                        let bitmap_buffer =
                            software_bitmap.LockBuffer(BitmapBufferAccessMode::Read)?;
                        let description = bitmap_buffer.GetPlaneDescription(0)?;
                        let reference: IMemoryBufferByteAccess =
                            bitmap_buffer.CreateReference()?.cast()?;
                        let buffer = reference.GetBuffer()?;

                        const PREFIX: &'static str = "\x1b[H";
                        const SUFFIX: &'static str = "\x1b[0m\n";
                        let pixel_count = (description.Width * description.Height) as usize;
                        let mut output =
                            String::with_capacity(PREFIX.len() + SUFFIX.len() + 41 * pixel_count);
                        output.push_str(PREFIX);

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
