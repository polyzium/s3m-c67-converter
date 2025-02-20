use std::{array, collections::HashMap};

use crate::{format_c67::{self, serialize_pattern, C67FMRegisters, C67Module, C67SampleMetadata, Channel, PlayNoteCommand, SetVolumeCommand}, format_s3m::{S3MAdlibInstrument, S3MInstrument, S3MModule, S3MPattern, S3MSample}};

pub struct Converter<'m> {
    module: &'m S3MModule,
    pcm_channel_remap_table: HashMap<u8, u8>,
    pcm_instrument_remap_table: HashMap<u8, u8>,
    adlib_instrument_remap_table: HashMap<u8, u8>,

    pcm_instruments: Vec<S3MSample>,
    adlib_instruments: Vec<S3MAdlibInstrument>
}

impl<'a> Converter<'a> {
    pub fn new(module: &'a S3MModule) -> Self {
        // PCM channel remap table
        let mut pcm_channel_remap_table: HashMap<u8, u8> = HashMap::new();
        let mut pcm_channel_remap_index = 0u8;
        for (index, channel_setting) in module.channel_settings.iter().enumerate() {
            if channel_setting & 0x80 != 1 {
                // Ignore muted channels
                continue;
            }

            if channel_setting & 0x7F <= 15 {
                pcm_channel_remap_table.insert(index as u8, pcm_channel_remap_index);
                pcm_channel_remap_index += 1;
            }

            if pcm_channel_remap_index >= 4 {
                println!("More than 4 PCM channels detected, discarding the rest");
                break;
            }
        }

        // PCM+AdLib instrument remap table
        let mut pcm_instrument_remap_table: HashMap<u8, u8> = HashMap::new();
        let mut pcm_instrument_remap_index = 0u8;
        let mut adlib_instrument_remap_table: HashMap<u8, u8> = HashMap::new();
        let mut adlib_instrument_remap_index = 0u8;
        let mut pcm_instruments: Vec<S3MSample> = Vec::new();
        let mut adlib_instruments: Vec<S3MAdlibInstrument> = Vec::new();
        for (index, instrument) in module.instruments.iter().enumerate() {
            if pcm_instrument_remap_index >= 32 {
                println!("More than 32 PCM instruments detected, discarding");
                continue;
            }

            if adlib_instrument_remap_index >= 32 {
                println!("More than 32 AdLib instruments detected, discarding");
                continue;
            }

            match instrument {
                S3MInstrument::Sample(sample) => {
                    pcm_instruments.push(sample.clone());
                    pcm_instrument_remap_table.insert(index as u8, pcm_instrument_remap_index);
                    pcm_instrument_remap_index += 1;
                },
                S3MInstrument::Adlib(instrument) => {
                    adlib_instruments.push(instrument.clone());
                    adlib_instrument_remap_table.insert(index as u8, adlib_instrument_remap_index);
                    adlib_instrument_remap_index += 1;
                },
            }
        }

        Self {
            module,
            pcm_channel_remap_table,
            pcm_instrument_remap_table,
            adlib_instrument_remap_table,
            pcm_instruments,
            adlib_instruments,
        }
    }

    pub fn convert(&self) -> C67Module {
        let mut module = C67Module::default();

        module.header.speed = self.module.initial_speed;
        module.header.loop_order = 0;

        // Instrument filenames
        let mut pcm_instrument_filenames = [0u8;13*32];
        let mut adlib_instrument_filenames = [0u8;13*32];
        for (index, sample) in self.pcm_instruments.iter().enumerate() {
            let begin = index*13;
            let end = (index*13)+13;
            let filename = &mut pcm_instrument_filenames[begin..end];

            filename[0..12].copy_from_slice(&sample.filename);
            filename[12] = 0;
        }
        for (index, instrument) in self.adlib_instruments.iter().enumerate() {
            let begin = index*13;
            let end = (index*13)+13;
            let filename = &mut adlib_instrument_filenames[begin..end];

            filename[0..12].copy_from_slice(&instrument.filename);
            filename[12] = 0;
        }

        // Instrument metadata
        let mut pcm_instrument_meta: [C67SampleMetadata;32] = array::from_fn(|_| C67SampleMetadata::default());
        let mut adlib_instrument_meta: [C67FMRegisters;32] = array::from_fn(|_| C67FMRegisters::default());
        for (index, sample) in self.pcm_instruments.iter().enumerate() {
            let meta = &mut pcm_instrument_meta[index];
            if sample.flags & 1 != 0 {
                meta.loop_start = sample.loop_begin;
                meta.loop_end = sample.loop_end;
            }

            meta.sample_length = sample.audio.len() as u32;
        }
        for (index, instrument) in self.adlib_instruments.iter().enumerate() {
            let meta = &mut adlib_instrument_meta[index];

            meta.feedback_connection = instrument.d0a;

            meta.modulator_characteristics = instrument.d00;
            meta.modulator_scale_and_output_level = instrument.d02 & 0x3F;
            meta.modulator_scale_and_output_level |= (instrument.d02 >> 6).reverse_bits() << 6;
            meta.modulator_attack_decay_level = instrument.d04;
            meta.modulator_sustain_release_level = instrument.d06;
            meta.modulator_wave_select = instrument.d08;

            meta.carrier_characteristics = instrument.d01;
            meta.carrier_scale_and_output_level = instrument.d03 & 0x3F;
            meta.carrier_scale_and_output_level |= (instrument.d03 >> 6).reverse_bits() << 6;
            meta.carrier_attack_decay_level = instrument.d05;
            meta.carrier_sustain_release_level = instrument.d07;
            meta.carrier_wave_select = instrument.d09;
        }

        module.header.playlist.fill(0xFF);
        let mut order_index = 0usize;
        for order in &self.module.orders {
            if *order == 254 {
                // Ignore separators
                continue;
            }
            module.header.playlist[order_index] = *order;
            order_index += 1;
        }

        let mut pattern_data: Vec<u8> = Vec::new();
        let mut pattern_offsets = [0u32;128];
        let mut pattern_lengths = [0u32;128];
        let mut pattern_index = 0usize;
        for pattern in &self.module.patterns {
            let converted_pattern = self.convert_pattern(&pattern);
            let mut serialized_pattern = serialize_pattern(&converted_pattern);
            let serialized_pattern_length = serialized_pattern.len();
            pattern_data.append(&mut serialized_pattern);
            let offset = pattern_data.len()-serialized_pattern_length;
            pattern_lengths[pattern_index] = serialized_pattern_length as u32;
            pattern_offsets[pattern_index] = offset as u32;

            pattern_index += 1;
        }
        for index in pattern_index..128 {
            let mut serialized_pattern = serialize_pattern(&self.generate_empty_pattern());
            let serialized_pattern_length = serialized_pattern.len();
            pattern_data.append(&mut serialized_pattern);
            let offset = pattern_data.len()-serialized_pattern_length;
            pattern_lengths[index] = serialized_pattern_length as u32;
            pattern_offsets[index] = offset as u32;

        }

        for sample in &self.pcm_instruments {
            for v in &sample.audio {
                module.sample_data.push(((v/256) + 128) as u8);
            }
        }

        module.header.instrument_filenames = pcm_instrument_filenames;
        module.header.instrument_meta = pcm_instrument_meta;
        module.header.adlib_instrument_filenames = adlib_instrument_filenames;
        module.header.adlib_instrument_meta = adlib_instrument_meta;
        module.header.pattern_lengths = format_c67::Plist { list: pattern_lengths };
        module.header.pattern_pointers = format_c67::Plist { list: pattern_offsets };
        module.pattern_data = pattern_data;

        module
    }

    pub fn generate_empty_pattern(&self) -> Vec<format_c67::C67PatternCommand> {
        let mut commands: Vec<format_c67::C67PatternCommand> = Vec::new();
        commands.push(format_c67::C67PatternCommand::Delay(64));
        commands.push(format_c67::C67PatternCommand::End);
        commands
    }

    pub fn convert_pattern(&self, pattern: &S3MPattern) -> Vec<format_c67::C67PatternCommand> {
        let mut commands: Vec<format_c67::C67PatternCommand> = Vec::new();

        for (row_index, row) in pattern.iter().enumerate() {
            let mut saved_instrument: u8 = 0;
            if row_index == 63 {
                commands.push(format_c67::C67PatternCommand::Delay(1));
                commands.push(format_c67::C67PatternCommand::End);
                break;
            }

            for (channel_index, col) in row.iter().enumerate() {
                if col.instrument != 0 {saved_instrument = col.instrument;}
                if col.note < 254 {
                    let octave = col.note >> 4;
                    let pitch = col.note & 0xF;
                    // let actual_note = octave*12+pitch+12;

                    let channel: Channel;
                    let mut volume: u8;

                    match &self.module.instruments[(saved_instrument-1) as usize] {
                        S3MInstrument::Adlib(instrument) => {
                            volume = instrument.volume;
                            let channel_setting = self.module.channel_settings[channel_index] & 0x7F;
                            if channel_setting > 26 {
                                // Drum adlib channel
                                continue;
                            }
                            channel = Channel::FM(channel_setting-16);
                        }
                        S3MInstrument::Sample(sample) => {
                            volume = sample.volume;
                            let channel_setting = self.module.channel_settings[channel_index] & 0x7F;
                            let remapped_channel_num = self.pcm_channel_remap_table.get(&channel_setting);

                            let mut formatted_channel: String;
                            if channel_setting >= 8 {
                                formatted_channel = ((channel_setting - 8) + 1).to_string();
                                formatted_channel.push('R');
                            } else {
                                formatted_channel = (channel_setting + 1).to_string();
                                formatted_channel.push('L');
                            }

                            if remapped_channel_num.is_none() {
                                println!("Discarding note in channel {} as it is not mapped", formatted_channel);
                                continue;
                            }
                            channel = Channel::PCM(*self.pcm_channel_remap_table.get(&channel_setting).unwrap());
                        },
                    }

                    if col.vol <= 64 {
                        volume = col.vol;
                    }

                    let instrument: u8;
                    match &self.module.instruments[(saved_instrument as usize)-1] {
                        S3MInstrument::Sample(_) => {
                            let remapped_instrument = self.pcm_instrument_remap_table.get(&(saved_instrument-1));
                            if remapped_instrument.is_none() {
                                println!("Discarding note with instrument {} as it is not mapped", saved_instrument);
                                continue;
                            }
                            instrument = *remapped_instrument.unwrap();
                        },
                        S3MInstrument::Adlib(_) => {
                            let remapped_instrument = self.adlib_instrument_remap_table.get(&(saved_instrument-1));
                            if remapped_instrument.is_none() {
                                println!("Discarding note with instrument {} as it is not mapped", saved_instrument);
                                continue;
                            }
                            instrument = *remapped_instrument.unwrap();
                        },
                    }

                    commands.push(format_c67::C67PatternCommand::PlayNote(PlayNoteCommand {
                        channel,
                        octave: octave & 7,
                        note: pitch,
                        instrument,
                        volume: (volume/4).clamp(0, 15),
                    }));
                } else if col.note == 254 {
                    // added even though volume 0 is not silent :)
                    let channel: Channel;
                    let channel_setting = self.module.channel_settings[channel_index] & 0x7F;
                    if channel_setting <= 15 { // is PCM channel
                        channel = Channel::PCM(*self.pcm_channel_remap_table.get(&channel_setting).unwrap());
                    } else {
                        channel = Channel::FM(channel_setting-16);
                    }

                    commands.push(format_c67::C67PatternCommand::SetVolume(SetVolumeCommand {
                        channel,
                        volume: 0,
                    }));
                } else if col.vol <= 64 {
                    let channel: Channel;
                    let channel_setting = self.module.channel_settings[channel_index] & 0x7F;
                    if channel_setting <= 15 { // is PCM channel
                        channel = Channel::PCM(*self.pcm_channel_remap_table.get(&channel_setting).unwrap());
                    } else {
                        channel = Channel::FM(channel_setting-16);
                    }

                    commands.push(format_c67::C67PatternCommand::SetVolume(SetVolumeCommand {
                        channel,
                        volume: (col.vol/4).clamp(0, 15),
                    }));
                }
            }

            commands.push(format_c67::C67PatternCommand::Delay(1));
            
        }

        //dbg!(&commands);

        commands
    }
}