use std::{sync::Arc, time::Duration};

use flume::{Receiver, Sender};
use symphonia::{
    core::{
        audio::{AudioBufferRef, Signal},
        codecs::CODEC_TYPE_NULL,
        formats::{FormatOptions, SeekMode, SeekTo},
        io::{MediaSourceStream, MediaSourceStreamOptions},
        probe::Hint,
    },
    default::{get_codecs, get_probe},
};
use tracing::info;

use crate::{
    audio::progress::TrackProgress, event::events::ControlSignal,
    stream::streamer::AudioStreamer,
};

pub fn decode_audio(
    buffer: AudioStreamer,
    channels: usize,
    codec: String,
    tx: Sender<(f32, i64)>,
    control_rx: Receiver<ControlSignal>,
    stopper_tx: Sender<bool>,
    track_progress: Arc<TrackProgress>,
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
        &FormatOptions {
            enable_gapless: true,
            ..Default::default()
        },
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

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let fade_samples = (sample_rate / 4) as usize;
    let total_frames = track.codec_params.n_frames.map(|f| f as usize);
    // let fade_start_frame = total_frames.map(|f| f.saturating_sub(fade_samples));
    let mut current_frames = 0;
    let mut fade_out = None;

    let total_duration = Duration::from_secs_f64(
        total_frames.unwrap_or(0) as f64 / sample_rate as f64,
    );
    track_progress.set_total_duration(total_duration);

    while let Ok(packet) = format.next_packet() {
        #[allow(clippy::single_match)]
        match control_rx.try_recv() {
            Ok(ControlSignal::Stop) => {
                info!("Received stop signal. Stopping without sending events.");
                // stopper_tx.send(true).unwrap();
                // while control_rx.try_recv().is_ok() {} // may be redundant
                fade_out = Some(fade_samples);
                // return Ok(());
            }
            Ok(ControlSignal::Seek(pos)) => {
                format
                    .seek(
                        SeekMode::Coarse,
                        SeekTo::TimeStamp { ts: pos, track_id },
                    )
                    .unwrap();
            }
            Ok(ControlSignal::SeekForward(pos)) => {
                info!("Seeking forward by {} * 1000; ts = {}", pos, packet.ts);
                format
                    .seek(
                        SeekMode::Coarse,
                        SeekTo::TimeStamp {
                            ts: packet.ts + pos * 1000,
                            track_id,
                        },
                    )
                    .unwrap();
            }
            Ok(ControlSignal::SeekBackward(pos)) => {
                format
                    .seek(
                        SeekMode::Coarse,
                        SeekTo::TimeStamp {
                            ts: packet.ts.saturating_sub(pos * 1000),
                            track_id,
                        },
                    )
                    .unwrap();
            }
            _ => {}
        }

        if packet.track_id() != track_id {
            continue;
        }

        while !format.metadata().is_latest() {
            format.metadata().pop();
        }

        let current_time =
            Duration::from_secs_f64(packet.ts as f64 / sample_rate as f64);
        track_progress.set_current_position(current_time);

        let ts = packet.ts as i64;
        match decoder.decode(&packet) {
            Ok(decoded) => match decoded {
                AudioBufferRef::F32(ref buf) => {
                    // let frames = buf.frames();
                    // for frame in 0..frames {
                    //     for channel in 0..channels {
                    //         let sample = buf.chan(channel)[frame];
                    //         tx.send(sample).unwrap();
                    //     }
                    // }
                    let frames = buf.frames();
                    // current_frames += frames;

                    // if let Some(start_frame) = fade_start_frame {
                    //     if current_frames > start_frame {
                    //         info!("Reached exact frame. Stopping.");
                    //         fade_out = Some(fade_samples);
                    //     }
                    // }

                    for frame in 0..frames {
                        current_frames += 1;
                        if let Some(ref mut remaining_samples) = fade_out {
                            if *remaining_samples == 0 {
                                info!("Fade out complete. Stopping.");
                                stopper_tx.send(true).unwrap();
                                return Ok(());
                            }
                            let fade_factor = (*remaining_samples as f32
                                / fade_samples as f32)
                                .max(0.0);
                            *remaining_samples -= 1;

                            // info!("Fade factor: {}", fade_factor);

                            for channel in 0..channels {
                                let sample =
                                    buf.chan(channel)[frame] * fade_factor;
                                tx.send((sample, {
                                    if current_frames - 1 == frame {
                                        ts
                                    } else {
                                        0
                                    }
                                }))
                                .unwrap();
                            }
                        } else {
                            for channel in 0..channels {
                                let sample = buf.chan(channel)[frame];
                                let _ = tx.send((sample, {
                                    if current_frames - 1 == frame {
                                        ts
                                    } else {
                                        0
                                    }
                                }));
                            }
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

    info!("Stopping decoder. Sending stop event.");
    stopper_tx.send(false).unwrap();

    Ok(())
}
