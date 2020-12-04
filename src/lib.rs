#[macro_use]
extern crate vst;
extern crate log;

use vst::buffer::AudioBuffer;
use vst::plugin::{Category, Info, Plugin, PluginParameters};
use vst::util::AtomicFloat;

use std::collections::VecDeque;
use std::sync::Arc;

use log::info;

use rand_xoshiro::rand_core::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use rand_xoshiro::rand_core::RngCore;

mod compute;

struct Effect {
    // Store a handle to the plugin's parameter object.
    params: Arc<EffectParameters>,

    // FIXME: refactor these variables so that they have consistent and meaningful
    // names
    l_buffs_div: Vec<VecDeque<f32>>,
    r_buffs_div: Vec<VecDeque<f32>>,
    aux_l_1: VecDeque<f32>,
    aux_r_1: VecDeque<f32>,
    yl: VecDeque<f32>,
    yr: VecDeque<f32>,
    x_s_p: f32,
    xl_p: f32,
    xr_p: f32,
    count: usize,
    sr: f32,
    rng: Xoshiro256Plus,
}

struct EffectParameters {
    // The plugin's state consists of a single parameter: amplitude.
    div: AtomicFloat,
    bias: AtomicFloat,
    chaos: AtomicFloat,
    perturb: AtomicFloat,
    sensing_lp: AtomicFloat,
}

impl Default for Effect {
    fn default() -> Effect {
        Effect {
            params: Arc::new(EffectParameters::default()),
            l_buffs_div: vec![VecDeque::with_capacity(1100);12],
            r_buffs_div: vec![VecDeque::with_capacity(1100);12],
            aux_l_1: VecDeque::with_capacity(1100),
            aux_r_1: VecDeque::with_capacity(1100),
            yl: VecDeque::with_capacity(1100),
            yr: VecDeque::with_capacity(1100),
            x_s_p: 0.0,
            xl_p: 0.0,
            xr_p: 0.0,
            count: 0,
            sr: 44100.0,
            rng: Xoshiro256Plus::seed_from_u64(69_420),
        }
    }
}

impl Default for EffectParameters {
    fn default() -> EffectParameters {
        EffectParameters {
            div: AtomicFloat::new(0.0),     
            bias: AtomicFloat::new(0.5),     // adds a constant to sensing threshold
            chaos: AtomicFloat::new(0.0),   // randomly flips boolean decisions
            perturb: AtomicFloat::new(0.0), // inject noise into sensing signal
            sensing_lp: AtomicFloat::new(0.0),  // low-pass sensing signal
        }
    }
}

// All plugins using `vst` also need to implement the `Plugin` trait.  Here, we
// define functions that give necessary info to our host.
impl Plugin for Effect {
    fn get_info(&self) -> Info {
        Info {
            name: "Glitch octave shift".to_string(),
            vendor: "".to_string(),
            unique_id: 243723072,
            version: 1,
            inputs: 2,
            outputs: 2,
            // This `parameters` bit is important; without it, none of our
            // parameters will be shown!
            parameters: 5,
            category: Category::Effect,
            ..Default::default()
        }
    }

    fn set_sample_rate(&mut self, rate: f32){
        self.sr = rate;
    }

    // called once
    fn init(&mut self) {
        info!("initialization...");

        // HACK: plugin crashes unless initialized vectors contain
        // at least one element, even if I check if they are empty. This is
        // because declaring a vector does not initialize it until something
        // is pushed to it, and there is no way to check if a vector is
        // initialized. Rust is cool, but sometimes it makes me want to throw
        // myself into the sea.
        // PS: this is retarded.
        for buf in &mut self.l_buffs_div{
            buf.push_back(0.0);
            buf.pop_front();
        }
        for buf in &mut self.r_buffs_div{
            buf.push_back(0.0);
            buf.pop_front();
        }
        self.aux_l_1.push_back(0.0);
        self.aux_r_1.pop_front();
        self.aux_r_1.push_back(0.0);
        self.aux_l_1.pop_front();
        self.yl.push_back(0.0);
        self.yl.pop_front();
        self.yr.push_back(0.0);
        self.yr.pop_front();
        // END of HACK
    }

    // Here is where the bulk of our audio processing code goes.
    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        let (inputs, outputs) = buffer.split();

        // Iterate over inputs as (&f32, &f32)
        let (l, r) = inputs.split_at(1);
        let stereo_in = l[0].iter().zip(r[0].iter());

        // Iterate over outputs as (&mut f32, &mut f32)
        let (mut l, mut r) = outputs.split_at_mut(1);
        let stereo_out = l[0].iter_mut().zip(r[0].iter_mut());

        // get all params
        let div = (self.params.div.get()*11.5 + 1.0) as usize;   // scale to int in 1..6
        let mut bias = self.params.bias.get()*2.0 - 1.0; // detect only "big" zero crossings
        bias = bias*bias*bias; // bias^3
        bias = if(bias < 0.0) {-(bias*bias)} else {bias*bias*bias*bias};  // bias^6 / bias 12
        let mut chaos = self.params.chaos.get();    // chance to flip some booleans
        chaos = chaos*chaos;
        let mut perturb = self.params.perturb.get();    // inject noise to sensing
        perturb = perturb/40.0;
        let mut cut = (1.0 - self.params.sensing_lp.get())*(self.sr/2.0);
        cut = (cut*cut*cut).sqrt().sqrt();

        // process
        for ((left_in, right_in), (left_out, right_out)) in stereo_in.zip(stereo_out) {
            // === compute pre-detection filtering =============================
            let xl = *left_in;
            let xr = *right_in;
            let mut x_sns = compute::lp(xl + xr, cut, 1.0/self.sr, self.x_s_p);
            x_sns += perturb*((self.rng.next_u64() as f32)/(u64::MAX as f32));
            
            // === compute div =================================================
            // apply chaos (skip detections)
            let mut detected = x_sns.signum() > (self.x_s_p + f32::EPSILON + bias).signum();
            if ((self.rng.next_u64() as f32)/(u64::MAX as f32) < chaos){
                detected = false;
            };

            if detected{
                self.count += 1;
                if self.count >= div{

                    self.aux_l_1.clear();
                    self.aux_r_1.clear();

                    compute::interlace(&mut self.l_buffs_div, div, &mut self.aux_l_1);
                    compute::interlace(&mut self.r_buffs_div, div, &mut self.aux_r_1);

                    while !self.aux_l_1.is_empty() && !self.aux_r_1.is_empty() {
                        let aux_l_2 = self.aux_l_1.pop_front();
                        let aux_r_2 = self.aux_r_1.pop_front();
                        self.yl.push_back(aux_l_2.unwrap());
                        self.yr.push_back(aux_r_2.unwrap());
                        self.count = 0;
                    }

                    // cap buffer to contain at most a single cycle at 20Hz after
                    // pitch shifting. This reduces latency while only affecting
                    // sub-sonic signals.
                    // TODO: consider high-passing sensing signal at ~30Hz to
                    // avoid artifacts induced by sub-sonic noise. Then again,
                    // this is meant to be glitchy so maybe don't
                    if self.yl.len() > (self.sr/120.0*6.0) as usize{
                        for i in 0..(self.yl.len() - self.sr as usize/120*6){
                            self.yl.pop_front();
                            self.yr.pop_front();
                        }
                    }
                }
            }

            self.l_buffs_div[self.count].push_back(xr);
            self.r_buffs_div[self.count].push_back(xl);

            *left_out = match self.yl.pop_front(){
                Some(front) => front,
                None => 0.0,
            };
            *right_out = match self.yr.pop_front(){
                Some(front) => front,
                None => 0.0,
            };

            self.x_s_p = x_sns;
        }
    }

    // Return the parameter object. This method can be omitted if the
    // plugin has no parameters.
    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }
}

impl PluginParameters for EffectParameters {
    // the `get_parameter` function reads the value of a parameter.
    fn get_parameter(&self, index: i32) -> f32 {
        match index {
            0 => self.div.get(),
            1 => self.bias.get(),
            2 => self.chaos.get(),
            3 => self.perturb.get(),
            4 => self.sensing_lp.get(),
            _ => 0.0,
        }
    }

    // the `set_parameter` function sets the value of a parameter.
    fn set_parameter(&self, index: i32, val: f32) {
        #[allow(clippy::single_match)]
        match index {
            0 => self.div.set(val),
            1 => self.bias.set(val),
            2 => self.chaos.set(val),
            3 => self.perturb.set(val),
            4 => self.sensing_lp.set(val),
            _ => (),
        }
    }

    // This is what will display underneath our control.  We can
    // format it into a string that makes the most since.
    fn get_parameter_text(&self, index: i32) -> String {
        match index {
            0 => format!("/ {}", (self.div.get()*11.5 + 1.0) as u8),
            1 => format!("{}", self.bias.get()*2.0 - 1.0),
            2 => format!("{}%", self.chaos.get()*100.0),
            3 => format!("{}", self.perturb.get()),
            4 => format!("{}", self.sensing_lp.get()),
            _ => "".to_string(),
        }
    }

    // This shows the control's name.
    fn get_parameter_name(&self, index: i32) -> String {
        match index {
            0 => "Freq div",
            1 => "Sens. bias",
            2 => "Sens. chaos",
            3 => "Sens. perturb",
            4 => "Sens. filter",
            _ => "",
        }
        .to_string()
    }
}

// This part is important!  Without it, our plugin won't work.
plugin_main!(Effect);