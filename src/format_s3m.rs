use byteorder::{LittleEndian, NativeEndian, ReadBytesExt};
use std::{
    io::{self, Read, SeekFrom},
    slice,
};
use anyhow::{Result, anyhow};

use crate::adlib::AdlibInstrument;

#[derive(Debug)]
pub struct S3MModule {
    // FILE STRUCTURE

    pub song_name: [u8;28],
    pub _unused: u32,
    pub order_amount: u16,
    pub sample_amount: u16,
    pub pattern_amount: u16,
    pub flags: u16,
    pub tracker_metadata: u16,
    pub ffi: u16,
    pub _scrm: u32,
    pub global_volume: u8,
    pub initial_speed: u8,
    pub initial_tempo: u8,
    pub mixing_volume: u8,
    pub ramping: u8,
    pub default_panning: u8,
    pub _unused2: [u8;8],
    pub special: u16,
    pub channel_settings: [u8;32],
    pub orders: Vec<u8>,

    pub sample_offsets: Vec<u16>,
    pub pattern_offsets: Vec<u16>,
    pub channel_panning: [u8;32],

    // PUBLIC
    pub instruments: Vec<S3MInstrument>,
    pub patterns: Vec<S3MPattern>,
}

#[derive(Debug)]
pub enum S3MInstrument {
    Sample(S3MSample),
    Adlib(S3MAdlibInstrument)
}

#[derive(Debug, Default, Clone)]
pub struct S3MSample {
    pub sample_type: u8,
    pub filename: [u8;12],
    pub memseg: [u8;3],
    pub length: u32,
    pub loop_begin: u32,
    pub loop_end: u32,
    pub volume: u8,
    pub _unused: u8,
    pub packed: u8,
    pub flags: u8,
    pub c4speed: u32,
    pub _unused2: u32,
    pub int_gp: u16,
    pub sample_name: [u8;28],
    pub _scrs: [u8;4],

    // Public
    pub audio: Vec<i16>,
}

#[derive(Debug, Default, Clone)]
pub struct S3MAdlibInstrument {
    pub instrument_type: u8,
    pub filename: [u8;12],
    pub _unused: [u8;3],
    pub d00: u8,
    pub d01: u8,
    pub d02: u8,
    pub d03: u8,
    pub d04: u8,
    pub d05: u8,
    pub d06: u8,
    pub d07: u8,
    pub d08: u8,
    pub d09: u8,
    pub d0a: u8,
    pub d0b: u8,
    pub volume: u8,
    pub disk: u8,
    pub _unused2: u16,
    pub c4freq: u32,
    pub _unused3: [u8;12],
    pub sample_name: [u8;28],
    pub _scri: [u8;4],
}

impl S3MAdlibInstrument {
    pub fn to_universal(&self) -> AdlibInstrument {
        let mut instrument = AdlibInstrument::default();

        instrument.modulator.freq_multiplier = self.d00 & 0xF;
        instrument.modulator.scale_envelope = self.d00 & 0x10 != 0;
        instrument.modulator.enable_sustain = self.d00 & 0x20 != 0;
        instrument.modulator.vibrato = self.d00 & 0x40 != 0;
        instrument.modulator.tremolo = self.d00 & 0x80 != 0;

        instrument.carrier.freq_multiplier = self.d01 & 0xF;
        instrument.carrier.scale_envelope = self.d01 & 0x10 != 0;
        instrument.carrier.enable_sustain = self.d01 & 0x20 != 0;
        instrument.carrier.vibrato = self.d01 & 0x40 != 0;
        instrument.carrier.tremolo = self.d01 & 0x80 != 0;

        instrument.modulator.scale_level = ((self.d02 & 0xC0) >> 6).reverse_bits();
        instrument.modulator.output_level = self.d02 & 0x3F;

        instrument.carrier.scale_level = ((self.d03 & 0xC0) >> 6).reverse_bits();
        instrument.carrier.output_level = self.d03 & 0x3F;

        instrument.modulator.attack = self.d04 >> 4;
        instrument.modulator.decay = self.d04 & 0xF;

        instrument.carrier.attack = self.d05 >> 4;
        instrument.carrier.decay = self.d05 & 0xF;

        instrument.modulator.sustain = self.d06 >> 4;
        instrument.modulator.release = self.d06 & 0xF;

        instrument.carrier.sustain = self.d07 >> 4;
        instrument.carrier.release = self.d07 & 0xF;

        instrument.modulator.wave = self.d08;
        instrument.carrier.wave = self.d09;

        instrument.modulator.feedback = self.d0a >> 1;
        instrument.modulator.connection = self.d0a & 1 != 0;

        instrument
    }
}

pub type S3MPattern = [S3MRow;64];

#[derive(Debug, Clone, Copy)]
pub struct S3MColumn {
    pub note: u8,
    pub instrument: u8,
    pub vol: u8,
    pub effect: u8,
    pub effect_value: u8,
}

impl Default for S3MColumn {
    fn default() -> Self {
        S3MColumn {
            note: 255,
            instrument: 0,
            vol: 255,
            effect: 0,
            effect_value: 0,
        }
    }
}

pub type S3MRow = [S3MColumn;32];

impl Default for S3MModule {
    fn default() -> Self {
        // Somebody please fix this monstrosity.
        S3MModule {
            song_name: [0;28],
            _unused: 0,
            order_amount: 0,
            sample_amount: 0,
            pattern_amount: 0,
            flags: 0,
            tracker_metadata: 0,
            ffi: 0,
            _scrm: 0,
            global_volume: 0,
            initial_speed: 0,
            initial_tempo: 0,
            mixing_volume: 0,
            ramping: 0,
            default_panning: 0,
            _unused2: [0;8],
            special: 0,
            channel_settings: [0;32],
            orders: Vec::new(),
            sample_offsets: Vec::new(),
            pattern_offsets: Vec::new(),
            channel_panning: [0;32],
            instruments: Vec::new(),
            patterns: Vec::new(),
        }
    }
}

impl S3MModule {
    pub fn load(mut reader: impl io::Read + io::Seek) -> Result<S3MModule> {
        let mut module = S3MModule::default();

        // HEADER START
        reader.read(&mut module.song_name).unwrap();
        module._unused = reader.read_u32::<LittleEndian>().unwrap();
        module.order_amount = reader.read_u16::<LittleEndian>().unwrap();
        module.sample_amount = reader.read_u16::<LittleEndian>().unwrap();
        module.pattern_amount = reader.read_u16::<LittleEndian>().unwrap();
        module.flags = reader.read_u16::<LittleEndian>().unwrap();
        module.tracker_metadata = reader.read_u16::<LittleEndian>().unwrap();
        module.ffi = reader.read_u16::<LittleEndian>().unwrap();
        module._scrm = reader.read_u32::<LittleEndian>().unwrap();
        if module._scrm != 0x4D524353 {
            return Err(anyhow!("File is not a valid module"))
        };
        module.global_volume = reader.read_u8().unwrap();
        module.initial_speed = reader.read_u8().unwrap();
        module.initial_tempo = reader.read_u8().unwrap();
        module.mixing_volume = reader.read_u8().unwrap();
        module.ramping = reader.read_u8().unwrap();
        module.default_panning = reader.read_u8().unwrap();
        reader.read(&mut module._unused2).unwrap();
        module.special = reader.read_u16::<LittleEndian>().unwrap();
        reader.read(&mut module.channel_settings).unwrap();
        module.orders.resize(module.order_amount as usize, 255);
        reader.read(&mut module.orders).unwrap();

        module.sample_offsets.resize(module.sample_amount as usize, 0);
        reader.read_u16_into::<LittleEndian>(&mut module.sample_offsets).unwrap();

        module.pattern_offsets.resize(module.pattern_amount as usize, 0);
        reader.read_u16_into::<LittleEndian>(&mut module.pattern_offsets).unwrap();

        reader.read(&mut module.channel_panning).unwrap();
        // HEADER END

        // SAMPLES START
        dbg!(module.sample_offsets.len());
        for offset in &module.sample_offsets {
            if *offset == 0 {
                module.instruments.push(S3MInstrument::Sample(S3MSample::default()));
                continue;
            }

            reader.seek(SeekFrom::Start((*offset as u64) << 4)).unwrap();
            let mut sample: S3MSample = S3MSample::default();

            sample.sample_type = reader.read_u8().unwrap();
            // if sample.sample_type > 1 {
            //     return Err(anyhow!("Adlib module detected"))
            // }
            if sample.sample_type == 0 {
                module.instruments.push(S3MInstrument::Sample(S3MSample::default()));
            } else if sample.sample_type == 1 {
                // PCM sample
                reader.read(&mut sample.filename).unwrap();
                reader.read(&mut sample.memseg).unwrap();
                sample.length = reader.read_u32::<LittleEndian>().unwrap();
                sample.loop_begin = reader.read_u32::<LittleEndian>().unwrap();
                sample.loop_end = reader.read_u32::<LittleEndian>().unwrap();
                sample.volume = reader.read_u8().unwrap();
                sample._unused = reader.read_u8().unwrap();
                sample.packed = reader.read_u8().unwrap();
                if sample.packed == 1 {
                    return Err(anyhow!("Compressed samples detected"))
                }
                sample.flags = reader.read_u8().unwrap();
                sample.c4speed = reader.read_u32::<LittleEndian>().unwrap();
                reader.seek(SeekFrom::Current(4)).unwrap();
                sample.int_gp = reader.read_u16::<LittleEndian>().unwrap();
                reader.seek(SeekFrom::Current(6)).unwrap();
                reader.read(&mut sample.sample_name).unwrap();

                let sampledata_offset: u32 =
                    ((sample.memseg[1] as u32) << 4) |
                    ((sample.memseg[2] as u32) << 12) |
                    ((sample.memseg[0] as u32) << 20);
                reader.seek(SeekFrom::Start(sampledata_offset as u64)).unwrap();

                if sample.flags & 0b100 != 0 {
                    // Sample is 16 bit
                    let mut data: Vec<u8> = Vec::with_capacity(sample.length as usize * 2);
                    data.resize((sample.length * 2).try_into().unwrap(), 0);
                    reader.read(&mut data).unwrap();

                    if module.ffi == 1 {
                        // Signed?
                        sample.audio = data
                            .chunks(2)
                            .map(|x| i16::from_le_bytes(x.try_into().unwrap()))
                            .collect();
                    } else {
                        sample.audio = data
                            .chunks(2)
                            .map(|x| (u16::from_le_bytes(x.try_into().unwrap()) ^ 0x8000) as i16)
                            .collect();
                    }
                } else {
                    // Sample is 8 bit
                    let mut data: Vec<u8> = Vec::with_capacity(sample.length as usize);
                    data.resize((sample.length).try_into().unwrap(), 0);
                    reader.read(&mut data).unwrap();

                    if module.ffi == 1 {
                        // Signed?
                        sample.audio = data
                            .iter()
                            .map(|x| i8::from_ne_bytes([*x]) as i16 * 256)
                            .collect();
                    } else {
                        sample.audio = data.iter().map(|x| (*x as i16 - 128) * 256).collect();
                    }
                }

                module.instruments.push(S3MInstrument::Sample(sample));
            } else if sample.sample_type >= 2 {
                // Adlib instrument
                let mut instrument = S3MAdlibInstrument::default();
                instrument.instrument_type = sample.sample_type;
                reader.read(&mut instrument.filename).unwrap();
                reader.read(&mut instrument._unused).unwrap();
                instrument.d00 = reader.read_u8().unwrap();
                instrument.d01 = reader.read_u8().unwrap();
                instrument.d02 = reader.read_u8().unwrap();
                instrument.d03 = reader.read_u8().unwrap();
                instrument.d06 = reader.read_u8().unwrap();
                instrument.d05 = reader.read_u8().unwrap();
                instrument.d06 = reader.read_u8().unwrap();
                instrument.d07 = reader.read_u8().unwrap();
                instrument.d08 = reader.read_u8().unwrap();
                instrument.d09 = reader.read_u8().unwrap();
                instrument.d0a = reader.read_u8().unwrap();
                instrument.d0b = reader.read_u8().unwrap();
                instrument.volume = reader.read_u8().unwrap();
                instrument.disk = reader.read_u8().unwrap();
                instrument._unused2 = reader.read_u16::<LittleEndian>().unwrap();
                instrument.c4freq = reader.read_u32::<LittleEndian>().unwrap();
                reader.read(&mut instrument._unused3).unwrap();
                reader.read(&mut instrument.sample_name).unwrap();
                reader.read(&mut instrument._scri).unwrap();

                module.instruments.push(S3MInstrument::Adlib(instrument));
            }
        }
        // SAMPLES END

        // PATTERNS START
        for offset in &module.pattern_offsets {
            if *offset == 0 {
                module.patterns.push([S3MRow::default();64]);
                continue;
            }

            // println!("Offset: {}", offset);
            reader.seek(SeekFrom::Start(((*offset as u64) << 4) + 2)).unwrap();
            let mut pattern = [S3MRow::default();64];

            let mut row = 0usize;
            let mut channel;
            'unpacking: loop {
                let packed_byte = reader.read_u8().unwrap();
                if packed_byte == 0 {
                    row += 1;
                }
                channel = (packed_byte & 31) as usize;
                if packed_byte & 32 != 0 { // note and instrument in the next 2 bytes
                    pattern[row][channel].note = reader.read_u8().unwrap();
                    pattern[row][channel].instrument = reader.read_u8().unwrap();
                }
                if packed_byte & 64 != 0 { // volume in the next byte
                    pattern[row][channel].vol = reader.read_u8().unwrap();
                }
                if packed_byte & 128 != 0 { // effect in the next 2 bytes
                    pattern[row][channel].effect = reader.read_u8().unwrap();
                    pattern[row][channel].effect_value = reader.read_u8().unwrap();
                }
                if row == 64 {
                    module.patterns.push(pattern);
                    break 'unpacking;
                }
            }

        }

        Ok(module)
    }
}