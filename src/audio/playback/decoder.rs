use crossbeam_channel::{Receiver, Sender};
use symphonia::{
    core::{
        audio::{AudioBufferRef, Signal},
        codecs::CODEC_TYPE_NULL,
        io::{MediaSourceStream, MediaSourceStreamOptions},
        probe::Hint,
    },
    default::{get_codecs, get_probe},
};

use crate::{event::events::ControlSignal, stream::streamer::AudioStreamer};

pub fn decode_audio(
    buffer: AudioStreamer,
    channels: usize,
    codec: String,
    tx: Sender<f32>,
    control_rx: Receiver<ControlSignal>,
) -> anyhow::Result<()> {
    let mss = MediaSourceStream::new(
        Box::new(buffer),
        MediaSourceStreamOptions {
            buffer_len: 128 * 1024,
        },
    );

    let mut hint = Hint::new();
    hint.with_extension(&codec);

    let probed = get_probe().format(
        &hint,
        mss,
        &Default::default(),
        &Default::default(),
    )?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(anyhow::Error::msg("No tracks"))?;

    let mut decoder =
        get_codecs().make(&track.codec_params, &Default::default())?;
    let track_id = track.id;

    while let Ok(packet) = format.next_packet() {
        #[allow(clippy::single_match)]
        match control_rx.try_recv() {
            Ok(ControlSignal::Stop) => {
                break;
            }
            _ => {}
        }

        if packet.track_id() != track_id {
            continue;
        }

        while !format.metadata().is_latest() {
            format.metadata().pop();
        }

        match decoder.decode(&packet) {
            Ok(decoded) => match decoded {
                AudioBufferRef::F32(ref buf) => {
                    let frames = buf.frames();
                    for frame in 0..frames {
                        for channel in 0..channels {
                            let sample = buf.chan(channel)[frame];
                            tx.send(sample)?;
                        }
                    }
                }
                _ => {
                    eprintln!("Unsupported sample format.");
                }
            },
            Err(e) => {
                eprintln!("{:?}", e);
            }
        }
    }

    Ok(())
}
