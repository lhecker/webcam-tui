fn main() {
    windows::build!(
        Windows::Foundation::*,
        Windows::Graphics::DirectX::Direct3D11::IDirect3DSurface,
        Windows::Graphics::DirectX::DirectXPixelFormat,
        Windows::Graphics::Imaging::{BitmapAlphaMode, BitmapBuffer, BitmapBufferAccessMode, BitmapPixelFormat, BitmapPlaneDescription, SoftwareBitmap},
        Windows::Media::Core::MediaSource,
        Windows::Media::Playback::MediaPlayer,
        Windows::Media::VideoFrame,
    );
}
