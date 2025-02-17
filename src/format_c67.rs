use serde_big_array::BigArray;

/* stupid serde bullshit */
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Plist {
    #[serde(with = "BigArray")]
    pub list: [u32; 128]
}

impl Default for Plist {
    fn default() -> Self {
        Plist {
            list: [0;128],
        }
    }
}


#[derive(Debug)]
#[repr(C)]
pub struct C67ModuleHeader {
    pub speed: u8, //mztempo
    pub loop_order: u8, //mzloop
    pub instrument_filenames: [u8;13*32], //dinsnames
    pub instrument_meta: [C67SampleMetadata;32], //dinsbase
    pub adlib_instrument_filenames: [u8;13*32], //fminsnames
    pub adlib_instrument_meta: [C67FMRegisters;32], //fminsbase
    pub playlist: [u8;256], //ordbase
    pub pattern_pointers: Plist, //patoffbase
    pub pattern_lengths: Plist, //patlenbase
}

impl Default for C67ModuleHeader {
    fn default() -> Self {
        Self {
            speed: Default::default(),
            loop_order: Default::default(),
            instrument_filenames: [0;13*32],
            instrument_meta: Default::default(),
            adlib_instrument_filenames: [0;13*32],
            adlib_instrument_meta: Default::default(),
            playlist: [0;256],
            pattern_pointers: Plist::default(),
            pattern_lengths: Plist::default(),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct C67Module {
    pub header: C67ModuleHeader,
    pub pattern_data: Vec<u8>,
    pub sample_data: Vec<u8>,
}

impl Default for C67Module {
    fn default() -> Self {
        Self {
            header: Default::default(),
            pattern_data: Default::default(),
            sample_data: Default::default()
        }
    }
}

impl C67Module {
    pub fn serialize(&self) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();

        data.push(self.header.speed);
        //dbg!("{}", data.len());
        data.push(self.header.loop_order);
        //dbg!("{}", data.len());
        data.extend_from_slice(&self.header.instrument_filenames);
        //dbg!("{}", data.len());
        data.append(&mut bincode::serialize(&self.header.instrument_meta).unwrap());
        //dbg!("{}", data.len());
        data.extend_from_slice(&self.header.adlib_instrument_filenames);
        //dbg!("{}", data.len());
        data.append(&mut bincode::serialize(&self.header.adlib_instrument_meta).unwrap());
        //dbg!("{}", data.len());
        data.extend_from_slice(&self.header.playlist);
        //dbg!("{}", data.len());
        data.extend_from_slice(&mut bincode::serialize(&self.header.pattern_pointers).unwrap());
        //dbg!("{}", data.len());
        data.extend_from_slice(&mut bincode::serialize(&self.header.pattern_lengths).unwrap());
        //dbg!("{}", data.len());
        data.extend_from_slice(&self.pattern_data);
        data.extend_from_slice(&self.sample_data);

        data
    }
}

#[derive(Debug, serde::Serialize)]
#[repr(C)]
pub struct C67SampleMetadata {
    pub _unused: u32,
    pub sample_length: u32,
    pub loop_start: u32,
    pub loop_end: u32,
}

impl Default for C67SampleMetadata {
    fn default() -> Self {
        Self {
            _unused: 0,
            sample_length: 0,
            loop_start: 0,
            loop_end: 0xFFFFF
        }
    }
}

#[derive(Debug, Default, serde::Serialize)]
#[repr(C)]
pub struct C67FMRegisters {
    pub feedback_connection: u8,
    pub modulator_characteristics: u8,
    pub modulator_scale_and_output_level: u8,
    pub modulator_attack_decay_level: u8,
    pub modulator_sustain_release_level: u8,
    pub modulator_wave_select: u8,
    pub carrier_characteristics: u8,
    pub carrier_scale_and_output_level: u8,
    pub carrier_attack_decay_level: u8,
    pub carrier_sustain_release_level: u8,
    pub carrier_wave_select: u8,
}

#[derive(Debug)]
pub enum C67PatternCommand {
    PlayNote(PlayNoteCommand),
    SetVolume(SetVolumeCommand),
    Delay(u8),
    End,
}

impl C67PatternCommand {
    pub fn serialize(&self) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();

        match self {
            C67PatternCommand::PlayNote(command) => {
                match command.channel {
                    Channel::PCM(num) => data.push(num),
                    Channel::FM(num) => data.push(4+num),
                }

                let mut byte1 = ((command.instrument >> 5) & 1) << 7;
                byte1 |= (command.octave & 7) << 4;
                byte1 |= command.note & 0xF;

                let mut byte2 = (command.instrument & 0xF) << 4;
                byte2 |= command.volume & 0xF;

                data.push(byte1);
                data.push(byte2);
            },
            C67PatternCommand::SetVolume(command) => {
                match command.channel {
                    Channel::PCM(num) => data.push(0x20+num),
                    Channel::FM(num) => data.push(0x24+num),
                }

                data.push(command.volume & 0xF);
            },
            C67PatternCommand::Delay(rows) => {
                data.push(0x40);
                data.push(*rows);
            },
            C67PatternCommand::End => {
                data.push(0x60);
            },
        }

        //dbg!(&data);

        data
    }
}

#[derive(Debug)]
pub enum Channel {
    PCM(u8),
    FM(u8)
}

#[derive(Debug)]
pub struct PlayNoteCommand {
    pub channel: Channel,
    pub octave: u8,
    pub note: u8,
    pub instrument: u8,
    pub volume: u8,
}

#[derive(Debug)]
pub struct SetVolumeCommand {
    pub channel: Channel,
    pub volume: u8,
}

pub fn serialize_pattern(commands: &[C67PatternCommand]) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::new();

    for c in commands {
        data.append(&mut c.serialize());
    }

    data
}