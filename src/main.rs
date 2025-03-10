use std::borrow::Cow;

use midly::{MetaMessage, Timing, TrackEventKind};
use osu_rs::{Beatmap, SoundTypes, TimingPointFlags};

const OFFSET: f64 = -26f64;
const MAX_POINTS_PER_BEAT: u64 = 1;
const BEATS_PER_BAR: u64 = 4;
const META: osu_rs::Metadata = osu_rs::Metadata {
    title: Cow::Borrowed("<title>"),
    title_unicode: Cow::Borrowed(""),
    artist: Cow::Borrowed("<artist>"),
    artist_unicode: Cow::Borrowed(""),
    creator: Cow::Borrowed("<your username>"),
    version: Cow::Borrowed("Timing"),
    source: Cow::Borrowed(""),
    tags: Cow::Borrowed(""),
    beatmap_id: 0,
    beatmap_set_id: -1,
};
const INPUT: &str = "input.mid";
const OUTPUT: &str = "out.osu";

fn main() {
    let data = std::fs::read(INPUT).unwrap();
    let smf = midly::Smf::parse(&data).unwrap();
    let mut us = 0.0f64;
    let Timing::Metrical(ticks_per_beat) = smf.header.timing else {
        panic!();
    };
    let ticks_per_beat = u64::from(u16::from(ticks_per_beat));
    #[allow(clippy::modulo_one)]
    {
        assert!(ticks_per_beat % MAX_POINTS_PER_BEAT == 0);
    }
    let mut us_per_beat = 0u64;
    let mut beatmap = Beatmap {
        context: osu_rs::Context { version: 14 },
        general: osu_rs::General {
            audio_filename: "audio.mp3".into(),
            audio_lead_in: 0,
            audio_hash: "".into(),
            preview_time: 0,
            countdown: osu_rs::Countdown::None,
            sample_set: osu_rs::SampleSet::Normal,
            stack_leniency: 0.7,
            mode: osu_rs::GameMode::Taiko,
            letterbox_in_breaks: false,
            story_fire_in_front: false,
            use_skin_sprites: false,
            always_show_playfield: false,
            custom_samples: false,
            overlay_position: osu_rs::OverlayPosition::NoChange,
            skin_preference: "".into(),
            epilepsy_warning: false,
            countdown_offset: 0,
            special_style: false,
            widescreen_storyboard: true,
            samples_match_playback_rate: false,
        },
        difficulty: osu_rs::Difficulty {
            hp_drain_rate: 5.0,
            circle_size: 5.0,
            overall_difficulty: 5.0,
            approach_rate: 5.0,
            slider_multiplier: 1.4,
            slider_tick_rate: 1.0,
        },
        colours: osu_rs::Colours { colours: vec![] },
        editor: osu_rs::Editor {
            bookmarks: vec![],
            distance_spacing: 3.1,
            beat_divisor: 4,
            grid_size: 32,
            timeline_zoom: 3.5,
        },
        events: osu_rs::Events { events: vec![] },
        hit_objects: vec![],
        metadata: META,
        timing_points: vec![],
        variables: osu_rs::Variables { variables: vec![] },
    };
    let mut t = 0u64;
    let mut added0 = false;
    for msg in smf.tracks.get(1).unwrap() {
        let d = u32::from(msg.delta) as u64;
        let new_bars =
            (t + d) / (ticks_per_beat * BEATS_PER_BAR) - t / (ticks_per_beat * BEATS_PER_BAR);
        // add a barline for previous timing if
        // case 1: this timing point is in a new bar but it doesn't align with the new bar's barline
        // case 2: this is over 1 bar away from the previous point and the previous point wasn't at
        //         a barline (if its exactly one bar away it just means the previous point was
        //         mid-barline, but any more means the previous point created some barlines that
        //         were misaligned)
        if (new_bars > 0 && (t + d) % (ticks_per_beat * BEATS_PER_BAR) != 0)
            || (new_bars > 1 && t % (ticks_per_beat * BEATS_PER_BAR) != 0)
        {
            let d = (ticks_per_beat * BEATS_PER_BAR) - (t % (ticks_per_beat * BEATS_PER_BAR));
            let us = us + (d * us_per_beat) as f64 / ticks_per_beat as f64;
            let t = t + d;
            assert!(t % (ticks_per_beat * BEATS_PER_BAR) == 0);
            beatmap.timing_points.push(osu_rs::TimingPoint {
                offset: osu_rs::Time((us / 1000.0).floor()),
                beat_length: us_per_beat as f64 / 1000.0,
                time_signature: BEATS_PER_BAR as i32,
                sample_set: None,
                custom_sample_index: 0,
                sample_volume: 100,
                changes_timing: true,
                flags: TimingPointFlags::default(),
            });
        }
        t += d;
        us += (d * us_per_beat) as f64 / ticks_per_beat as f64;
        let TrackEventKind::Meta(meta) = msg.kind else {
            continue;
        };
        let MetaMessage::Tempo(us_per_beat1) = meta else {
            continue;
        };
        // only add one point at the start (to work around some quirks)
        if t == 0 {
            if added0 {
                continue;
            }
            added0 = true;
        }
        us_per_beat = u64::from(u32::from(us_per_beat1));
        beatmap.timing_points.push(osu_rs::TimingPoint {
            offset: osu_rs::Time((us / 1000.0).floor()),
            beat_length: us_per_beat as f64 / 1000.0,
            time_signature: BEATS_PER_BAR as i32,
            sample_set: None,
            custom_sample_index: 0,
            sample_volume: 100,
            changes_timing: true,
            flags: if t % (ticks_per_beat * BEATS_PER_BAR) == 0 {
                TimingPointFlags::default()
            } else if t % (ticks_per_beat / MAX_POINTS_PER_BEAT) == 0 {
                // use kiai as a marker of timing points we definitely want to keep
                TimingPointFlags::KIAI | TimingPointFlags::OMIT_FIRST_BARLINE
            } else {
                TimingPointFlags::OMIT_FIRST_BARLINE
            },
        });
    }
    let points = beatmap
        .timing_points
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            let i = *i;
            if i == 0 {
                return true;
            }
            if i == beatmap.timing_points.len() - 1 {
                return true;
            }
            let prev = &beatmap.timing_points[i - 1];
            let next = &beatmap.timing_points[i + 1];
            if p.flags.contains(TimingPointFlags::OMIT_FIRST_BARLINE) {
                if p.flags.contains(TimingPointFlags::KIAI) {
                    true
                } else {
                    (next.offset.0 - p.offset.0).max(p.offset.0 - prev.offset.0) > 1000.0
                }
            } else {
                true
            }
        })
        .map(|(_i, p)| p.clone())
        .collect::<Vec<_>>();
    beatmap.timing_points = points;
    for tp in &mut beatmap.timing_points {
        tp.offset.0 += OFFSET;
        let kat = tp.flags.is_empty();
        let add_obj = kat || tp.flags.contains(TimingPointFlags::KIAI);
        tp.flags.remove(TimingPointFlags::KIAI);
        if add_obj {
            beatmap.hit_objects.push(osu_rs::HitObject {
                x: 64,
                y: 64,
                time: tp.offset.0 as i32,
                combo_start: false,
                combo_colour_skip: 0,
                hit_sound: osu_rs::FullHitSound::default(),
                kind: osu_rs::HitObjectKind::Circle,
            });
            if kat {
                beatmap
                    .hit_objects
                    .last_mut()
                    .unwrap()
                    .hit_sound
                    .hit_sound
                    .sounds
                    .insert(SoundTypes::CLAP);
            }
        }
    }
    beatmap
        .serialize(std::fs::File::create(OUTPUT).unwrap())
        .unwrap();
}
