use super::video_frame::new_video_frame_buffer;
use crate::video_frame::{BoxVideoFrame, VideoFrame};
use crate::video_track::RtcVideoTrack;
use cxx::{SharedPtr, UniquePtr};
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use webrtc_sys::video_track as sys_vt;

pub struct NativeVideoStream {
    native_sink: SharedPtr<sys_vt::ffi::NativeVideoSink>,
    video_track: RtcVideoTrack,
    frame_rx: mpsc::UnboundedReceiver<BoxVideoFrame>,
}

impl NativeVideoStream {
    pub fn new(video_track: RtcVideoTrack) -> Self {
        let (frame_tx, frame_rx) = mpsc::unbounded_channel();
        let observer = Arc::new(VideoTrackObserver { frame_tx });
        let native_sink = sys_vt::ffi::new_native_video_sink(Box::new(
            sys_vt::VideoSinkWrapper::new(observer.clone()),
        ));

        let video = unsafe { sys_vt::ffi::media_to_video(video_track.sys_handle()) };
        video.add_sink(&native_sink);

        Self {
            native_sink,
            video_track,
            frame_rx,
        }
    }

    pub fn track(&self) -> RtcVideoTrack {
        self.video_track.clone()
    }

    pub fn close(&mut self) {
        let video = unsafe { sys_vt::ffi::media_to_video(self.video_track.sys_handle()) };
        video.remove_sink(&self.native_sink);

        self.frame_rx.close();
    }
}

impl Drop for NativeVideoStream {
    fn drop(&mut self) {
        self.close();
    }
}

impl Stream for NativeVideoStream {
    type Item = BoxVideoFrame;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        self.frame_rx.poll_recv(cx)
    }
}

struct VideoTrackObserver {
    frame_tx: mpsc::UnboundedSender<BoxVideoFrame>,
}

impl sys_vt::VideoSink for VideoTrackObserver {
    fn on_frame(&self, frame: UniquePtr<webrtc_sys::video_frame::ffi::VideoFrame>) {
        let _ = self.frame_tx.send(VideoFrame {
            rotation: frame.rotation().into(),
            timestamp_us: frame.timestamp_us(),
            buffer: new_video_frame_buffer(unsafe { frame.video_frame_buffer() }),
        });
    }

    fn on_discarded_frame(&self) {}

    fn on_constraints_changed(&self, _constraints: sys_vt::ffi::VideoTrackSourceConstraints) {}
}
