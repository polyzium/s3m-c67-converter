#[derive(Default, Debug, Clone, Copy)]
pub struct AdlibInstrument {
    pub modulator: AdlibParams,
    pub carrier: AdlibParams,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct AdlibParams {
    pub freq_multiplier: u8,
    pub scale_envelope: bool,
    pub enable_sustain: bool,
    pub vibrato: bool,
    pub tremolo: bool,
    pub scale_level: u8,
    pub output_level: u8,
    pub attack: u8,
    pub decay: u8,
    pub sustain: u8,
    pub release: u8,
    pub wave: u8,
    // Below are used for modulator only
    pub feedback: u8,
    pub connection: bool
}