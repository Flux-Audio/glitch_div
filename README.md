# Installation
If you just want the vst plugin, download the `.dll` file in the `bin\` folder.
If you want to compile the code (not necessary to use the plugin) you can simply
run `cargo build` from the root of the repository. You will need to have Rust
installed to compile the code (not to use the vst though, that you can do right
away!).

# Quick Start
GLITCH_DIV is a glitchy pitch shifter. The div knob pitch shifts down by integer
divisions of the root note frequency. The pitch shifter is stable for any monophonic
input with little to no mutation over time. Any polyphonic signal or modulated
signal will start to produce artifacts (which is the point, so do it!).

Some additional parameters let you mess with the plugin even more. The bias knob
makes the plugin more or less sensitive to zero crossings (counting zero crossings
is how the plugin determines the input's pitch). Lower levels of bias will make
the plugin sloppier and higher biases will make it overly-sensitive, mis-detecting
near-zero signals as zero-crossings.

The chaos knob introduces a chance that a detected zero crossing will not be
counted. Making the pitch tracking unstable (introduces artifacts where the signal
briefly jumps to a different pitch).

The perturb knob adds noise to the detection path (but not to the sound you hear),
making the zero-crossing counter miss-fire, introducing a sort of hiss / geiger
noise floor to the signal.

The filter knob low-passes the detection path, this is useful if a complex waveform
confuses the pitch tracker, as it allows to better isolate the frequency of the
fundamental.

# How it works
GLITCH_DIV works by counting zero-crossings. The audio is buffered, without sending
anything out initially. When a full cycle of a waveform is detected, the buffer
is switched out to another buffer, this keeps on going until N buffers have been
filled, where N is the number of divisions of the input frequency.

The contents of the buffers are processed and put into an output queue, while 
the buffers are emptied and the zero-crossing counter is re-started.

The pitch shifting process consists in taking the N buffers (each containing a
consecutive wave cycle of the input) and interlacing them. Each of the buffers
is cut into equal slices (this is based on the ratio from the 0 buffer to the
other buffers) and the slices are woven together, to form a single wave cycle
spanning the original N wave cycles. This down-pitched wave is then pushed to
the output queue.

The output queue is read once per sample, if it's empty it returns 0.0, if it isn't
it returns the oldest element in the queue (FIFO).